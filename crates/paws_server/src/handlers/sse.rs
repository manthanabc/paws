//! Server-Sent Events streaming handlers.

use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::response::sse::{Event, KeepAlive, Sse};
use futures::stream::StreamExt;
use tokio_stream::wrappers::BroadcastStream;

use crate::AppError;
use crate::server::AppState;
use crate::task::TaskId;
use super::parse_task_id;

/// Streams task events via Server-Sent Events.
///
/// GET /api/tasks/{id}/stream
///
/// This endpoint supports reconnection. Clients can reconnect and receive
/// missed events by using the `/api/tasks/{id}/events/since` endpoint first.
pub async fn stream_task_events(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let task_id = parse_task_id(&id)?;

    // Verify task exists
    let task = state
        .task_store
        .get_task(task_id)
        .await
        .ok_or_else(|| AppError::not_found(format!("Task not found: {}. Please verify the task ID or create a new task using POST /api/tasks", task_id)))?;

    // If task is already complete, return final events
    if task.status.is_terminal() {
        let events = state.task_store.get_events(task_id).await;
        let stream = futures::stream::iter(
            events
                .into_iter()
                .filter_map(|e| Some(Ok::<_, axum::Error>(Event::default().json_data(e).ok()?))),
        );
        return Ok(Sse::new(stream)
            .keep_alive(KeepAlive::default())
            .into_response());
    }

    // Get any events that were already stored (e.g., started event)
    let stored_events = state.task_store.get_events(task_id).await;

    // Subscribe to live events
    let receiver = state.broadcaster.subscribe(task_id).await;
    let live_stream = BroadcastStream::new(receiver);

    // First yield stored events, then live events
    let stored_event_stream = futures::stream::iter(
        stored_events
            .into_iter()
            .filter_map(|e| Some(Ok::<_, axum::Error>(Event::default().json_data(e).ok()?))),
    );

    let live_sse_stream = live_stream.filter_map(|result| async move {
        match result {
            Ok(event) => {
                let json = serde_json::to_string(&event).ok()?;
                Some(Ok::<_, axum::Error>(Event::default().data(json)))
            }
            Err(e) => {
                tracing::warn!("SSE stream error: {}", e);
                None
            }
        }
    });

    let combined_stream = stored_event_stream.chain(live_sse_stream);

    Ok(Sse::new(combined_stream)
        .keep_alive(KeepAlive::default())
        .into_response())
}

/// Query parameters for resumable streaming.
#[derive(Debug, serde::Deserialize)]
pub struct StreamSinceQuery {
    /// Resume from this event index.
    pub since: Option<usize>,
}

/// Streams task events with reconnection support.
///
/// GET /api/tasks/{id}/stream/resumable
///
/// First sends any missed events since `since` index, then streams live events.
pub async fn stream_task_events_resumable(
    State(state): State<AppState>,
    Path(id): Path<String>,
    axum::extract::Query(query): axum::extract::Query<StreamSinceQuery>,
) -> Result<impl IntoResponse, AppError> {
    let task_id = parse_task_id(&id)?;

    let since = query.since.unwrap_or(0);

    // Get missed events first
    let missed_events = state.task_store.get_events_since(task_id, since).await;

    // Subscribe to live events
    let receiver = state.broadcaster.subscribe(task_id).await;
    let live_stream = BroadcastStream::new(receiver);

    // First yield missed events, then live events
    let missed_stream = futures::stream::iter(
        missed_events
            .into_iter()
            .filter_map(|e| Some(Ok::<_, axum::Error>(Event::default().json_data(e).ok()?))),
    );

    let live_sse_stream = live_stream.filter_map(|result| async move {
        match result {
            Ok(event) => {
                let json = serde_json::to_string(&event).ok()?;
                Some(Ok::<_, axum::Error>(Event::default().data(json)))
            }
            Err(e) => {
                tracing::warn!("SSE stream error: {}", e);
                None
            }
        }
    });

    let combined_stream = missed_stream.chain(live_sse_stream);

    Ok(Sse::new(combined_stream).keep_alive(KeepAlive::default()))
}
