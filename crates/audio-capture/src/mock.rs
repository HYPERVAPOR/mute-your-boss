use myb_core::traits::audio::{AudioCapture, AudioChunk, AudioProcessInfo, AudioStream};

/// A mock audio capture implementation for unit testing.
///
/// It returns a fixed list of processes and feeds audio from an in-memory
/// buffer.
#[derive(Debug, Clone)]
pub struct MockAudioCapture {
    processes: Vec<AudioProcessInfo>,
    chunks: Vec<AudioChunk>,
}

impl MockAudioCapture {
    pub fn new(processes: Vec<AudioProcessInfo>, chunks: Vec<AudioChunk>) -> Self {
        Self { processes, chunks }
    }
}

impl AudioCapture for MockAudioCapture {
    fn list_processes(&self) -> anyhow::Result<Vec<AudioProcessInfo>> {
        Ok(self.processes.clone())
    }

    fn start_capture(&self, _pid: u32) -> anyhow::Result<Box<dyn AudioStream>> {
        let stream = MockAudioStream {
            chunks: self.chunks.clone(),
            index: 0,
        };
        Ok(Box::new(stream))
    }
}

struct MockAudioStream {
    chunks: Vec<AudioChunk>,
    index: usize,
}

impl AudioStream for MockAudioStream {
    fn next_chunk(&mut self) -> anyhow::Result<Option<AudioChunk>> {
        if self.index < self.chunks.len() {
            let chunk = self.chunks[self.index].clone();
            self.index += 1;
            Ok(Some(chunk))
        } else {
            Ok(None)
        }
    }
}
