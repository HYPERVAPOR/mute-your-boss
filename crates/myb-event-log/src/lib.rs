use chrono::{DateTime, Utc};
use myb_core::{EventLog as EventLogTrait, TriggerEvent};
use serde::{Deserialize, Serialize};

/// A single trigger event persisted for later inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub policy_name: String,
    pub keyword: String,
    pub confidence: f64,
}

/// Event log backend.
///
/// The default implementation stores events in memory. Persistent backends
/// (JSONL, SQLite) can be added later without changing consumers.
#[derive(Debug, Default, Clone)]
pub struct EventLog {
    events: Vec<EventLogEntry>,
}

impl EventLog {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn entries(&self, limit: usize) -> Vec<EventLogEntry> {
        self.events
            .iter()
            .rev()
            .take(limit)
            .rev()
            .cloned()
            .collect()
    }
}

impl EventLogTrait for EventLog {
    fn append(&mut self, event: TriggerEvent) {
        self.events.push(EventLogEntry {
            timestamp: DateTime::from_timestamp_millis(event.timestamp_ms).unwrap_or_else(Utc::now),
            session_id: event.session_id,
            policy_name: event.policy_name.unwrap_or_default(),
            keyword: event.keyword,
            confidence: event.confidence,
        });
    }

    fn recent(&self, limit: usize) -> Vec<TriggerEvent> {
        self.events
            .iter()
            .rev()
            .take(limit)
            .rev()
            .map(|e| TriggerEvent {
                timestamp_ms: e.timestamp.timestamp_millis(),
                session_id: e.session_id.clone(),
                policy_name: Some(e.policy_name.clone()),
                keyword: e.keyword.clone(),
                confidence: e.confidence,
            })
            .collect()
    }

    fn clear(&mut self) {
        self.events.clear();
    }
}
