use std::fmt;

/// Metadata about an audio-outputting process.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioProcessInfo {
    pub pid: u32,
    pub name: String,
    pub window_title: Option<String>,
    /// Current session volume in the range [0.0, 1.0].
    pub current_volume: f32,
    /// Whether this process matches a known meeting application.
    pub is_meeting_app: bool,
}

/// A single chunk of PCM audio.
///
/// Format: 16 kHz sample rate, mono, 32-bit float (`f32`).
#[derive(Debug, Clone)]
pub struct AudioChunk {
    /// Unix timestamp in milliseconds when this chunk was captured.
    pub timestamp_ms: i64,
    /// PCM samples.
    pub samples: Vec<f32>,
}

/// Abstraction over a continuous audio stream from a single process.
pub trait AudioStream: Send + Sync {
    /// Pull the next chunk of audio.
    ///
    /// Returns `None` when the stream has ended or the target process exited.
    fn next_chunk(&mut self) -> anyhow::Result<Option<AudioChunk>>;
}

impl fmt::Debug for dyn AudioStream + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioStream").finish()
    }
}

/// Platform-agnostic audio capture interface.
pub trait AudioCapture: Send + Sync {
    /// List processes that are currently producing audio output.
    fn list_processes(&self) -> anyhow::Result<Vec<AudioProcessInfo>>;

    /// Start capturing audio from the process identified by `pid`.
    ///
    /// Returns a stream that yields `AudioChunk`s until capture stops.
    fn start_capture(&self, pid: u32) -> anyhow::Result<Box<dyn AudioStream>>;
}

impl fmt::Debug for dyn AudioCapture + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AudioCapture").finish()
    }
}
