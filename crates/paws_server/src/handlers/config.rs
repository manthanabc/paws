//! Configuration and resource HTTP handlers (read-only for UI).

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use paws_domain::{AgentId, ModelId, ProviderId};
use serde::Deserialize;

use crate::server::AppState;
use crate::AppError;

// =============================================================================
// Health & Environment
// =============================================================================

/// Health check endpoint.
pub async fn health() -> &'static str {
    "OK"
}

/// Gets environment information.
///
/// GET /api/env
pub async fn get_env(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    Ok(Json(state.api.environment()))
}

// =============================================================================
// Resources (Read-Only)
// =============================================================================

/// Lists available tools.
///
/// GET /api/tools
pub async fn list_tools(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let tools = state.api.get_tools().await?;
    Ok(Json(tools))
}

/// Query parameters for listing tools by agent.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct ListToolsQuery {
    pub agent_id: Option<AgentId>,
}

/// Lists available models for a specific provider.
///
/// GET /api/providers/:id/models
pub async fn list_provider_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let provider_id = ProviderId::from(id);
    let provider = state.api.get_provider(&provider_id).await?;
    // Return models from provider if available, otherwise return empty list
    // The AnyProvider type doesn't expose models directly, so we return
    // the provider info which includes model details
    Ok(Json(provider))
}

/// Lists available models.
///
/// GET /api/models
pub async fn list_models(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let models = state.api.get_models().await?;
    Ok(Json(models))
}

/// Lists available agents.
///
/// GET /api/agents
pub async fn list_agents(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let agents = state.api.get_agents().await?;
    Ok(Json(agents))
}

/// Lists available providers.
///
/// GET /api/providers
pub async fn list_providers(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let providers = state.api.get_providers().await?;
    Ok(Json(providers))
}

/// Gets a specific provider.
///
/// GET /api/providers/:id
pub async fn get_provider(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let provider = state.api.get_provider(&ProviderId::from(id)).await?;
    Ok(Json(provider))
}

/// Lists available skills.
///
/// GET /api/skills
pub async fn list_skills(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let skills = state.api.get_skills().await?;
    Ok(Json(skills))
}

/// Lists available custom commands.
///
/// GET /api/commands
pub async fn list_commands(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let commands = state.api.get_commands().await?;
    Ok(Json(commands))
}

/// Gets the workflow configuration.
///
/// GET /api/workflow
pub async fn get_workflow(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let workflow = state.api.read_merged(None).await?;
    Ok(Json(workflow))
}

// =============================================================================
// Configuration
// =============================================================================

/// Gets the default provider.
///
/// GET /api/config/default-provider
pub async fn get_default_provider(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let provider = state.api.get_default_provider().await?;
    Ok(Json(provider))
}

/// Request to set the default provider.
#[derive(Debug, Deserialize)]
pub struct SetProviderRequest {
    pub provider_id: ProviderId,
}

/// Sets the default provider.
///
/// POST /api/config/default-provider
pub async fn set_default_provider(
    State(state): State<AppState>,
    Json(request): Json<SetProviderRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.api.set_default_provider(request.provider_id).await?;
    Ok(StatusCode::OK)
}

/// Gets the default model.
///
/// GET /api/config/default-model
pub async fn get_default_model(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let model_id = state.api.get_default_model().await;
    Ok(Json(model_id))
}

/// Request to set the default model.
#[derive(Debug, Deserialize)]
pub struct SetModelRequest {
    pub model_id: ModelId,
}

/// Sets the default model.
///
/// POST /api/config/default-model
pub async fn set_default_model(
    State(state): State<AppState>,
    Json(request): Json<SetModelRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.api.set_default_model(request.model_id).await?;
    Ok(StatusCode::OK)
}

/// Gets the active agent.
///
/// GET /api/config/active-agent
pub async fn get_active_agent(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let agent_id = state.api.get_active_agent().await;
    Ok(Json(agent_id))
}

/// Request to set the active agent.
#[derive(Debug, Deserialize)]
pub struct SetAgentRequest {
    pub agent_id: AgentId,
}

/// Sets the active agent.
///
/// POST /api/config/active-agent
pub async fn set_active_agent(
    State(state): State<AppState>,
    Json(request): Json<SetAgentRequest>,
) -> Result<impl IntoResponse, AppError> {
    state.api.set_active_agent(request.agent_id).await?;
    Ok(StatusCode::OK)
}

// =============================================================================
// MCP Configuration
// =============================================================================

/// Query parameters for MCP config.
#[derive(Debug, Deserialize)]
pub struct McpConfigQuery {
    pub scope: Option<paws_domain::Scope>,
}

/// Gets MCP configuration.
///
/// GET /api/mcp/config
pub async fn get_mcp_config(
    State(state): State<AppState>,
    Query(query): Query<McpConfigQuery>,
) -> Result<impl IntoResponse, AppError> {
    let config = state.api.read_mcp_config(query.scope.as_ref()).await?;
    Ok(Json(config))
}

/// Request to write MCP config.
#[derive(Debug, Deserialize)]
pub struct WriteMcpConfigRequest {
    pub scope: paws_domain::Scope,
    pub config: paws_domain::McpConfig,
}

/// Writes MCP configuration.
///
/// POST /api/mcp/config
pub async fn write_mcp_config(
    State(state): State<AppState>,
    Json(request): Json<WriteMcpConfigRequest>,
) -> Result<impl IntoResponse, AppError> {
    state
        .api
        .write_mcp_config(&request.scope, &request.config)
        .await?;
    Ok(StatusCode::OK)
}

/// Reloads MCP servers.
///
/// POST /api/mcp/reload
pub async fn reload_mcp(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    state.api.reload_mcp().await?;
    Ok(StatusCode::OK)
}

// =============================================================================
// Authentication
// =============================================================================

/// Request to initiate provider authentication.
#[derive(Debug, Deserialize)]
pub struct InitAuthRequest {
    pub provider_id: ProviderId,
    pub method: paws_domain::AuthMethod,
}

/// Initiates provider authentication.
///
/// POST /api/auth/init
pub async fn init_auth(
    State(state): State<AppState>,
    Json(request): Json<InitAuthRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = state
        .api
        .init_provider_auth(request.provider_id, request.method)
        .await?;
    Ok(Json(context))
}

/// Request to complete authentication.
#[derive(Debug, Deserialize)]
pub struct CompleteAuthRequest {
    pub provider_id: ProviderId,
    pub context: paws_domain::AuthContextResponse,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

fn default_timeout() -> u64 {
    60
}

/// Completes provider authentication.
///
/// POST /api/auth/complete
pub async fn complete_auth(
    State(state): State<AppState>,
    Json(request): Json<CompleteAuthRequest>,
) -> Result<impl IntoResponse, AppError> {
    let timeout = std::time::Duration::from_secs(request.timeout_secs);
    state
        .api
        .complete_provider_auth(request.provider_id, request.context, timeout)
        .await?;
    Ok(StatusCode::OK)
}

/// Request to logout.
#[derive(Debug, Deserialize)]
pub struct LogoutRequest {
    pub provider_id: Option<ProviderId>,
}

/// Logs out from a provider or all providers.
///
/// POST /api/auth/logout
pub async fn logout(
    State(state): State<AppState>,
    Json(request): Json<LogoutRequest>,
) -> Result<impl IntoResponse, AppError> {
    if let Some(provider_id) = request.provider_id {
        state.api.remove_provider(&provider_id).await?;
    } else {
        state.api.logout().await?;
    }
    Ok(StatusCode::OK)
}

/// Gets user information.
///
/// GET /api/auth/user
pub async fn get_user_info(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let info = state.api.user_info().await?;
    Ok(Json(info))
}

/// Gets user usage statistics.
///
/// GET /api/auth/usage
pub async fn get_user_usage(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let usage = state.api.user_usage().await?;
    Ok(Json(usage))
}

// =============================================================================
// Platform Authentication
// =============================================================================

/// Initiates platform login.
///
/// POST /api/platform/auth/init
pub async fn platform_init_login(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let auth = state.api.init_login().await?;
    Ok(Json(auth))
}

/// Completes platform login.
///
/// POST /api/platform/auth/login
pub async fn platform_login(
    State(state): State<AppState>,
    Json(auth): Json<paws_domain::InitAuth>,
) -> Result<impl IntoResponse, AppError> {
    state.api.login(&auth).await?;
    Ok(StatusCode::OK)
}

/// Gets platform user info.
///
/// GET /api/platform/auth/info
pub async fn platform_user_info(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let info = state.api.get_login_info().await?;
    Ok(Json(info))
}
