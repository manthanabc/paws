//! Task manager for spawning and tracking background tasks.

use std::sync::Arc;

use paws_api::API;
use paws_domain::{AgentId, ChatRequest, Conversation, ConversationId, Event, EventValue};
use tracing::{error, info};

use super::store::{Task, TaskId, TaskStore};
use crate::events::{EventBroadcaster, TaskEvent};

/// Manages task lifecycle and background execution.
pub struct TaskManager {
    store: Arc<TaskStore>,
    broadcaster: Arc<EventBroadcaster>,
    api: Arc<dyn API>,
}

impl TaskManager {
    /// Creates a new task manager.
    pub fn new(
        store: Arc<TaskStore>,
        broadcaster: Arc<EventBroadcaster>,
        api: Arc<dyn API>,
    ) -> Self {
        Self { store, broadcaster, api }
    }

    /// Submits a new task for execution.
    ///
    /// This creates the task, stores it, and spawns background execution.
    /// Returns the task ID for tracking.
    pub async fn submit(
        &self,
        conversation_id: ConversationId,
        message: String,
        agent_id: Option<AgentId>,
        attachments: Vec<paws_domain::Attachment>,
    ) -> anyhow::Result<TaskId> {
        // Resolve agent ID
        let agent_id = match agent_id {
            Some(id) => id,
            None => self
                .api
                .get_active_agent()
                .await
                .ok_or_else(|| anyhow::anyhow!("No active agent configured"))?,
        };

        // Ensure conversation exists
        self.ensure_conversation(&conversation_id).await?;

        // Create task
        // Use the first 100 chars of message as title
        let title = if message.len() > 100 {
            format!("{}...", &message[0..100])
        } else {
            message.clone()
        };

        let task = Task::new(conversation_id, agent_id.clone(), title);
        let task_id = task.id;

        // Store task
        self.store.insert_task(task.clone()).await;

        // Create event for the chat request
        let event = Event {
            id: uuid::Uuid::new_v4().to_string(),
            value: Some(EventValue::Text(message.into())),
            timestamp: chrono::Utc::now().to_rfc3339(),
            attachments,
            additional_context: None,
        };

        // Ensure broadcast channel exists before spawning execution.
        // This creates the channel if it doesn't exist, so events won't be lost
        // when the SSE handler subscribes later.
        let _ensure_channel = self.broadcaster.ensure_channel(task_id).await;

        // Spawn background execution
        self.spawn_execution(task_id, conversation_id, agent_id, event);

        info!(task_id = %task_id, conversation_id = %conversation_id, "Task submitted");

        Ok(task_id)
    }

    /// Ensures the conversation exists, creating it if necessary.
    async fn ensure_conversation(&self, conversation_id: &ConversationId) -> anyhow::Result<()> {
        let existing = self.api.conversation(conversation_id).await?;
        if existing.is_none() {
            let conversation = Conversation::new(*conversation_id);
            self.api.upsert_conversation(conversation).await?;
        }
        Ok(())
    }

    /// Spawns the background task execution.
    fn spawn_execution(
        &self,
        task_id: TaskId,
        conversation_id: ConversationId,
        _agent_id: AgentId,
        event: Event,
    ) {
        let store = self.store.clone();
        let broadcaster = self.broadcaster.clone();
        let api = self.api.clone();

        tokio::spawn(async move {
            // Mark task as running
            if let Some(mut task) = store.get_task(task_id).await {
                task.start();
                store.update_task(task).await;
            }

            // Emit started event
            let start_event = TaskEvent::started();
            store.append_event(task_id, start_event.clone()).await;
            broadcaster.broadcast(task_id, start_event).await;

            // Create chat request
            let chat_request = ChatRequest { event, conversation_id };

            // Execute chat
            match api.chat(chat_request).await {
                Ok(mut stream) => {
                    let mut has_error = false;
                    let mut last_error = None;

                    while let Some(result) = futures::StreamExt::next(&mut stream).await {
                        match result {
                            Ok(response) => {
                                // Check for completion
                                let is_complete =
                                    matches!(response, paws_domain::ChatResponse::TaskComplete);

                                // Broadcast the response
                                let event = TaskEvent::message(response);
                                store.append_event(task_id, event.clone()).await;
                                broadcaster.broadcast(task_id, event).await;

                                if is_complete {
                                    break;
                                }
                            }
                            Err(e) => {
                                error!(task_id = %task_id, error = %e, "Stream error");
                                has_error = true;
                                last_error = Some(e.to_string());
                                let event = TaskEvent::error(e.to_string());
                                store.append_event(task_id, event.clone()).await;
                                broadcaster.broadcast(task_id, event).await;
                            }
                        }
                    }

                    // Mark task as completed or failed based on whether errors occurred
                    if let Some(mut task) = store.get_task(task_id).await {
                        if has_error {
                            // Safety: has_error is only set to true when last_error is Some
                            let error_msg = last_error.unwrap();
                            task.fail(error_msg.clone());
                            store.update_task(task.clone()).await;
                            let event = TaskEvent::failed(error_msg);
                            store.append_event(task_id, event.clone()).await;
                            broadcaster.broadcast(task_id, event).await;
                            error!(task_id = %task_id, "Task failed");
                        } else {
                            task.complete();
                            store.update_task(task.clone()).await;
                            let event = TaskEvent::completed();
                            store.append_event(task_id, event.clone()).await;
                            broadcaster.broadcast(task_id, event).await;
                            info!(task_id = %task_id, "Task completed");
                        }
                    }
                }
                Err(e) => {
                    error!(task_id = %task_id, error = %e, "Failed to start chat");

                    // Mark task as failed
                    if let Some(mut task) = store.get_task(task_id).await {
                        task.fail(e.to_string());
                        store.update_task(task.clone()).await;
                        let event = TaskEvent::failed(e.to_string());
                        store.append_event(task_id, event.clone()).await;
                        broadcaster.broadcast(task_id, event).await;
                    }
                }
            }
        });
    }

    /// Gets a task by ID.
    pub async fn get_task(&self, id: TaskId) -> Option<Task> {
        self.store.get_task(id).await
    }

    /// Lists tasks, optionally filtered by conversation.
    pub async fn list_tasks(&self, conversation_id: Option<ConversationId>) -> Vec<Task> {
        self.store.list_tasks(conversation_id).await
    }

    /// Gets events for a task.
    pub async fn get_events(&self, task_id: TaskId) -> Vec<TaskEvent> {
        self.store.get_events(task_id).await
    }

    /// Gets events since a specific index (for reconnection).
    pub async fn get_events_since(&self, task_id: TaskId, since_index: usize) -> Vec<TaskEvent> {
        self.store.get_events_since(task_id, since_index).await
    }

    /// Cancels a running task.
    pub async fn cancel(&self, id: TaskId) -> anyhow::Result<()> {
        if let Some(mut task) = self.store.get_task(id).await {
            if task.status.is_terminal() {
                return Err(anyhow::anyhow!("Task already completed"));
            }

            task.cancel();
            self.store.update_task(task).await;
            let event = TaskEvent::cancelled();
            self.store.append_event(id, event.clone()).await;
            self.broadcaster.broadcast(id, event).await;
            info!(task_id = %id, "Task cancelled");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_id_generation() {
        let id1 = TaskId::new();
        let id2 = TaskId::new();
        assert_ne!(id1, id2);
    }
}
