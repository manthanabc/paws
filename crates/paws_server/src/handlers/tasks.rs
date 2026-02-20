//! Task-related HTTP handlers.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use paws_domain::ConversationId;
use serde::{Deserialize, Serialize};

use super::parse_task_id;
use crate::AppError;
use crate::server::AppState;
use crate::task::TaskId;

/// Request to create a new task.
#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    /// Conversation to add the message to.
    pub conversation_id: ConversationId,
    /// The user's message.
    pub message: String,
    /// Optional agent to use.
    #[serde(default)]
    pub agent_id: Option<paws_domain::AgentId>,
    /// Optional file attachments.
    #[serde(default)]
    pub attachments: Vec<paws_domain::Attachment>,
}

/// Response for task creation.
#[derive(Debug, Serialize)]
pub struct CreateTaskResponse {
    pub task_id: TaskId,
    pub conversation_id: ConversationId,
}

/// Submits a new task for execution.
///
/// POST /api/tasks
pub async fn create_task(
    State(state): State<AppState>,
    Json(request): Json<CreateTaskRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(
        conversation_id = %request.conversation_id,
        agent_id = ?request.agent_id,
        "Creating task"
    );

    let task_id = state
        .task_manager
        .submit(
            request.conversation_id,
            request.message,
            request.agent_id,
            request.attachments,
        )
        .await?;

    let response = CreateTaskResponse { task_id, conversation_id: request.conversation_id };

    Ok((StatusCode::ACCEPTED, Json(response)))
}

/// Query parameters for listing tasks.
#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    /// Filter by conversation.
    pub conversation_id: Option<ConversationId>,
}

/// Lists tasks, optionally filtered by conversation.
///
/// GET /api/tasks
pub async fn list_tasks(
    State(state): State<AppState>,
    axum::extract::Query(query): axum::extract::Query<ListTasksQuery>,
) -> Result<impl IntoResponse, AppError> {
    let tasks = state.task_manager.list_tasks(query.conversation_id).await;
    Ok(Json(tasks))
}

/// Gets a specific task by ID.
///
/// GET /api/tasks/{id}
pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let task_id = parse_task_id(&id)?;

    let task = state
        .task_manager
        .get_task(task_id)
        .await
        .ok_or_else(|| AppError::not_found(format!("Task not found: {}. Please verify the task ID or create a new task using POST /api/tasks", task_id)))?;

    Ok(Json(task))
}

/// Cancels a running task.
///
/// POST /api/tasks/{id}/cancel
pub async fn cancel_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let task_id = parse_task_id(&id)?;

    state.task_manager.cancel(task_id).await?;
    Ok(StatusCode::OK)
}

/// Gets all events for a task (for reconnection).
///
/// GET /api/tasks/{id}/events
pub async fn get_task_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let task_id = parse_task_id(&id)?;

    let events = state.task_manager.get_events(task_id).await;
    Ok(Json(events))
}

/// Query parameters for getting events since an index.
#[derive(Debug, Deserialize)]
pub struct EventsSinceQuery {
    /// Get events starting from this index.
    pub since: Option<usize>,
}

/// Gets events for a task since a specific index.
///
/// GET /api/tasks/{id}/events/since
pub async fn get_task_events_since(
    State(state): State<AppState>,
    Path(id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<EventsSinceQuery>,
) -> Result<impl IntoResponse, AppError> {
    let task_id = parse_task_id(&id)?;

    let since = query.since.unwrap_or(0);
    let events = state.task_manager.get_events_since(task_id, since).await;
    Ok(Json(events))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_create_task_request_deserialization() {
        let json = r#"{
            "conversation_id": "550e8400-e29b-41d4-a716-446655440000",
            "message": "Hello, world!"
        }"#;

        let actual: CreateTaskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(actual.message, "Hello, world!");
        assert!(actual.agent_id.is_none());
        assert!(actual.attachments.is_empty());
    }

    #[test]
    fn test_create_task_request_with_agent() {
        let json = r#"{
            "conversation_id": "550e8400-e29b-41d4-a716-446655440000",
            "message": "Hello!",
            "agent_id": "paws"
        }"#;

        let actual: CreateTaskRequest = serde_json::from_str(json).unwrap();
        assert_eq!(actual.agent_id, Some(paws_domain::AgentId::new("paws")));
    }
}
