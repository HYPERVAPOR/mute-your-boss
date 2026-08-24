use myb_core::{AudioStream, KwsEngine as KwsEngineTrait, KwsHit};

/// Keyword spotting engine stub.
///
/// In M1.4 this will load a sherpa-onnx model and perform streaming detection.
/// For now it returns no hits so the pipeline can be wired up.
#[derive(Debug, Default)]
pub struct KwsEngine {
    _placeholder: (),
}

impl KwsEngine {
    pub fn new() -> Self {
        Self::default()
    }
}

impl KwsEngineTrait for KwsEngine {
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
