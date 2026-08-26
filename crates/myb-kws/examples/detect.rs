use myb_core::{KeywordVocab, KwsEngine};
use myb_kws::{KeywordVocabExt, KwsConfig, SherpaKwsEngine};
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

/// Decode an audio file into interleaved f32 samples, sample rate, and channel count.
fn decode_audio(path: &str) -> anyhow::Result<(Vec<f32>, u32, usize)> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let hint = Hint::new();
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe().format(
        &hint,
        mss,
        &fmt_opts,
        &meta_opts,
    )?;
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
    let n_channels = track
        .codec_params
        .channels
        .map(|c| c.count())
        .unwrap_or(1);

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs().make(&track.codec_params, &dec_opts)?;

    let mut samples: Vec<f32> = Vec::new();
    let track_id = track.id;

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                append_decoded(decoded, n_channels, &mut samples);
            }
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

/// Average interleaved channels down to mono.
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

/// Resample mono f32 audio to the target sample rate.
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

#[derive(Debug)]
struct Args {
    model_dir: PathBuf,
    audio: String,
    keywords: Vec<String>,
    keywords_file: Option<PathBuf>,
}

fn parse_args() -> anyhow::Result<Args> {
    let mut args = std::env::args().skip(1);
    let mut model_dir = None;
    let mut audio = None;
    let mut keywords = Vec::new();
    let mut keywords_file = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--model" => model_dir = args.next().map(PathBuf::from),
            "--audio" => audio = args.next(),
            "--keyword" => {
                if let Some(kw) = args.next() {
                    keywords.push(kw);
                }
            }
            "--keywords-file" => keywords_file = args.next().map(PathBuf::from),
            _ => eprintln!("warning: unknown arg {}", arg),
        }
    }

    let model_dir = model_dir.ok_or_else(|| anyhow::anyhow!("--model is required"))?;
    let audio = audio.ok_or_else(|| anyhow::anyhow!("--audio is required"))?;

    Ok(Args {
        model_dir,
        audio,
        keywords,
        keywords_file,
    })
}

fn main() -> anyhow::Result<()> {
    let args = parse_args()?;

    // Build vocabulary.
    let mut vocab = KeywordVocab::new();
    for kw in &args.keywords {
        vocab.add_str(kw)?;
    }
    if let Some(path) = &args.keywords_file {
        let buf = std::fs::read_to_string(path)?;
        for line in buf.lines() {
            let line = line.trim();
            if !line.is_empty() && !line.starts_with('#') {
                vocab.add_str(line)?;
            }
        }
    }
    anyhow::ensure!(!vocab.is_empty(), "at least one keyword is required");

    let mut config = KwsConfig::from_model_dir(&args.model_dir);
    config.model_paths.keywords_file = {
        let path = std::env::temp_dir().join("myb_detect_keywords.txt");
        vocab.write_to_file(&path)?;
        path
    };

    let mut engine = SherpaKwsEngine::new(config)?;

    // Decode, convert to mono, resample to 16 kHz.
    println!("Decoding {} ...", args.audio);
    let (interleaved, src_rate, n_channels) = decode_audio(&args.audio)?;
    let mono = to_mono(&interleaved, n_channels);
    let resampled = resample(&mono, src_rate, 16000)?;

    println!(
        "Original: {} Hz, {} samples; resampled to 16 kHz, {} samples",
        src_rate,
        interleaved.len(),
        resampled.len()
    );

    // Process in 1-second chunks to simulate streaming.
    let chunk_samples = 16000usize;
    let mut timestamp_ms = 0i64;
    let mut total_hits = 0usize;
    for chunk in resampled.chunks(chunk_samples) {
        let hits = engine.process_chunk(chunk, timestamp_ms)?;
        for hit in hits {
            println!(
                "Detected: {} @ {:.2}s",
                hit.keyword,
                hit.timestamp_ms as f64 / 1000.0
            );
            total_hits += 1;
        }
        timestamp_ms += (chunk.len() as f64 / 16.0) as i64; // ms at 16 kHz
    }

    println!("Total hits: {}", total_hits);
    Ok(())
}
