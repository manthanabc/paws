//! Task domain types and storage.

use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use paws_domain::{AgentId, ConversationId};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::events::TaskEvent;

/// Unique identifier for a task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TaskId(pub Uuid);

impl TaskId {
    /// Generates a new random task ID.
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for TaskId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for TaskId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(TaskId)
    }
}

/// Status of a task in its lifecycle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is queued but not yet started.
    Pending,
    /// Task is currently being processed.
    Running { started_at: DateTime<Utc> },
    /// Task completed successfully.
    Completed {
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
    },
    /// Task failed with an error.
    Failed {
        started_at: DateTime<Utc>,
        completed_at: DateTime<Utc>,
        error: String,
    },
    /// Task was cancelled by user request.
    Cancelled {
        started_at: Option<DateTime<Utc>>,
        completed_at: DateTime<Utc>,
    },
}

impl TaskStatus {
    /// Checks if the task is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed { .. } | TaskStatus::Failed { .. } | TaskStatus::Cancelled { .. }
        )
    }

    /// Checks if the task is currently running.
    pub fn is_running(&self) -> bool {
        matches!(self, TaskStatus::Running { .. })
    }
}

/// A task represents a unit of work submitted to the server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Unique task identifier.
    pub id: TaskId,
    /// Conversation this task belongs to.
    pub conversation_id: ConversationId,
    /// Current status of the task.
    pub status: TaskStatus,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// Agent assigned to process this task.
    pub agent_id: AgentId,
    /// Title or summary of the task (usually the initial message).
    pub title: String,
}

impl Task {
    /// Creates a new pending task.
    pub fn new(conversation_id: ConversationId, agent_id: AgentId, title: String) -> Self {
        Self {
            id: TaskId::new(),
            conversation_id,
            status: TaskStatus::Pending,
            created_at: Utc::now(),
            agent_id,
            title,
        }
    }

    /// Marks the task as running.
    pub fn start(&mut self) {
        self.status = TaskStatus::Running { started_at: Utc::now() };
    }

    /// Marks the task as completed.
    pub fn complete(&mut self) {
        if let TaskStatus::Running { started_at } = self.status {
            self.status = TaskStatus::Completed { started_at, completed_at: Utc::now() };
        }
    }

    /// Marks the task as failed.
    pub fn fail(&mut self, error: String) {
        let started_at = match &self.status {
            TaskStatus::Running { started_at } => *started_at,
            _ => Utc::now(),
        };
        self.status = TaskStatus::Failed { started_at, completed_at: Utc::now(), error };
    }

    /// Marks the task as cancelled.
    pub fn cancel(&mut self) {
        let started_at = match &self.status {
            TaskStatus::Running { started_at } => Some(*started_at),
            _ => None,
        };
        self.status = TaskStatus::Cancelled { started_at, completed_at: Utc::now() };
    }
}

/// In-memory store for tasks and their events.
#[derive(Debug, Default)]
pub struct TaskStore {
    tasks: RwLock<HashMap<TaskId, Task>>,
    events: RwLock<HashMap<TaskId, Vec<TaskEvent>>>,
}

impl TaskStore {
    /// Creates a new empty task store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Stores a new task.
    pub async fn insert_task(&self, task: Task) {
        self.tasks.write().await.insert(task.id, task);
    }

    /// Retrieves a task by ID.
    pub async fn get_task(&self, id: TaskId) -> Option<Task> {
        self.tasks.read().await.get(&id).cloned()
    }

    /// Updates a task.
    pub async fn update_task(&self, task: Task) {
        self.tasks.write().await.insert(task.id, task);
    }

    /// Lists all tasks, optionally filtered by conversation.
    pub async fn list_tasks(&self, conversation_id: Option<ConversationId>) -> Vec<Task> {
        let tasks = self.tasks.read().await;
        tasks
            .values()
            .filter(|t| {
                conversation_id
                    .map(|cid| t.conversation_id == cid)
                    .unwrap_or(true)
            })
            .cloned()
            .collect()
    }

    /// Appends an event to a task's event log.
    pub async fn append_event(&self, task_id: TaskId, event: TaskEvent) {
        self.events
            .write()
            .await
            .entry(task_id)
            .or_default()
            .push(event);
    }

    /// Retrieves all events for a task.
    pub async fn get_events(&self, task_id: TaskId) -> Vec<TaskEvent> {
        self.events
            .read()
            .await
            .get(&task_id)
            .cloned()
            .unwrap_or_default()
    }

    /// Gets events since a specific index (for reconnection).
    pub async fn get_events_since(&self, task_id: TaskId, since_index: usize) -> Vec<TaskEvent> {
        self.events
            .read()
            .await
            .get(&task_id)
            .map(|events| events.iter().skip(since_index).cloned().collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_task_lifecycle() {
        let conversation_id = ConversationId::default();
        let agent_id = AgentId::new("test-agent");
        let title = "Test task".to_string();

        let mut fixture = Task::new(conversation_id, agent_id, title);
        assert!(matches!(fixture.status, TaskStatus::Pending));

        fixture.start();
        assert!(fixture.status.is_running());

        fixture.complete();
        assert!(fixture.status.is_terminal());
    }

    #[test]
    fn test_task_failure() {
        let conversation_id = ConversationId::default();
        let agent_id = AgentId::new("test-agent");
        let title = "Test task".to_string();

        let mut fixture = Task::new(conversation_id, agent_id, title);
        fixture.start();
        fixture.fail("Something went wrong".to_string());

        let actual = &fixture.status;
        assert!(
            matches!(actual, TaskStatus::Failed { error, .. } if error == "Something went wrong")
        );
    }

    #[tokio::test]
    async fn test_task_store() {
        let store = TaskStore::new();
        let conversation_id = ConversationId::default();
        let agent_id = AgentId::new("test-agent");
        let title = "Test task".to_string();
        let task = Task::new(conversation_id, agent_id, title);
        let task_id = task.id;

        store.insert_task(task.clone()).await;
        let actual = store.get_task(task_id).await;
        assert_eq!(actual.unwrap().id, task_id);
    }
}
