use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Result;
use paws_domain::{Step, StepInput, Task};
use tokio::sync::RwLock;

pub struct AgentProtocolService {
    tasks: Arc<RwLock<HashMap<String, Task>>>,
    steps: Arc<RwLock<HashMap<String, Vec<Step>>>>,
}

impl AgentProtocolService {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            steps: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn create_task(&self, input: String) -> Task {
        let task = Task::new(input);
        let mut tasks = self.tasks.write().await;
        tasks.insert(task.task_id.clone(), task.clone());
        task
    }

    pub async fn list_tasks(&self) -> Vec<Task> {
        let tasks = self.tasks.read().await;
        tasks.values().cloned().collect()
    }

    pub async fn get_task(&self, task_id: &str) -> Option<Task> {
        let tasks = self.tasks.read().await;
        tasks.get(task_id).cloned()
    }

    pub async fn list_steps(&self, task_id: &str) -> Vec<Step> {
        let steps = self.steps.read().await;
        steps.get(task_id).cloned().unwrap_or_default()
    }

    pub async fn create_step(&self, task_id: &str, input: StepInput) -> Result<Step> {
        let mut steps_guard = self.steps.write().await;
        let task_steps = steps_guard.entry(task_id.to_string()).or_default();

        let step = Step::new(task_id.to_string(), input.input, true);
        task_steps.push(step.clone());

        Ok(step)
    }

    pub async fn get_step(&self, task_id: &str, step_id: &str) -> Option<Step> {
        let steps = self.steps.read().await;
        steps
            .get(task_id)
            .and_then(|steps| steps.iter().find(|s| s.step_id == step_id).cloned())
    }
}

impl Default for AgentProtocolService {
    fn default() -> Self {
        Self::new()
    }
}
