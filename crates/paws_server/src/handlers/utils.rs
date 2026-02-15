//! Shared utilities for HTTP handlers.

use crate::AppError;
use crate::task::TaskId;

/// Parses and validates a task ID from a string.
///
/// # Errors
///
/// Returns an error if the task ID is "undefined", empty, or not a valid UUID.
pub fn parse_task_id(id: &str) -> Result<TaskId, AppError> {
    // Validate task ID before parsing
    if id == "undefined" || id.is_empty() {
        return Err(AppError::bad_request(
            "Invalid task ID: task ID is undefined or empty. Please create a task first using POST /api/tasks",
        ));
    }

    id.parse().map_err(|e: uuid::Error| {
        AppError::bad_request(format!(
            "Invalid task ID '{}': {}. Task ID must be a valid UUID. Please create a task first using POST /api/tasks",
            id, e
        ))
    })
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_parse_task_id_valid() {
        let fixture = "550e8400-e29b-41d4-a716-446655440000";
        let actual = parse_task_id(fixture);
        assert!(actual.is_ok());
    }

    #[test]
    fn test_parse_task_id_undefined() {
        let fixture = "undefined";
        let actual = parse_task_id(fixture);
        assert!(actual.is_err());
    }

    #[test]
    fn test_parse_task_id_empty() {
        let fixture = "";
        let actual = parse_task_id(fixture);
        assert!(actual.is_err());
    }

    #[test]
    fn test_parse_task_id_invalid_uuid() {
        let fixture = "not-a-uuid";
        let actual = parse_task_id(fixture);
        assert!(actual.is_err());
    }
}
