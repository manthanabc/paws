//! Error handling for the HTTP API.

use axum::{http::StatusCode, response::{IntoResponse, Response}, Json};
use serde::Serialize;

/// Standard error response format.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

/// Application error type with proper HTTP status codes.
#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
    pub details: Option<String>,
}

impl AppError {
    /// Creates a new error with the given status and message.
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
            details: None,
        }
    }

    /// Creates a bad request error (400).
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    /// Creates a not found error (404).
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    /// Creates an internal server error (500).
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }

    /// Creates a conflict error (409).
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    /// Adds additional details to the error.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = ErrorResponse {
            error: self.message,
            details: self.details,
        };
        (self.status, Json(body)).into_response()
    }
}

impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        let err = err.into();
        Self::internal(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_error_response_serialization() {
        let fixture = ErrorResponse {
            error: "Something went wrong".to_string(),
            details: None,
        };
        let actual = serde_json::to_string(&fixture).unwrap();
        assert!(actual.contains("Something went wrong"));
        assert!(!actual.contains("details"));
    }

    #[test]
    fn test_error_response_with_details() {
        let fixture = ErrorResponse {
            error: "Not found".to_string(),
            details: Some("Resource ID: 123".to_string()),
        };
        let actual = serde_json::to_string(&fixture).unwrap();
        assert!(actual.contains("Not found"));
        assert!(actual.contains("Resource ID: 123"));
    }

    #[test]
    fn test_app_error_bad_request() {
        let actual = AppError::bad_request("Invalid input");
        assert_eq!(actual.status, StatusCode::BAD_REQUEST);
        assert_eq!(actual.message, "Invalid input");
    }

    #[test]
    fn test_app_error_not_found() {
        let actual = AppError::not_found("Resource not found");
        assert_eq!(actual.status, StatusCode::NOT_FOUND);
        assert_eq!(actual.message, "Resource not found");
    }

    #[test]
    fn test_app_error_with_details() {
        let actual = AppError::bad_request("Invalid input")
            .with_details("Field 'name' is required");
        assert_eq!(actual.details, Some("Field 'name' is required".to_string()));
    }
}
