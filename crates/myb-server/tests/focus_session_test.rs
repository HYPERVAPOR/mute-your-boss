use myb_audio_capture::mock::MockAudioCapture;
use myb_core::{
    AudioChunk, EventLog, FocusSession, KwsEngine, KwsHit, SessionState, VolumeController,
};
use myb_event_log::EventLog as EventLogImpl;
use myb_policy::PolicyEngine as PolicyEngineImpl;
use myb_volume_control::mock::MockVolumeController;

/// A KWS engine that returns predefined hits for the first N chunks.
struct ScriptedKwsEngine {
    hits_per_chunk: Vec<Vec<KwsHit>>,
    index: usize,
}

impl ScriptedKwsEngine {
    fn new(hits_per_chunk: Vec<Vec<KwsHit>>) -> Self {
        Self {
            hits_per_chunk,
            index: 0,
        }
    }
}

impl KwsEngine for ScriptedKwsEngine {
    fn process_chunk(
        &mut self,
        _samples: &[f32],
        timestamp_ms: i64,
    ) -> anyhow::Result<Vec<KwsHit>> {
        if self.index < self.hits_per_chunk.len() {
            let hits: Vec<_> = self.hits_per_chunk[self.index]
                .iter()
                .map(|h| KwsHit {
                    keyword: h.keyword.clone(),
                    confidence: h.confidence,
                    timestamp_ms,
                })
                .collect();
            self.index += 1;
            Ok(hits)
        } else {
            Ok(vec![])
        }
    }
}

fn build_session(
    chunks: Vec<AudioChunk>,
    hits: Vec<Vec<KwsHit>>,
    policy_yaml: &str,
) -> anyhow::Result<(FocusSession, MockVolumeController, EventLogImpl)> {
    let audio = MockAudioCapture::new(vec![], chunks);
    let kws = ScriptedKwsEngine::new(hits);
    let policy = PolicyEngineImpl::from_yaml(policy_yaml)?;
    let volume = MockVolumeController::new();
    let event_log = EventLogImpl::new();

    let session = FocusSession::new(
        "test-session".into(),
        1234,
        Box::new(audio),
        Box::new(kws),
        Box::new(policy),
        Box::new(volume.clone()),
        Box::new(event_log.clone()),
    )?;

    Ok((session, volume, event_log))
}

#[test]
fn keyword_hit_changes_volume_and_logs_event() {
    let yaml = r#"
policies:
  - name: unmute-on-keyword
    match:
      keywords: ["LIGHT_UP"]
      threshold: 0.2
    action:
      volume: 80
      duration_seconds: 2
      then: mute
"#;

    let chunks = vec![AudioChunk {
        samples: vec![0.0; 160], // 10ms of silence at 16kHz
        timestamp_ms: 100,
    }];
    let hits = vec![vec![KwsHit {
        keyword: "LIGHT_UP".into(),
        confidence: 0.9,
        timestamp_ms: 100,
    }]];

    let (mut session, volume, event_log) = build_session(chunks, hits, yaml).unwrap();
    session.start().unwrap();
    session.tick().unwrap();

    assert_eq!(
        volume.get_volume(1234).unwrap(),
        0.8,
        "volume should be set to 80%"
    );
    assert_eq!(event_log.count(), 1, "one event should be logged");
}

#[test]
fn no_hit_keeps_volume_unchanged() {
    let yaml = r#"
policies:
  - name: unmute-on-keyword
    match:
      keywords: ["LIGHT_UP"]
      threshold: 0.2
    action:
      volume: 80
      duration_seconds: 2
      then: mute
"#;

    let chunks = vec![AudioChunk {
        samples: vec![0.0; 160],
        timestamp_ms: 100,
    }];
    let hits = vec![vec![]];

    let (mut session, volume, _event_log) = build_session(chunks, hits, yaml).unwrap();
    session.start().unwrap();
    session.tick().unwrap();

    assert_eq!(
        volume.get_volume(1234).unwrap(),
        1.0,
        "default volume should remain 100%"
    );
}

#[test]
fn external_volume_change_pauses_automatic_control() {
    let yaml = r#"
policies:
  - name: unmute-on-keyword
    match:
      keywords: ["LIGHT_UP"]
      threshold: 0.2
    action:
      volume: 80
      duration_seconds: 2
      then: mute
"#;

    let chunks = vec![
        AudioChunk {
            samples: vec![0.0; 160],
            timestamp_ms: 100,
        },
        AudioChunk {
            samples: vec![0.0; 160],
            timestamp_ms: 200,
        },
        AudioChunk {
            samples: vec![0.0; 160],
            timestamp_ms: 300,
        },
    ];
    let hits = vec![
        vec![KwsHit {
            keyword: "LIGHT_UP".into(),
            confidence: 0.9,
            timestamp_ms: 100,
        }],
        vec![],
        vec![KwsHit {
            keyword: "LIGHT_UP".into(),
            confidence: 0.9,
            timestamp_ms: 300,
        }],
    ];

    let (mut session, volume, _event_log) = build_session(chunks, hits, yaml).unwrap();
    session.start().unwrap();

    // First tick triggers the policy and sets volume to 80%.
    session.tick().unwrap();
    assert_eq!(volume.get_volume(1234).unwrap(), 0.8);
    assert!(matches!(session.state(), SessionState::Triggered));

    // Simulate the user changing volume manually outside the app.
    volume.set_volume(1234, 0.3).unwrap();

    // Second tick detects the external change and pauses.
    session.tick().unwrap();
    assert!(matches!(session.state(), SessionState::Paused));

    // Third tick has another hit, but automatic control stays paused.
    session.tick().unwrap();
    assert_eq!(
        volume.get_volume(1234).unwrap(),
        0.3,
        "volume should not be overwritten while paused"
    );
}

#[test]
fn drop_restores_volume() {
    let yaml = r#"
policies:
  - name: unmute-on-keyword
    match:
      keywords: ["LIGHT_UP"]
      threshold: 0.2
    action:
      volume: 80
      duration_seconds: 2
      then: mute
"#;

    let chunks = vec![AudioChunk {
        samples: vec![0.0; 160],
        timestamp_ms: 100,
    }];
    let hits = vec![vec![KwsHit {
        keyword: "LIGHT_UP".into(),
        confidence: 0.9,
        timestamp_ms: 100,
    }]];

    let (mut session, volume, _event_log) = build_session(chunks, hits, yaml).unwrap();
    session.start().unwrap();
    session.tick().unwrap();
    assert_eq!(volume.get_volume(1234).unwrap(), 0.8);

    drop(session);
    assert_eq!(
        volume.get_volume(1234).unwrap(),
        1.0,
        "volume should be restored on drop"
    );
}
