use myb_core::KwsEngine;
use myb_kws::{KeywordVocab, KwsConfig, SherpaKwsEngine};
use sherpa_onnx::Wave;
use std::path::PathBuf;

fn build_vocab_from_policy() -> KeywordVocab {
    let policy = myb_policy::Policy {
        name: "unmute".into(),
        match_: myb_policy::KeywordMatch {
            keywords: vec!["LIGHT_UP=L AY1 T AH1 P".into()],
            threshold: 0.25,
        },
        action: myb_policy::PolicyAction {
            volume: 100,
            duration_seconds: 5,
            then: "mute".into(),
        },
    };
    KeywordVocab::from_policies(&[policy]).expect("build vocab")
}

fn model_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../models/sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20")
}

#[test]
fn detects_english_keyword_from_test_wav() {
    let dir = model_dir();
    if !dir.exists() {
        eprintln!("model dir not found, skipping");
        return;
    }

    let mut config = KwsConfig::from_model_dir(&dir);
    // The bundled keywords file lives under test_wavs/.
    config.model_paths.keywords_file = dir.join("test_wavs/keywords.txt");

    let mut engine = SherpaKwsEngine::new(config).expect("create engine");

    let wave_path = dir.join("test_wavs/en_0.wav");
    let wave = Wave::read(wave_path.to_str().unwrap()).expect("read wav");
    let hits = engine
        .process_chunk(wave.samples(), 0)
        .expect("process chunk");

    let keywords: Vec<_> = hits.iter().map(|h| h.keyword.as_str()).collect();
    assert!(
        keywords.contains(&"LIGHT_UP"),
        "expected LIGHT_UP in {:?}",
        keywords
    );
}

#[test]
fn detects_chinese_keyword_from_test_wav() {
    let dir = model_dir();
    if !dir.exists() {
        eprintln!("model dir not found, skipping");
        return;
    }

    let mut config = KwsConfig::from_model_dir(&dir);
    config.model_paths.keywords_file = dir.join("test_wavs/keywords.txt");

    let mut engine = SherpaKwsEngine::new(config).expect("create engine");

    // zh_3.wav is the shortest Chinese test clip.
    let wave_path = dir.join("test_wavs/zh_3.wav");
    let wave = Wave::read(wave_path.to_str().unwrap()).expect("read wav");
    let hits = engine
        .process_chunk(wave.samples(), 0)
        .expect("process chunk");

    let keywords: Vec<_> = hits.iter().map(|h| h.keyword.as_str()).collect();
    assert!(
        !keywords.is_empty(),
        "expected at least one Chinese keyword hit"
    );
}

#[test]
fn detects_keyword_from_policy_generated_vocab() {
    let dir = model_dir();
    if !dir.exists() {
        eprintln!("model dir not found, skipping");
        return;
    }

    let vocab = build_vocab_from_policy();
    let keywords_file = dir.join("test_wavs/policy_keywords.txt");
    vocab.write_to_file(&keywords_file).expect("write vocab");

    let mut config = KwsConfig::from_model_dir(&dir);
    config.model_paths.keywords_file = keywords_file;

    let mut engine = SherpaKwsEngine::new(config).expect("create engine");

    let wave_path = dir.join("test_wavs/en_0.wav");
    let wave = Wave::read(wave_path.to_str().unwrap()).expect("read wav");
    let hits = engine
        .process_chunk(wave.samples(), 0)
        .expect("process chunk");

    let keywords: Vec<_> = hits.iter().map(|h| h.keyword.as_str()).collect();
    assert!(
        keywords.contains(&"LIGHT_UP"),
        "expected LIGHT_UP in {:?}",
        keywords
    );
}
