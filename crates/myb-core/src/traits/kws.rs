/// A single keyword hit produced by the KWS engine.
#[derive(Debug, Clone)]
pub struct KwsHit {
    pub keyword: String,
    pub confidence: f64,
    pub timestamp_ms: i64,
}

/// Keyword spotting engine interface.
pub trait KwsEngine: Send + Sync {
    /// Process one chunk of 16kHz/mono/f32 PCM audio and return detected keywords.
    fn process_chunk(&mut self, samples: &[f32], timestamp_ms: i64) -> anyhow::Result<Vec<KwsHit>>;
}
