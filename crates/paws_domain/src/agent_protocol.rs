use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub task_id: String,
    pub input: String,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskInput {
    pub input: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub task_id: String,
    pub step_id: String,
    pub name: Option<String>,
    pub input: Option<String>,
    pub output: Option<String>,
    pub status: StepStatus,
    pub is_last: bool,
    pub artifacts: Vec<Artifact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Created,
    Running,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepInput {
    pub name: Option<String>,
    pub input: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    pub artifact_id: String,
    pub file_name: String,
    pub relative_path: Option<String>,
}

impl Task {
    pub fn new(input: String) -> Self {
        Self {
            task_id: Uuid::new_v4().to_string(),
            input,
            artifacts: Vec::new(),
        }
    }
}

impl Step {
    pub fn new(task_id: String, input: Option<String>, is_last: bool) -> Self {
        Self {
            task_id,
            step_id: Uuid::new_v4().to_string(),
            name: None,
            input,
            output: None,
            status: StepStatus::Created,
            is_last,
            artifacts: Vec::new(),
        }
    }
}
