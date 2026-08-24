use crate::traits::{
    AudioCapture, AudioStream, EventLog, KwsEngine, PolicyEngine, TriggerEvent, VolumeController,
    VolumeDecision,
};

/// Runtime state of a focus session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session created but not yet started.
    Idle,
    /// Actively capturing and detecting keywords.
    Listening,
    /// A policy has triggered and the volume is being controlled.
    Triggered,
    /// Automatic control is paused because the user manually changed volume.
    Paused,
}

/// A single focus session: captures audio from one process, detects keywords,
/// evaluates policies, and controls the target process volume.
pub struct FocusSession {
    session_id: String,
    target_pid: u32,
    audio: Box<dyn AudioCapture>,
    audio_stream: Option<Box<dyn AudioStream>>,
    kws: Box<dyn KwsEngine>,
    policy: Box<dyn PolicyEngine>,
    volume: Box<dyn VolumeController>,
    event_log: Box<dyn EventLog>,
    state: SessionState,
    /// Volume level we last set, used to detect external manual changes.
    expected_volume: Option<f32>,
}

/// Tolerance for detecting external volume changes.
const VOLUME_EPSILON: f32 = 0.02;

impl FocusSession {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        session_id: String,
        target_pid: u32,
        audio: Box<dyn AudioCapture>,
        kws: Box<dyn KwsEngine>,
        policy: Box<dyn PolicyEngine>,
        volume: Box<dyn VolumeController>,
        event_log: Box<dyn EventLog>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            session_id,
            target_pid,
            audio,
            audio_stream: None,
            kws,
            policy,
            volume,
            event_log,
            state: SessionState::Idle,
            expected_volume: None,
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn target_pid(&self) -> u32 {
        self.target_pid
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Start capturing audio from the target process.
    pub fn start(&mut self) -> anyhow::Result<()> {
        self.audio_stream = Some(self.audio.start_capture(self.target_pid)?);
        self.state = SessionState::Listening;
        Ok(())
    }

    /// Process one audio chunk and apply any volume decisions.
    pub fn tick(&mut self) -> anyhow::Result<()> {
        let Some(stream) = self.audio_stream.as_deref_mut() else {
            anyhow::bail!("session not started");
        };

        let Some(chunk) = stream.next_chunk()? else {
            return Ok(());
        };

        // If we are controlling volume, check whether the user changed it
        // manually outside of our control.
        if self.state == SessionState::Triggered {
            if let Ok(current) = self.volume.get_volume(self.target_pid) {
                if let Some(expected) = self.expected_volume {
                    if (current - expected).abs() > VOLUME_EPSILON {
                        self.state = SessionState::Paused;
                        tracing::info!(
                            session_id = %self.session_id,
                            "external volume change detected; pausing automatic control"
                        );
                    }
                }
            }
        }

        let hits = match self.kws.process_chunk(&chunk.samples, chunk.timestamp_ms) {
            Ok(hits) => hits,
            Err(e) => {
                self.restore_volume_safely("KWS error");
                return Err(e);
            }
        };

        for hit in hits {
            let decision = self
                .policy
                .evaluate(&hit.keyword, hit.confidence, hit.timestamp_ms);
            match decision {
                VolumeDecision::SetVolume {
                    volume,
                    duration_seconds,
                } => {
                    if self.state == SessionState::Paused {
                        // While paused we ignore automatic triggers.
                        continue;
                    }
                    let normalized = volume.clamp(0, 100) as f32 / 100.0;
                    if let Err(e) = self.volume.set_volume(self.target_pid, normalized) {
                        self.restore_volume_safely("set_volume failed");
                        return Err(e);
                    }
                    self.expected_volume = Some(normalized);
                    self.state = SessionState::Triggered;
                    self.event_log.append(TriggerEvent {
                        timestamp_ms: hit.timestamp_ms,
                        session_id: self.session_id.clone(),
                        policy_name: None,
                        keyword: hit.keyword,
                        confidence: hit.confidence,
                    });
                    let _ = duration_seconds; // TODO: schedule reset
                }
                VolumeDecision::Renew => {
                    // TODO: extend the active volume duration without changing
                    // the volume level.
                }
                VolumeDecision::Default => {
                    // TODO: apply default action (usually mute) when no policy
                    // matches and the active duration has expired.
                }
            }
        }

        Ok(())
    }

    /// Restore the original volume and stop capture.
    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.audio_stream = None;
        self.volume.unmute(self.target_pid)?;
        self.state = SessionState::Idle;
        self.expected_volume = None;
        Ok(())
    }

    /// Restore volume after an internal error.  Errors are logged but not
    /// propagated, because we are already in an error path.
    fn restore_volume_safely(&self, reason: &str) {
        if let Err(e) = self.volume.unmute(self.target_pid) {
            tracing::error!(
                session_id = %self.session_id,
                reason,
                "failed to restore volume: {e}"
            );
        } else {
            tracing::warn!(
                session_id = %self.session_id,
                reason,
                "volume restored due to error"
            );
        }
    }
}

impl Drop for FocusSession {
    fn drop(&mut self) {
        // Best-effort restore on normal destruction.  This does not run on
        // SIGKILL; a platform-specific guardian is needed for that case.
        if self.audio_stream.is_some() {
            let _ = self.volume.unmute(self.target_pid);
        }
    }
}
