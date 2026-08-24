pub mod engine;
pub mod vocab;

pub use engine::{KwsConfig, KwsModelPaths, SherpaKwsEngine};
pub use vocab::{KeywordEntry, KeywordVocab};

use myb_core::{AudioStream, KwsEngine as KwsEngineTrait, KwsHit};

/// A no-op keyword spotting engine for early pipeline wiring and tests.
#[derive(Debug, Default)]
pub struct StubKwsEngine {
    _placeholder: (),
}

impl StubKwsEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KwsEngineTrait for StubKwsEngine {
    fn process_chunk(
        &mut self,
        _samples: &[f32],
        _timestamp_ms: i64,
    ) -> anyhow::Result<Vec<KwsHit>> {
        Ok(vec![])
    }
}

/// Convenience helper: drain hits from an audio stream one chunk at a time.
pub fn process_stream(
    engine: &mut dyn KwsEngineTrait,
    stream: &mut dyn AudioStream,
) -> anyhow::Result<Vec<KwsHit>> {
    let mut hits = vec![];
    while let Some(chunk) = stream.next_chunk()? {
        hits.extend(engine.process_chunk(&chunk.samples, chunk.timestamp_ms)?);
    }
    Ok(hits)
}
