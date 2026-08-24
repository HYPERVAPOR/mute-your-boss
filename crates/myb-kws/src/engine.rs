use myb_core::{KwsEngine as KwsEngineTrait, KwsHit};
use sherpa_onnx::{KeywordResult, KeywordSpotter, KeywordSpotterConfig};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Paths to a sherpa-onnx keyword spotting model.
#[derive(Debug, Clone)]
pub struct KwsModelPaths {
    pub encoder: PathBuf,
    pub decoder: PathBuf,
    pub joiner: PathBuf,
    pub tokens: PathBuf,
    pub keywords_file: PathBuf,
}

impl KwsModelPaths {
    /// Validate that all required model files exist.
    pub fn validate(&self) -> anyhow::Result<()> {
        for (name, path) in [
            ("encoder", &self.encoder),
            ("decoder", &self.decoder),
            ("joiner", &self.joiner),
            ("tokens", &self.tokens),
            ("keywords_file", &self.keywords_file),
        ] {
            if !path.exists() {
                anyhow::bail!("KWS {name} file not found: {}", path.display());
            }
        }
        Ok(())
    }
}

/// Configuration for the sherpa-onnx keyword spotting engine.
#[derive(Debug, Clone)]
pub struct KwsConfig {
    pub model_paths: KwsModelPaths,
    pub provider: String,
    pub num_threads: i32,
    pub debug: bool,
}

impl KwsConfig {
    /// Build a config from a model directory, using the default file names for the
    /// `sherpa-onnx-kws-zipformer-zh-en-3M-2025-12-20` model layout.
    pub fn from_model_dir<P: AsRef<Path>>(dir: P) -> Self {
        let dir = dir.as_ref();
        Self {
            model_paths: KwsModelPaths {
                encoder: dir.join("encoder-epoch-13-avg-2-chunk-16-left-64.onnx"),
                decoder: dir.join("decoder-epoch-13-avg-2-chunk-16-left-64.onnx"),
                joiner: dir.join("joiner-epoch-13-avg-2-chunk-16-left-64.onnx"),
                tokens: dir.join("tokens.txt"),
                keywords_file: dir.join("keywords.txt"),
            },
            provider: "cpu".to_string(),
            num_threads: 1,
            debug: false,
        }
    }
}

/// A keyword spotting engine backed by sherpa-onnx.
pub struct SherpaKwsEngine {
    spotter: KeywordSpotter,
    sample_rate: i32,
    /// In-memory buffer used to feed audio chunk-by-chunk into a single stream.
    state: Mutex<StreamState>,
}

struct StreamState {
    samples: Vec<f32>,
    /// Timestamp (ms) of the first sample currently in `samples`.
    base_timestamp_ms: i64,
    /// Whether the previous call yielded a result; used for basic debouncing.
    last_result: Option<String>,
}

impl SherpaKwsEngine {
    /// Create a new engine from the provided configuration.
    ///
    /// This loads the model files and fails fast if anything is missing or invalid.
    pub fn new(config: KwsConfig) -> anyhow::Result<Self> {
        config.model_paths.validate()?;

        let mut spotter_config = KeywordSpotterConfig::default();
        spotter_config.model_config.transducer.encoder =
            Some(config.model_paths.encoder.to_string_lossy().into_owned());
        spotter_config.model_config.transducer.decoder =
            Some(config.model_paths.decoder.to_string_lossy().into_owned());
        spotter_config.model_config.transducer.joiner =
            Some(config.model_paths.joiner.to_string_lossy().into_owned());
        spotter_config.model_config.tokens =
            Some(config.model_paths.tokens.to_string_lossy().into_owned());
        spotter_config.model_config.provider = Some(config.provider);
        spotter_config.model_config.num_threads = config.num_threads;
        spotter_config.model_config.debug = config.debug;
        spotter_config.keywords_file = Some(
            config
                .model_paths
                .keywords_file
                .to_string_lossy()
                .into_owned(),
        );

        let spotter = KeywordSpotter::create(&spotter_config)
            .ok_or_else(|| anyhow::anyhow!("Failed to create KeywordSpotter"))?;

        let sample_rate = spotter_config.feat_config.sample_rate;
        tracing::info!(
            "SherpaKwsEngine loaded; sample_rate={sample_rate}, keywords_file={}",
            config.model_paths.keywords_file.display()
        );

        Ok(Self {
            spotter,
            sample_rate,
            state: Mutex::new(StreamState {
                samples: Vec::new(),
                base_timestamp_ms: 0,
                last_result: None,
            }),
        })
    }

    /// Create a stream pre-loaded with extra keywords.
    ///
    /// `extra_keywords` uses the sherpa-onnx text format, e.g.
    /// "y ǎn y uán @演员/zh ī m íng @知名".
    pub fn create_stream_with_keywords(
        &self,
        extra_keywords: &str,
    ) -> anyhow::Result<KeywordHits<'_>> {
        let stream = self.spotter.create_stream_with_keywords(extra_keywords);
        Ok(KeywordHits {
            spotter: &self.spotter,
            stream,
        })
    }

    fn result_to_hit(result: &KeywordResult, base_timestamp_ms: i64) -> Option<KwsHit> {
        if result.keyword.is_empty() {
            return None;
        }
        // Sherpa-onnx returns start_time relative to the stream; add the base offset.
        let start_ms = (result.start_time * 1000.0) as i64 + base_timestamp_ms;
        Some(KwsHit {
            keyword: result.keyword.clone(),
            // Sherpa-onnx KWS result does not expose a confidence score.
            // Use a neutral placeholder until we parse it from `json`.
            confidence: 1.0,
            timestamp_ms: start_ms,
        })
    }
}

impl KwsEngineTrait for SherpaKwsEngine {
    fn process_chunk(&mut self, samples: &[f32], timestamp_ms: i64) -> anyhow::Result<Vec<KwsHit>> {
        let mut state = self.state.lock().unwrap();

        // Reset the buffer if the caller skipped ahead.
        if state.samples.is_empty() {
            state.base_timestamp_ms = timestamp_ms;
        }

        state.samples.extend_from_slice(samples);

        // We create a fresh stream from the buffered audio so far and decode it.
        // This is simple but not the most efficient; for production we should keep
        // a persistent stream and slide a window. Good enough for M1.4.
        let stream = self.spotter.create_stream();
        stream.accept_waveform(self.sample_rate, &state.samples);
        stream.input_finished();

        let mut hits = Vec::new();
        while self.spotter.is_ready(&stream) {
            self.spotter.decode(&stream);
            if let Some(result) = self.spotter.get_result(&stream) {
                if !result.keyword.is_empty() {
                    // Simple debounce: skip duplicate consecutive results.
                    if state.last_result.as_deref() != Some(&result.keyword) {
                        if let Some(hit) = Self::result_to_hit(&result, state.base_timestamp_ms) {
                            hits.push(hit);
                        }
                        state.last_result = Some(result.keyword.clone());
                    }
                    self.spotter.reset(&stream);
                }
            }
        }

        Ok(hits)
    }
}

/// Helper to detect keywords from a complete WAV file using a pre-created stream.
pub struct KeywordHits<'a> {
    spotter: &'a KeywordSpotter,
    stream: sherpa_onnx::OnlineStream,
}

impl<'a> KeywordHits<'a> {
    pub fn accept_waveform(&self, sample_rate: i32, samples: &[f32]) {
        self.stream.accept_waveform(sample_rate, samples);
    }

    pub fn input_finished(&self) {
        self.stream.input_finished();
    }

    pub fn decode(&self) -> Vec<KeywordResult> {
        let mut results = Vec::new();
        while self.spotter.is_ready(&self.stream) {
            self.spotter.decode(&self.stream);
            if let Some(result) = self.spotter.get_result(&self.stream) {
                if !result.keyword.is_empty() {
                    results.push(result.clone());
                    self.spotter.reset(&self.stream);
                }
            }
        }
        results
    }
}
