//! Pure-mock end-to-end test of the FocusSession pipeline.
//!
//! This test does not require a KWS model or real audio.  It wires a fake
//! keyword engine to a fake audio stream and verifies that a keyword hit
//! propagates through the policy engine to the volume controller and event log.

use myb_audio_capture::mock::MockAudioCapture;
use myb_core::traits::audio::{AudioChunk, AudioProcessInfo};
use myb_core::traits::kws::{KwsEngine, KwsHit};
use myb_core::traits::volume::VolumeController;
use myb_core::{EventLog, FocusSession, SessionState};
use myb_event_log::EventLog as JsonlEventLog;
use myb_policy::PolicyEngine as YamlPolicyEngine;
use myb_volume_control::mock::MockVolumeController;
use std::sync::atomic::{AtomicUsize, Ordering};

/// A fake keyword engine that emits a hit on the N-th chunk.
struct FakeKwsEngine {
    trigger_on: usize,
    seen: AtomicUsize,
    keyword: String,
}

impl FakeKwsEngine {
    fn new(trigger_on: usize, keyword: impl Into<String>) -> Self {
        Self {
            trigger_on,
            seen: AtomicUsize::new(0),
            keyword: keyword.into(),
        }
    }
}

impl KwsEngine for FakeKwsEngine {
    fn process_chunk(
        &mut self,
        _samples: &[f32],
        timestamp_ms: i64,
    ) -> anyhow::Result<Vec<KwsHit>> {
        let n = self.seen.fetch_add(1, Ordering::SeqCst);
        if n == self.trigger_on {
            Ok(vec![KwsHit {
                keyword: self.keyword.clone(),
                confidence: 1.0,
                timestamp_ms,
            }])
        } else {
            Ok(vec![])
        }
    }
}

fn silence_chunk(timestamp_ms: i64, samples: usize) -> AudioChunk {
    AudioChunk {
        timestamp_ms,
        samples: vec![0.0f32; samples],
    }
}

fn build_policy() -> YamlPolicyEngine {
    let yaml = r#"
policies:
  - name: unmute-on-keyword
    match:
      keywords:
        - "UNMUTE=AH0 N M Y UW1 T"
      threshold: 0.25
    action:
      volume: 100
      duration_seconds: 5
      then: mute
"#;
    YamlPolicyEngine::from_yaml(yaml).unwrap()
}

#[test]
fn keyword_hit_unmutes_target_and_logs_event() {
    // 16 kHz / 1-second chunks.
    let chunk_samples = 16_000;
    let chunks = vec![
        silence_chunk(0, chunk_samples),
        silence_chunk(1_000, chunk_samples),
        silence_chunk(2_000, chunk_samples),
    ];

    let audio = MockAudioCapture::new(
        vec![AudioProcessInfo {
            pid: 1234,
            name: "mock-meeting".into(),
            window_title: None,
            current_volume: 0.0,
            is_meeting_app: true,
        }],
        chunks,
    );

    let kws = Box::new(FakeKwsEngine::new(1, "UNMUTE"));
    let policy = Box::new(build_policy());
    let volume = Box::new(MockVolumeController::new());
    volume.set_volume(1234, 0.0).unwrap(); // simulate meeting app already muted
    let event_log = Box::new(JsonlEventLog::new());

    let mut session = FocusSession::new(
        "test-session".into(),
        1234,
        Box::new(audio),
        kws,
        policy,
        volume.clone(),
        event_log.clone(),
    )
    .unwrap();

    session.start().unwrap();
    assert_eq!(session.state(), SessionState::Listening);

    // Tick 1: no hit, volume stays muted.
    session.tick().unwrap();
    assert_eq!(session.state(), SessionState::Listening);
    assert!(
        volume.get_volume(1234).unwrap() < 0.01,
        "volume should still be muted before the keyword"
    );

    // Tick 2: fake keyword hit at chunk index 1.
    session.tick().unwrap();
    assert_eq!(session.state(), SessionState::Triggered);
    assert!(
        (volume.get_volume(1234).unwrap() - 1.0).abs() < 0.01,
        "volume should be restored to 100% after the keyword"
    );

    // Tick 3: no new hit; external volume matches expectation, so stay triggered.
    session.tick().unwrap();
    assert_eq!(session.state(), SessionState::Triggered);

    // Event log should contain one trigger.
    let events = event_log.recent(10);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].keyword, "UNMUTE");
    assert_eq!(events[0].session_id, "test-session");

    session.stop().unwrap();
    assert_eq!(session.state(), SessionState::Idle);
}

#[test]
fn low_confidence_hit_is_ignored() {
    struct LowConfidenceEngine;
    impl KwsEngine for LowConfidenceEngine {
        fn process_chunk(
            &mut self,
            _samples: &[f32],
            _timestamp_ms: i64,
        ) -> anyhow::Result<Vec<KwsHit>> {
            Ok(vec![KwsHit {
                keyword: "UNMUTE".into(),
                confidence: 0.1, // below threshold 0.25
                timestamp_ms: 0,
            }])
        }
    }

    let chunk = silence_chunk(0, 16_000);
    let audio = MockAudioCapture::new(vec![], vec![chunk]);
    let volume = Box::new(MockVolumeController::new());
    volume.set_volume(42, 0.0).unwrap();
    let event_log = Box::new(JsonlEventLog::new());

    let mut session = FocusSession::new(
        "low-conf".into(),
        42,
        Box::new(audio),
        Box::new(LowConfidenceEngine),
        Box::new(build_policy()),
        volume.clone(),
        event_log.clone(),
    )
    .unwrap();

    session.start().unwrap();
    session.tick().unwrap();

    assert_eq!(session.state(), SessionState::Listening);
    assert!(
        volume.get_volume(42).unwrap() < 0.01,
        "low-confidence hit should not change volume"
    );
    assert!(event_log.recent(10).is_empty());
}
