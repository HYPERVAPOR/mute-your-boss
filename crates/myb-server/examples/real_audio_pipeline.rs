//! Real-audio pipeline integration example.
//!
//! This example wires the real sherpa-onnx KWS engine, the YAML policy engine,
//! the JSONL event log, and mock audio capture / volume controllers.  It takes
//! an audio file as input, feeds it through a `FocusSession`, and asserts that
//! the configured keyword triggers the expected volume change and event log
//! entry.
//!
//! Run with:
//!
//! ```bash
//! cargo run -p myb-server --example real_audio_pipeline \
//!   -- --model models/sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20 \
//!   --audio models/sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20/test_wavs/en_0.wav \
//!   --keyword "LIGHT_UP=L AY1 T AH1 P"
//! ```

use myb_audio_capture::mock::MockAudioCapture;
use myb_core::traits::audio::{AudioChunk, AudioProcessInfo};
use myb_core::traits::volume::VolumeController;
use myb_core::{EventLog, FocusSession, KeywordVocab};
use myb_event_log::EventLog as JsonlEventLog;
use myb_kws::{KeywordVocabExt, KwsConfig, SherpaKwsEngine};
use myb_policy::PolicyEngine as YamlPolicyEngine;
use myb_volume_control::mock::MockVolumeController;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::path::PathBuf;
use symphonia::core::audio::{AudioBufferRef, SampleBuffer};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug)]
struct Args {
    model_dir: PathBuf,
    audio: PathBuf,
    keyword: String,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = std::env::args().skip(1);
    let mut model_dir = None;
    let mut audio = None;
    let mut keyword = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--model" => model_dir = args.next().map(PathBuf::from),
            "--audio" => audio = args.next().map(PathBuf::from),
            "--keyword" => keyword = args.next(),
            _ => eprintln!("warning: unknown arg {arg}"),
        }
    }

    let model_dir = model_dir
        .unwrap_or_else(|| PathBuf::from("models/sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20"));
    let audio = audio.unwrap_or_else(|| model_dir.join("test_wavs").join("en_0.wav"));
    let keyword = keyword.unwrap_or_else(|| "LIGHT_UP=L AY1 T AH1 P".into());

    Ok(Args {
        model_dir,
        audio,
        keyword,
    })
}

fn decode_audio(path: &std::path::Path) -> anyhow::Result<(Vec<f32>, u32, usize)> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let hint = Hint::new();
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow::anyhow!("no audio track found"))?;

    let sample_rate = track
        .codec_params
        .sample_rate
        .ok_or_else(|| anyhow::anyhow!("unknown sample rate"))?;
    let n_channels = track.codec_params.channels.map(|c| c.count()).unwrap_or(1);

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    let mut samples: Vec<f32> = Vec::new();
    let track_id = track.id;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break
            }
            Err(err) => return Err(err.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => append_decoded(decoded, n_channels, &mut samples),
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(err) => return Err(err.into()),
        }
    }

    Ok((samples, sample_rate, n_channels))
}

fn append_decoded(buf: AudioBufferRef<'_>, n_channels: usize, out: &mut Vec<f32>) {
    let capacity = buf.capacity() * n_channels;
    let mut sample_buf = SampleBuffer::<f32>::new(capacity as u64, *buf.spec());
    sample_buf.copy_interleaved_ref(buf);
    out.extend_from_slice(sample_buf.samples());
}

fn to_mono(interleaved: &[f32], n_channels: usize) -> Vec<f32> {
    if n_channels == 1 {
        interleaved.to_vec()
    } else {
        let n_frames = interleaved.len() / n_channels;
        let mut mono = Vec::with_capacity(n_frames);
        for i in 0..n_frames {
            let sum: f32 = interleaved[i * n_channels..(i + 1) * n_channels]
                .iter()
                .sum();
            mono.push(sum / n_channels as f32);
        }
        mono
    }
}

fn resample(mono: &[f32], src_rate: u32, dst_rate: u32) -> anyhow::Result<Vec<f32>> {
    if src_rate == dst_rate {
        return Ok(mono.to_vec());
    }

    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        dst_rate as f64 / src_rate as f64,
        2.0,
        params,
        mono.len(),
        1,
    )?;

    let waves_in = vec![mono.to_vec()];
    let waves_out = resampler.process(&waves_in, None)?;
    Ok(waves_out.into_iter().next().unwrap_or_default())
}

fn build_policy(keyword: &str) -> anyhow::Result<YamlPolicyEngine> {
    let yaml = format!(
        r#"
policies:
  - name: unmute-on-keyword
    match:
      keywords:
        - "{keyword}"
      threshold: 0.25
    action:
      volume: 100
      duration_seconds: 5
      then: mute
"#
    );
    YamlPolicyEngine::from_yaml(&yaml)
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = parse_args()?;
    println!("Model dir: {}", args.model_dir.display());
    println!("Audio:     {}", args.audio.display());
    println!("Keyword:   {}", args.keyword);

    // Build KWS vocabulary from the single keyword under test.
    let mut vocab = KeywordVocab::new();
    vocab.add_str(&args.keyword)?;

    let mut config = KwsConfig::from_model_dir(&args.model_dir);
    config.model_paths.keywords_file = {
        let path = std::env::temp_dir().join("myb_real_audio_pipeline_keywords.txt");
        vocab.write_to_file(&path)?;
        path
    };

    let kws = Box::new(SherpaKwsEngine::new(config)?);

    // Decode and resample the input audio to 16 kHz mono f32.
    println!("Decoding audio...");
    let (interleaved, src_rate, n_channels) = decode_audio(&args.audio)?;
    let mono = to_mono(&interleaved, n_channels);
    let resampled = resample(&mono, src_rate, 16_000)?;
    println!(
        "Original: {} Hz / {} channels / {} samples; resampled: {} samples @ 16 kHz",
        src_rate,
        n_channels,
        interleaved.len(),
        resampled.len()
    );

    // Chop the PCM into 1-second chunks to simulate a live stream.
    let chunk_samples = 16_000usize;
    let mut chunks = Vec::new();
    let mut timestamp_ms = 0i64;
    for chunk in resampled.chunks(chunk_samples) {
        chunks.push(AudioChunk {
            timestamp_ms,
            samples: chunk.to_vec(),
        });
        timestamp_ms += (chunk.len() as f64 / 16.0) as i64;
    }

    // Remember how many chunks we have so we can drive the session to the end
    // of this finite audio file.  A live session would run until `stop()` is
    // called instead.
    let n_chunks = chunks.len();

    let audio = MockAudioCapture::new(
        vec![AudioProcessInfo {
            pid: 1234,
            name: "real-audio-test".into(),
            window_title: None,
            current_volume: 0.0,
            is_meeting_app: true,
        }],
        chunks,
    );

    let policy = Box::new(build_policy(&args.keyword)?);
    let volume = Box::new(MockVolumeController::new());
    let event_log = Box::new(JsonlEventLog::new());

    let mut session = FocusSession::new(
        "real-audio-test".into(),
        1234,
        Box::new(audio),
        kws,
        policy,
        volume.clone(),
        event_log.clone(),
    )?;

    session.start()?;
    println!("Session state: {:?}", session.state());

    // Drive the session through every chunk of the pre-recorded audio.
    for _ in 0..n_chunks {
        if let Err(e) = session.tick() {
            eprintln!("tick error: {e}");
            break;
        }
    }

    let final_volume = volume.get_volume(1234)?;
    let events = event_log.recent(10);

    println!("Final volume: {final_volume}");
    println!("Logged events: {}", events.len());
    for ev in &events {
        println!(
            "  - {:.2}s keyword={} confidence={:.2}",
            ev.timestamp_ms as f64 / 1000.0,
            ev.keyword,
            ev.confidence
        );
    }

    // Assertions that define the expected behaviour.
    let keyword_display = myb_core::KeywordEntry::display_of(&args.keyword);
    assert!(
        final_volume > 0.9,
        "expected volume to be restored after keyword '{keyword_display}'"
    );
    assert!(
        events.iter().any(|e| e.keyword == keyword_display),
        "expected at least one event for keyword '{keyword_display}'"
    );

    session.stop()?;
    Ok(())
}
