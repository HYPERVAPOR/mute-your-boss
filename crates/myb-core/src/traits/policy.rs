/// Volume decision produced by the policy engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeDecision {
    /// Set volume to this level (0–100) and keep it for `duration_seconds`.
    SetVolume { volume: u32, duration_seconds: u32 },
    /// A renewal hit for an already-active policy; the consumer should extend
    /// the current volume duration without changing the volume level.
    Renew,
    /// No policy matched; apply the default action (usually mute).
    Default,
}

/// Policy engine interface.
pub trait PolicyEngine: Send + Sync {
    /// Evaluate a keyword hit and decide what volume action to take.
    ///
    /// `timestamp_ms` is the engine-provided time of the hit so the policy
    /// engine can debounce and renew without relying on the system clock.
    fn evaluate(&mut self, keyword: &str, confidence: f64, timestamp_ms: i64) -> VolumeDecision;
}
