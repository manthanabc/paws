use std::sync::Arc;

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use paws_app::AgentProtocolService;
use paws_domain::{Step, StepInput, Task, TaskInput};
use tower_http::cors::CorsLayer;

pub struct AgentProtocolServer {
    service: Arc<AgentProtocolService>,
}

impl AgentProtocolServer {
    pub fn new() -> Self {
        Self { service: Arc::new(AgentProtocolService::new()) }
    }

    pub async fn serve(&self, host: &str, port: u16) -> anyhow::Result<()> {
        let app = Router::new()
            .route("/agent/tasks", post(create_task).get(list_tasks))
            .route("/agent/tasks/:task_id", get(get_task))
            .route(
                "/agent/tasks/:task_id/steps",
                post(execute_step).get(list_steps),
            )
            .route("/agent/tasks/:task_id/steps/:step_id", get(get_step))
            .layer(CorsLayer::permissive())
            .with_state(self.service.clone());

        let listener = tokio::net::TcpListener::bind(format!("{}:{}", host, port)).await?;
        tracing::info!("Agent Protocol server listening on {}:{}", host, port);
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn create_task(
    State(service): State<Arc<AgentProtocolService>>,
    Json(input): Json<TaskInput>,
) -> Json<Task> {
    Json(service.create_task(input.input).await)
}

async fn list_tasks(State(service): State<Arc<AgentProtocolService>>) -> Json<Vec<Task>> {
    Json(service.list_tasks().await)
}

async fn get_task(
    State(service): State<Arc<AgentProtocolService>>,
    Path(task_id): Path<String>,
) -> Result<Json<Task>, axum::http::StatusCode> {
    service
        .get_task(&task_id)
        .await
        .map(Json)
        .ok_or(axum::http::StatusCode::NOT_FOUND)
}

async fn list_steps(
    State(service): State<Arc<AgentProtocolService>>,
    Path(task_id): Path<String>,
) -> Json<Vec<Step>> {
    Json(service.list_steps(&task_id).await)
}

async fn execute_step(
    State(service): State<Arc<AgentProtocolService>>,
    Path(task_id): Path<String>,
    Json(input): Json<StepInput>,
) -> Result<Json<Step>, axum::http::StatusCode> {
    service
        .create_step(&task_id, input)
        .await
        .map(Json)
        .map_err(|_| axum::http::StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_step(
    State(service): State<Arc<AgentProtocolService>>,
    Path((task_id, step_id)): Path<(String, String)>,
) -> Result<Json<Step>, axum::http::StatusCode> {
    service
        .get_step(&task_id, &step_id)
        .await
        .map(Json)
        .ok_or(axum::http::StatusCode::NOT_FOUND)
}

impl Default for AgentProtocolServer {
    fn default() -> Self {
        Self::new()
    }
}
