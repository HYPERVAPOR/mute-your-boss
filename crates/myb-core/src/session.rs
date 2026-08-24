use crate::traits::{
    AudioCapture, AudioStream, EventLog, KwsEngine, PolicyEngine, TriggerEvent, VolumeController,
    VolumeDecision,
};

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
}

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
        })
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn target_pid(&self) -> u32 {
        self.target_pid
    }

    /// Start capturing audio from the target process.
    pub fn start(&mut self) -> anyhow::Result<()> {
        self.audio_stream = Some(self.audio.start_capture(self.target_pid)?);
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

        let hits = self.kws.process_chunk(&chunk.samples, chunk.timestamp_ms)?;
        for hit in hits {
            let decision = self.policy.evaluate(&hit.keyword, hit.confidence);
            match decision {
                VolumeDecision::SetVolume {
                    volume,
                    duration_seconds,
                } => {
                    let normalized = volume.clamp(0, 100) as f32 / 100.0;
                    self.volume.set_volume(self.target_pid, normalized)?;
                    self.event_log.append(TriggerEvent {
                        timestamp_ms: hit.timestamp_ms,
                        session_id: self.session_id.clone(),
                        policy_name: None,
                        keyword: hit.keyword,
                        confidence: hit.confidence,
                    });
                    let _ = duration_seconds; // TODO: schedule reset
                }
                VolumeDecision::Default => {
                    // TODO: apply default action (usually mute)
                }
            }
        }

        Ok(())
    }

    /// Restore the original volume and stop capture.
    pub fn stop(&mut self) -> anyhow::Result<()> {
        self.audio_stream = None;
        self.volume.unmute(self.target_pid)?;
        Ok(())
    }
}
