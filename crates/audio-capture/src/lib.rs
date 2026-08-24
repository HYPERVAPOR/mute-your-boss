pub mod mock;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::WindowsAudioCapture;

pub use mock::MockAudioCapture;
