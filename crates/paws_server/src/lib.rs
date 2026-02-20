//! Task-based server architecture for Paws.
//!
//! This module implements a server that runs the orchestration loop
//! server-side, allowing the frontend to be stateless and reconnectable.

mod error;
mod events;
mod handlers;
mod server;
mod task;

pub use error::{AppError, ErrorResponse};
pub use events::{EventBroadcaster, EventLog, TaskEvent};
pub use server::Server;
pub use task::{Task, TaskId, TaskManager, TaskStatus};
