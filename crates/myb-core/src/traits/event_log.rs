use std::fmt;

/// A persisted trigger event.
#[derive(Debug, Clone)]
pub struct TriggerEvent {
    pub timestamp_ms: i64,
    pub session_id: String,
    pub policy_name: Option<String>,
    pub keyword: String,
    pub confidence: f64,
}

/// Event log interface.
pub trait EventLog: Send + Sync {
    /// Append a new event.
    fn append(&mut self, event: TriggerEvent);

    /// Return the most recent `limit` events in chronological order.
    fn recent(&self, limit: usize) -> Vec<TriggerEvent>;

    /// Clear all stored events.
    fn clear(&mut self);
}

impl fmt::Debug for dyn EventLog + Send + Sync {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EventLog").finish()
    }
}
