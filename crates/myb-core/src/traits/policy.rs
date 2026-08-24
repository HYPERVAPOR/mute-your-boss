/// Volume decision produced by the policy engine.
#[derive(Debug, Clone, Copy)]
pub enum VolumeDecision {
    /// Set volume to this level (0–100) and keep it for `duration_seconds`.
    SetVolume { volume: u32, duration_seconds: u32 },
    /// No policy matched; apply the default action (usually mute).
    Default,
}

/// Policy engine interface.
pub trait PolicyEngine: Send + Sync {
    /// Evaluate a keyword hit and decide what volume action to take.
    fn evaluate(&self, keyword: &str, confidence: f64) -> VolumeDecision;
}
