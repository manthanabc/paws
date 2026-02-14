//! File related HTTP handlers.

use axum::Json;
use axum::extract::{Query, State};
use axum::response::IntoResponse;
use serde::Deserialize;

use crate::AppError;
use crate::server::AppState;

/// Query parameters for reading a file.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ReadFileQuery {
    pub path: Option<String>,
}

/// Lists files in a directory.
///
/// GET /api/files
pub async fn list_files(
    State(state): State<AppState>,
    Query(query): Query<ReadFileQuery>,
) -> Result<impl IntoResponse, AppError> {
    let path = query.path.map(std::path::PathBuf::from);
    let files = state.api.discover().await?;
    // Filter by path if provided
    let filtered = if let Some(ref path) = path {
        let path_str = path.to_string_lossy().to_string();
        files
            .into_iter()
            .filter(|f| f.path.starts_with(&path_str))
            .collect()
    } else {
        files
    };
    Ok(Json(filtered))
}

// Note: File read/write operations are handled internally by the orchestrator
// and are not exposed via the API for security reasons.
