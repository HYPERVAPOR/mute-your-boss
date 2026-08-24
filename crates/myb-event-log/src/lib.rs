use chrono::{DateTime, Utc};
use myb_core::{EventLog as EventLogTrait, TriggerEvent};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// A single trigger event persisted for later inspection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: String,
    pub policy_name: String,
    pub keyword: String,
    pub confidence: f64,
}

impl EventLogEntry {
    fn from_trigger(event: TriggerEvent) -> Self {
        Self {
            timestamp: DateTime::from_timestamp_millis(event.timestamp_ms).unwrap_or_else(Utc::now),
            session_id: event.session_id,
            policy_name: event.policy_name.unwrap_or_default(),
            keyword: event.keyword,
            confidence: event.confidence,
        }
    }

    fn to_trigger(&self) -> TriggerEvent {
        TriggerEvent {
            timestamp_ms: self.timestamp.timestamp_millis(),
            session_id: self.session_id.clone(),
            policy_name: Some(self.policy_name.clone()),
            keyword: self.keyword.clone(),
            confidence: self.confidence,
        }
    }
}

/// Event log backend.
///
/// The default implementation stores events in memory. Persistent backends
/// (JSONL, SQLite) can be added later without changing consumers.
///
/// Clones share the same underlying storage, so an event log can be passed to
/// multiple consumers (e.g. a session and a gRPC status handler) without
/// losing writes.
#[derive(Debug, Clone)]
pub struct EventLog {
    events: Arc<Mutex<Vec<EventLogEntry>>>,
    path: Option<PathBuf>,
}

impl EventLog {
    /// Create an in-memory event log.
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(Vec::new())),
            path: None,
        }
    }

    /// Create an event log that persists to a JSONL file.
    ///
    /// Existing entries are loaded from the file on creation.
    pub fn new_jsonl<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let events = if path.exists() {
            Self::load_from_file(&path)?
        } else {
            Vec::new()
        };
        Ok(Self {
            events: Arc::new(Mutex::new(events)),
            path: Some(path),
        })
    }

    /// Return a view of the raw entries (newest last).
    pub fn entries(&self, limit: usize) -> Vec<EventLogEntry> {
        let events = self.events.lock().unwrap();
        events.iter().rev().take(limit).rev().cloned().collect()
    }

    fn load_from_file(path: &Path) -> anyhow::Result<Vec<EventLogEntry>> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str::<EventLogEntry>(&line) {
                Ok(entry) => events.push(entry),
                Err(e) => {
                    tracing::warn!("skipping malformed event log line: {e}");
                }
            }
        }
        Ok(events)
    }

    fn append_to_file(&self, entry: &EventLogEntry) -> anyhow::Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(path)?;
        let line = serde_json::to_string(entry)?;
        writeln!(file, "{line}")?;
        Ok(())
    }
}

impl Default for EventLog {
    fn default() -> Self {
        Self::new()
    }
}

impl EventLogTrait for EventLog {
    fn append(&mut self, event: TriggerEvent) {
        let entry = EventLogEntry::from_trigger(event);
        if let Err(e) = self.append_to_file(&entry) {
            tracing::error!("failed to persist event log entry: {e}");
        }
        let mut events = self.events.lock().unwrap();
        events.push(entry);
    }

    fn recent(&self, limit: usize) -> Vec<TriggerEvent> {
        let events = self.events.lock().unwrap();
        events
            .iter()
            .rev()
            .take(limit)
            .rev()
            .map(|e| e.to_trigger())
            .collect()
    }

    fn query(
        &self,
        session_id: Option<&str>,
        since_ms: Option<i64>,
        limit: usize,
    ) -> Vec<TriggerEvent> {
        let events = self.events.lock().unwrap();
        let filtered: Vec<_> = events
            .iter()
            .filter(|e| session_id.is_none_or(|sid| e.session_id == sid))
            .filter(|e| since_ms.is_none_or(|since| e.timestamp.timestamp_millis() >= since))
            .collect();
        filtered
            .into_iter()
            .rev()
            .take(limit)
            .rev()
            .map(|e| e.to_trigger())
            .collect()
    }

    fn clear(&mut self) {
        let mut events = self.events.lock().unwrap();
        events.clear();
        if let Some(path) = &self.path {
            if let Err(e) = std::fs::remove_file(path) {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::error!("failed to remove event log file: {e}");
                }
            }
        }
    }

    fn count(&self) -> usize {
        let events = self.events.lock().unwrap();
        events.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_event(ts: i64, sid: &str, keyword: &str) -> TriggerEvent {
        TriggerEvent {
            timestamp_ms: ts,
            session_id: sid.into(),
            policy_name: Some("p1".into()),
            keyword: keyword.into(),
            confidence: 0.9,
        }
    }

    #[test]
    fn memory_append_and_recent() {
        let mut log = EventLog::new();
        log.append(sample_event(1000, "s1", "A"));
        log.append(sample_event(2000, "s1", "B"));
        assert_eq!(log.count(), 2);
        let recent = log.recent(1);
        assert_eq!(recent.len(), 1);
        assert_eq!(recent[0].keyword, "B");
    }

    #[test]
    fn query_by_session_and_since() {
        let mut log = EventLog::new();
        log.append(sample_event(1000, "s1", "A"));
        log.append(sample_event(2000, "s2", "B"));
        log.append(sample_event(3000, "s1", "C"));

        let hits = log.query(Some("s1"), None, 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].keyword, "A");
        assert_eq!(hits[1].keyword, "C");

        let hits = log.query(None, Some(1500), 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].keyword, "B");
    }

    #[test]
    fn jsonl_persistence_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("events.jsonl");

        {
            let mut log = EventLog::new_jsonl(&path).unwrap();
            log.append(sample_event(1000, "s1", "A"));
            log.append(sample_event(2000, "s1", "B"));
        }

        let mut log = EventLog::new_jsonl(&path).unwrap();
        assert_eq!(log.count(), 2);
        let recent = log.recent(2);
        assert_eq!(recent[0].keyword, "A");
        assert_eq!(recent[1].keyword, "B");

        log.clear();
        assert!(!path.exists());
    }
}
