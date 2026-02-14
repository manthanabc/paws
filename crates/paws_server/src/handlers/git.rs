//! Git related handlers.

use std::path::PathBuf;

use axum::Json;
use axum::extract::State;
use axum::response::IntoResponse;
use serde::Serialize;

use crate::AppError;
use crate::server::AppState;

/// Response for git status/diff.
#[derive(Debug, Serialize)]
pub struct GitDiffResponse {
    pub diff: String,
}

/// Gets the current git diff.
///
/// GET /api/git/diff
pub async fn get_git_diff(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    // We'll execute "git diff" and "git diff --staged" to get all changes.
    // For now, let's just get "git diff HEAD" to see everything against the last
    // commit.

    // Note: We are assuming the current working directory is the git root or inside
    // it. In a real agent scenario, we might need to know the workspace path.
    // For this MVP, we use the current directory.

    let output = state
        .api
        .execute_shell_command("git diff HEAD", PathBuf::from("."))
        .await?;

    Ok(Json(GitDiffResponse { diff: output.stdout }))
}

/// Response for git status (simplified).
#[derive(Debug, Serialize)]
pub struct GitStatusResponse {
    pub status: String,
}

/// Gets the current git status.
///
/// GET /api/git/status
pub async fn get_git_status(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let output = state
        .api
        .execute_shell_command("git status --porcelain", PathBuf::from("."))
        .await?;

    Ok(Json(GitStatusResponse { status: output.stdout }))
}

use serde::Deserialize;

/// Request for committing changes.
#[derive(Debug, Deserialize)]
pub struct CommitRequest {
    pub message: String,
}

/// Commits changes to git.
///
/// POST /api/git/commit
pub async fn commit_changes(
    State(state): State<AppState>,
    Json(request): Json<CommitRequest>,
) -> Result<impl IntoResponse, AppError> {
    // First add all changes
    state
        .api
        .execute_shell_command("git add .", PathBuf::from("."))
        .await?;

    // Then commit
    // Escape quotes in message to prevent shell injection/breaking
    let escaped_message = request.message.replace('"', "\\\"");
    let cmd = format!("git commit -m \"{}\"", escaped_message);

    let output = state
        .api
        .execute_shell_command(&cmd, PathBuf::from("."))
        .await?;

    Ok(Json(GitStatusResponse { status: output.stdout }))
}
