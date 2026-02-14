//! Event broadcasting and storage for real-time updates.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use paws_domain::ChatResponse;
use serde::{Deserialize, Serialize};
use tokio::sync::{RwLock, broadcast};

use crate::task::TaskId;

/// Events emitted during task execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskEvent {
    /// Task started processing.
    Started { timestamp: DateTime<Utc> },
    /// Agent message/response chunk.
    Message {
        content: ChatResponse,
        #[serde(skip_serializing_if = "Option::is_none")]
        sequence: Option<usize>,
    },
    /// Tool execution notification (info only).
    ToolExecution {
        tool: String,
        status: ToolExecutionStatus,
        timestamp: DateTime<Utc>,
    },
    /// Transient error (non-fatal).
    Error {
        message: String,
        timestamp: DateTime<Utc>,
    },
    /// Task completed successfully.
    Completed { timestamp: DateTime<Utc> },
    /// Task failed with error.
    Failed {
        error: String,
        timestamp: DateTime<Utc>,
    },
    /// Task was cancelled.
    Cancelled { timestamp: DateTime<Utc> },
}

impl TaskEvent {
    /// Creates a new Started event.
    pub fn started() -> Self {
        Self::Started { timestamp: Utc::now() }
    }

    /// Creates a new Message event.
    pub fn message(content: ChatResponse) -> Self {
        Self::Message { content, sequence: None }
    }

    /// Creates a new ToolExecution event.
    pub fn tool_execution(tool: String, status: ToolExecutionStatus) -> Self {
        Self::ToolExecution { tool, status, timestamp: Utc::now() }
    }

    /// Creates a new Error event.
    pub fn error(message: String) -> Self {
        Self::Error { message, timestamp: Utc::now() }
    }

    /// Creates a new Completed event.
    pub fn completed() -> Self {
        Self::Completed { timestamp: Utc::now() }
    }

    /// Creates a new Failed event.
    pub fn failed(error: String) -> Self {
        Self::Failed { error, timestamp: Utc::now() }
    }

    /// Creates a new Cancelled event.
    pub fn cancelled() -> Self {
        Self::Cancelled { timestamp: Utc::now() }
    }
}

/// Status of a tool execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolExecutionStatus {
    Started,
    Completed,
    Failed,
}

/// Handle for receiving task events.
pub type EventReceiver = broadcast::Receiver<TaskEvent>;

/// Broadcasts events to multiple subscribers.
///
/// Uses a broadcast channel per task for efficient fan-out to multiple
/// SSE clients.
#[derive(Debug)]
pub struct EventBroadcaster {
    /// Broadcast channels per task.
    channels: RwLock<HashMap<TaskId, broadcast::Sender<TaskEvent>>>,
    /// Channel capacity for broadcast channels.
    capacity: usize,
}

impl EventBroadcaster {
    /// Creates a new event broadcaster.
    pub fn new() -> Self {
        Self { channels: RwLock::new(HashMap::new()), capacity: 256 }
    }

    /// Creates a broadcaster with custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self { channels: RwLock::new(HashMap::new()), capacity }
    }

    /// Subscribes to events for a task.
    ///
    /// Creates a new broadcast channel if one doesn't exist.
    pub async fn subscribe(&self, task_id: TaskId) -> EventReceiver {
        let mut channels = self.channels.write().await;

        let sender = channels
            .entry(task_id)
            .or_insert_with(|| broadcast::channel(self.capacity).0);

        sender.subscribe()
    }

    /// Ensures a broadcast channel exists for a task.
    ///
    /// This is useful to create the channel before any events are broadcast,
    /// so that subsequent subscribers can receive events from the buffer.
    pub async fn ensure_channel(&self, task_id: TaskId) {
        let mut channels = self.channels.write().await;
        channels
            .entry(task_id)
            .or_insert_with(|| broadcast::channel(self.capacity).0);
    }

    /// Broadcasts an event to all subscribers of a task.
    pub async fn broadcast(&self, task_id: TaskId, event: TaskEvent) {
        // Broadcast to subscribers
        let channels = self.channels.read().await;
        if let Some(sender) = channels.get(&task_id) {
            // Ignore send errors (no subscribers)
            let _ = sender.send(event);
        }
    }

    /// Cleans up resources for a completed task.
    pub async fn cleanup(&self, task_id: TaskId) {
        self.channels.write().await.remove(&task_id);
    }
}

impl Default for EventBroadcaster {
    fn default() -> Self {
        Self::new()
    }
}

/// Persistent log of events for reconnection support.
#[derive(Debug, Default)]
pub struct EventLog {
    events: RwLock<HashMap<TaskId, Vec<TaskEvent>>>,
}

impl EventLog {
    /// Creates a new empty event log.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends an event to the log.
    pub async fn append(&self, task_id: TaskId, event: TaskEvent) {
        self.events
            .write()
            .await
            .entry(task_id)
            .or_default()
            .push(event);
    }
}

#[cfg(test)]
mod tests {
    

    use super::*;

    #[test]
    fn test_task_event_serialization() {
        let fixture = TaskEvent::started();
        let json = serde_json::to_string(&fixture).unwrap();
        assert!(json.contains("started"));
    }

    #[tokio::test]
    async fn test_event_log() {
        let log = EventLog::new();
        let task_id = TaskId::new();

        log.append(task_id, TaskEvent::started()).await;
        // The rest of this test relied on methods we just removed
        // since EventLog is now just a helper struct if used at all
        // or we can remove EventLog entirely if it's not used by Broadcaster
        // anymore.
    }
}
