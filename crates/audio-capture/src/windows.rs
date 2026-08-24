use myb_core::traits::audio::{AudioCapture, AudioChunk, AudioProcessInfo, AudioStream};

/// Windows WASAPI per-process loopback capture implementation.
///
/// This is a stub for M1.1; the actual WASAPI implementation is M1.2.
#[derive(Debug, Default)]
pub struct WindowsAudioCapture;

impl WindowsAudioCapture {
    pub fn new() -> Self {
        Self
    }
}

impl AudioCapture for WindowsAudioCapture {
    fn list_processes(&self) -> anyhow::Result<Vec<AudioProcessInfo>> {
        Ok(vec![])
    }

    fn start_capture(&self, _pid: u32) -> anyhow::Result<Box<dyn AudioStream>> {
        anyhow::bail!("WASAPI capture not yet implemented (M1.2)")
    }
}
