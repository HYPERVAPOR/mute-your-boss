use myb_core::traits::volume::VolumeController;
use std::sync::{Arc, Mutex};

/// A mock volume controller for unit testing.
///
/// It stores the last set volume per PID in memory.
#[derive(Debug, Default, Clone)]
pub struct MockVolumeController {
    state: Arc<Mutex<std::collections::HashMap<u32, f32>>>,
}

impl MockVolumeController {
    pub fn new() -> Self {
        Self::default()
    }
}

impl VolumeController for MockVolumeController {
    fn get_volume(&self, pid: u32) -> anyhow::Result<f32> {
        let state = self.state.lock().unwrap();
        Ok(*state.get(&pid).unwrap_or(&1.0))
    }

    fn set_volume(&self, pid: u32, volume: f32) -> anyhow::Result<()> {
        let mut state = self.state.lock().unwrap();
        state.insert(pid, volume.clamp(0.0, 1.0));
        Ok(())
    }
}
