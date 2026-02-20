//! Task management for background orchestration.

mod manager;
mod store;

pub use manager::TaskManager;
pub use store::{Task, TaskId, TaskStatus, TaskStore};
