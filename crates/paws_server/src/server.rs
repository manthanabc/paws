//! HTTP server setup and routing.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Router,
};
use paws_api::API;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

use crate::events::EventBroadcaster;
use crate::handlers::*;
use crate::task::{TaskManager, TaskStore};

/// Combined application state shared across all handlers.
#[derive(Clone)]
pub struct AppState {
    /// API instance for backend operations.
    pub api: Arc<dyn API>,
    /// Task manager for background orchestration.
    pub task_manager: Arc<TaskManager>,
    /// Task store for persistence.
    pub task_store: Arc<TaskStore>,
    /// Event broadcaster for SSE streaming.
    pub broadcaster: Arc<EventBroadcaster>,
}

/// HTTP server for the Paws API.
pub struct Server {
    api: Arc<dyn API>,
    port: u16,
}

impl Server {
    /// Creates a new server instance.
    pub fn new(api: Arc<dyn API>, port: u16) -> Self {
        Self { api, port }
    }

    /// Runs the HTTP server.
    pub async fn run(self) -> anyhow::Result<()> {
        tracing::info!("Initializing server components...");

        // Initialize shared components
        let task_store = Arc::new(TaskStore::new());
        let broadcaster = Arc::new(EventBroadcaster::new());
        let task_manager = Arc::new(TaskManager::new(
            task_store.clone(),
            broadcaster.clone(),
            self.api.clone(),
        ));

        // Create unified state
        let state = Arc::new(AppState {
            api: self.api.clone(),
            task_manager,
            task_store,
            broadcaster,
        });

        // Build router with all routes
        let app = Router::new()
            // Health & Environment
            .route("/api/health", get(health))
            .route("/api/env", get(get_env))
            // Resources (Read-Only)
            .route("/api/files", get(list_files))
            .route("/api/tools", get(list_tools))
            .route("/api/models", get(list_models))
            .route("/api/agents", get(list_agents))
            .route("/api/providers", get(list_providers))
            .route("/api/providers/:id", get(get_provider))
            .route("/api/providers/:id/models", get(list_provider_models))
            .route("/api/skills", get(list_skills))
            .route("/api/commands", get(list_commands))
            .route("/api/workflow", get(get_workflow))
            // Tasks (Core API)
            .route("/api/tasks", get(list_tasks).post(create_task))
            .route("/api/tasks/:id", get(get_task))
            .route("/api/tasks/:id/cancel", post(cancel_task))
            .route("/api/tasks/:id/events", get(get_task_events))
            .route("/api/tasks/:id/events/since", get(get_task_events_since))
            .route("/api/tasks/:id/stream", get(stream_task_events))
            .route("/api/tasks/:id/stream/resumable", get(stream_task_events_resumable))
            // Git
            .route("/api/git/diff", get(get_git_diff))
            .route("/api/git/status", get(get_git_status))
            .route("/api/git/commit", post(commit_changes))
            // Conversations
            .route("/api/conversations", get(list_conversations).post(create_conversation))
            .route("/api/conversations/summaries", get(list_conversation_summaries))
            .route("/api/conversations/:id", get(get_conversation).delete(delete_conversation).put(update_conversation))
            .route("/api/conversations/:id/compact", post(compact_conversation))
            // Configuration
            .route("/api/config/default-provider", get(get_default_provider).post(set_default_provider))
            .route("/api/config/default-model", get(get_default_model).post(set_default_model))
            .route("/api/config/active-agent", get(get_active_agent).post(set_active_agent))
            // MCP Configuration
            .route("/api/mcp/config", get(get_mcp_config).post(write_mcp_config))
            .route("/api/mcp/reload", post(reload_mcp))
            // Provider Authentication
            .route("/api/auth/init", post(init_auth))
            .route("/api/auth/complete", post(complete_auth))
            .route("/api/auth/logout", post(logout))
            .route("/api/auth/user", get(get_user_info))
            .route("/api/auth/usage", get(get_user_usage))
            // Platform Authentication
            .route("/api/platform/auth/init", post(platform_init_login))
            .route("/api/platform/auth/login", post(platform_login))
            .route("/api/platform/auth/info", get(platform_user_info))
            // Middleware
            .layer(
                CorsLayer::new()
                    .allow_origin(Any)
                    .allow_methods([axum::http::Method::GET, axum::http::Method::POST, axum::http::Method::PUT, axum::http::Method::DELETE, axum::http::Method::OPTIONS])
                    .allow_headers(Any)
                    .allow_credentials(false)
            )
            .layer(TraceLayer::new_for_http())
            .with_state((*state).clone());

        let addr = SocketAddr::from(([0, 0, 0, 0], self.port));
        tracing::info!("Server listening on {}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        tracing::info!("Server started successfully, accepting connections...");
        axum::serve(listener, app).await?;

        Ok(())
    }
}
