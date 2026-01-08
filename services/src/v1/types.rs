//! Shared types for V1 API endpoints.

use serde::Serialize;

/// Generic error response for V1 API.
#[derive(Debug, Serialize)]
pub struct V1ErrorResponse {
    pub error: String,
    pub message: String,
}

impl V1ErrorResponse {
    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            error: "not_found".to_string(),
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            error: "bad_request".to_string(),
            message: message.into(),
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            error: "internal_error".to_string(),
            message: message.into(),
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            error: "forbidden".to_string(),
            message: message.into(),
        }
    }
}
