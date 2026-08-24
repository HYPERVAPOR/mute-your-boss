pub mod mock;

#[cfg(target_os = "windows")]
pub mod windows;

#[cfg(target_os = "windows")]
pub use windows::WindowsVolumeController;

pub use mock::MockVolumeController;
