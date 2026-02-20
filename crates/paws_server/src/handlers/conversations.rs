//! Conversation-related HTTP handlers.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use paws_domain::{Conversation, ConversationId};
use serde::{Deserialize, Serialize};

use crate::AppError;
use crate::server::AppState;

/// Query parameters for listing conversations.
#[derive(Debug, Deserialize)]
pub struct ListConversationsQuery {
    pub limit: Option<usize>,
}

/// Lists all conversations.
///
/// GET /api/conversations
pub async fn list_conversations(
    State(state): State<AppState>,
    Query(query): Query<ListConversationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let conversations = state.api.get_conversations(query.limit).await?;
    Ok(Json(conversations))
}

/// Lists conversation summaries (lightweight, no context).
///
/// GET /api/conversations/summaries
pub async fn list_conversation_summaries(
    State(state): State<AppState>,
    Query(query): Query<ListConversationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let summaries = state.api.get_conversation_summaries(query.limit).await?;
    Ok(Json(summaries))
}

/// Request body for creating a new conversation.
#[derive(Debug, Deserialize)]
pub struct CreateConversationRequest {
    pub id: ConversationId,
    #[serde(default)]
    pub title: Option<String>,
}

/// Response for conversation creation.
#[derive(Debug, Serialize)]
pub struct CreateConversationResponse {
    pub id: ConversationId,
    pub title: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Creates a new conversation.
///
/// POST /api/conversations
pub async fn create_conversation(
    State(state): State<AppState>,
    Json(request): Json<CreateConversationRequest>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!(
        conversation_id = %request.id,
        title = ?request.title,
        "Creating conversation"
    );

    let conversation = Conversation::new(request.id).title(request.title.clone());
    let created_at = conversation.metadata.created_at;
    let id = conversation.id;

    state.api.upsert_conversation(conversation).await?;

    let response = CreateConversationResponse { id, title: request.title, created_at };

    Ok((StatusCode::CREATED, Json(response)))
}

/// Gets a specific conversation.
///
/// GET /api/conversations/:id
pub async fn get_conversation(
    State(state): State<AppState>,
    Path(id): Path<ConversationId>,
) -> Result<Response, AppError> {
    let conversation = state.api.conversation(&id).await?;
    match conversation {
        Some(c) => Ok(Json(c).into_response()),
        None => Err(AppError::not_found(format!(
            "Conversation not found: {}",
            id
        ))),
    }
}

/// Updates a conversation.
///
/// PUT /api/conversations/:id
pub async fn update_conversation(
    State(state): State<AppState>,
    Path(id): Path<ConversationId>,
    Json(conversation): Json<Conversation>,
) -> Result<impl IntoResponse, AppError> {
    if id != conversation.id {
        return Err(AppError::bad_request(
            "Conversation ID in path does not match body",
        ));
    }

    tracing::info!(
        conversation_id = %conversation.id,
        title = ?conversation.title,
        "Updating conversation"
    );

    state.api.upsert_conversation(conversation).await?;
    Ok(StatusCode::OK)
}

/// Deletes a conversation.
///
/// DELETE /api/conversations/:id
pub async fn delete_conversation(
    State(state): State<AppState>,
    Path(id): Path<ConversationId>,
) -> Result<impl IntoResponse, AppError> {
    state.api.delete_conversation(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Compacts a conversation to reduce token usage.
///
/// POST /api/conversations/:id/compact
pub async fn compact_conversation(
    State(state): State<AppState>,
    Path(id): Path<ConversationId>,
) -> Result<impl IntoResponse, AppError> {
    let result = state.api.compact_conversation(&id).await?;
    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn test_create_conversation_request_deserialization() {
        let json = r#"{"id": "550e8400-e29b-41d4-a716-446655440000"}"#;
        let actual: CreateConversationRequest = serde_json::from_str(json).unwrap();
        assert!(actual.title.is_none());
    }

    #[test]
    fn test_create_conversation_request_with_title() {
        let json = r#"{"id": "550e8400-e29b-41d4-a716-446655440000", "title": "My Chat"}"#;
        let actual: CreateConversationRequest = serde_json::from_str(json).unwrap();
        assert_eq!(actual.title, Some("My Chat".to_string()));
    }
}
