use myb_core::traits::volume::VolumeController;

/// Windows `ISimpleAudioVolume` per-session volume control implementation.
///
/// This is a stub for M1.1; the actual WASAPI implementation is M1.3.
#[derive(Debug, Default)]
pub struct WindowsVolumeController;

impl WindowsVolumeController {
    pub fn new() -> Self {
        Self
    }
}

impl VolumeController for WindowsVolumeController {
    fn get_volume(&self, _pid: u32) -> anyhow::Result<f32> {
        anyhow::bail!("Windows volume control not yet implemented (M1.3)")
    }

    fn set_volume(&self, _pid: u32, _volume: f32) -> anyhow::Result<()> {
        anyhow::bail!("Windows volume control not yet implemented (M1.3)")
    }
}
