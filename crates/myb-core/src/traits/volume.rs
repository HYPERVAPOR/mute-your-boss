use std::fmt;

/// Platform-agnostic volume control interface for a single audio session.
pub trait VolumeController: Send + Sync {
    /// Get the current volume of the target process in the range [0.0, 1.0].
    fn get_volume(&self, pid: u32) -> anyhow::Result<f32>;

    /// Set the volume of the target process.
    ///
    /// `volume` is in the range [0.0, 1.0]. Implementations may apply a short
    /// fade to avoid popping.
    fn set_volume(&self, pid: u32, volume: f32) -> anyhow::Result<()>;

    /// Mute the target process (equivalent to `set_volume(pid, 0.0)`).
    fn mute(&self, pid: u32) -> anyhow::Result<()> {
        self.set_volume(pid, 0.0)
    }

    /// Unmute the target process (restore to the previous non-zero volume).
    ///
    /// Default implementation simply sets volume to 1.0. Stateful controllers
    /// should override this to restore the volume captured before `mute`.
    fn unmute(&self, pid: u32) -> anyhow::Result<()> {
        self.set_volume(pid, 1.0)
    }
}

impl fmt::Debug for dyn VolumeController + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VolumeController").finish()
    }
}
