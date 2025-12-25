//! HTTP routes for user management endpoints.
//!
//! This module provides API endpoints for user creation with OTP setup
//! and OTP verification for signin.

use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::post};
use serde::Serialize;

use super::otp::{
    CreateUserRequest, CreateUserResponse, OtpError, VerifyOtpRequest, VerifyOtpResponse,
    generate_otp_secret,
};
use crate::database::SqlStorage;

/// Error response for API endpoints.
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
    pub message: String,
}

impl From<OtpError> for (StatusCode, Json<ErrorResponse>) {
    fn from(err: OtpError) -> Self {
        let (status, error_type) = match &err {
            OtpError::InvalidUsername(_) => (StatusCode::BAD_REQUEST, "invalid_username"),
            OtpError::InvalidCode => (StatusCode::UNAUTHORIZED, "invalid_code"),
            OtpError::SecretGeneration(_) | OtpError::TotpCreation(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error")
            }
        };

        (
            status,
            Json(ErrorResponse {
                error: error_type.to_string(),
                message: err.to_string(),
            }),
        )
    }
}

/// Creates the router for user-related internal endpoints.
///
/// These endpoints are intended to be used only in internal environments
/// protected by Cloudflare Zero Trust or similar access control.
pub fn internal_routes<S>() -> Router<S>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
{
    Router::new().route("/users", post(create_user::<S>))
}

/// Creates the router for authentication endpoints.
pub fn auth_routes<S>() -> Router<S>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
{
    Router::new().route("/verify-otp", post(verify_otp::<S>))
}

/// Handler for creating a new user with OTP authentication.
///
/// # Request
///
/// POST /internal/users
///
/// ```json
/// {
///     "username": "john_doe"
/// }
/// ```
///
/// # Response
///
/// ```json
/// {
///     "username": "john_doe",
///     "secret": "BASE32ENCODEDSECRET",
///     "otpauth_url": "otpauth://totp/Collects:john_doe?secret=..."
/// }
/// ```
///
/// The `otpauth_url` can be used to generate a QR code for the user to scan
/// with Google Authenticator or similar apps.
#[tracing::instrument(skip_all, fields(username = %payload.username))]
async fn create_user<S>(
    State(_storage): State<S>,
    Json(payload): Json<CreateUserRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
{
    tracing::info!("Creating user with OTP setup");

    match generate_otp_secret(&payload.username) {
        Ok((secret, otpauth_url)) => {
            // In a real implementation, we would:
            // 1. Check if the username already exists
            // 2. Store the username and secret in the database
            // 3. Return the response

            // For now, we just return the generated secret
            // TODO: Implement database storage for user secrets

            tracing::info!("Successfully generated OTP secret for user");

            (
                StatusCode::CREATED,
                Json(CreateUserResponse {
                    username: payload.username,
                    secret,
                    otpauth_url,
                }),
            )
                .into_response()
        }
        Err(err) => {
            tracing::warn!("Failed to create user: {}", err);
            let (status, json): (StatusCode, Json<ErrorResponse>) = err.into();
            (status, json).into_response()
        }
    }
}

/// Handler for verifying an OTP code.
///
/// # Request
///
/// POST /auth/verify-otp
///
/// ```json
/// {
///     "username": "john_doe",
///     "code": "123456"
/// }
/// ```
///
/// # Response
///
/// ```json
/// {
///     "valid": true
/// }
/// ```
#[tracing::instrument(skip_all, fields(username = %payload.username))]
async fn verify_otp<S>(
    State(_storage): State<S>,
    Json(payload): Json<VerifyOtpRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
{
    tracing::info!("Verifying OTP code");

    // In a real implementation, we would:
    // 1. Look up the user's secret from the database
    // 2. Verify the OTP code against the stored secret
    // 3. Return the result

    // For now, we return a placeholder response since we don't have
    // database integration for user secrets yet.
    // TODO: Implement database lookup for user secrets

    // Validate that username is not empty
    if payload.username.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(VerifyOtpResponse {
                valid: false,
                message: Some("Username cannot be empty".to_string()),
            }),
        )
            .into_response();
    }

    // Validate that code is not empty and is 6 digits
    if payload.code.len() != 6 || !payload.code.chars().all(|c| c.is_ascii_digit()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(VerifyOtpResponse {
                valid: false,
                message: Some("Invalid OTP code format. Code must be 6 digits.".to_string()),
            }),
        )
            .into_response();
    }

    // TODO: Replace with actual database lookup and verification
    // For now, return a response indicating the feature is not fully implemented
    tracing::warn!("OTP verification attempted but user secrets are not yet stored in database");

    (
        StatusCode::NOT_IMPLEMENTED,
        Json(VerifyOtpResponse {
            valid: false,
            message: Some(
                "User verification requires database integration. User secrets not yet stored."
                    .to_string(),
            ),
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[derive(Clone)]
    struct MockStorage {
        is_connected: bool,
    }

    impl SqlStorage for MockStorage {
        async fn is_connected(&self) -> bool {
            self.is_connected
        }
    }

    fn create_test_app() -> Router {
        let storage = MockStorage { is_connected: true };

        Router::new()
            .nest("/internal", internal_routes::<MockStorage>())
            .nest("/auth", auth_routes::<MockStorage>())
            .with_state(storage)
    }

    #[tokio::test]
    async fn test_create_user_success() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/internal/users")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": "testuser"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: CreateUserResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert_eq!(response.username, "testuser");
        assert!(!response.secret.is_empty());
        assert!(response.otpauth_url.starts_with("otpauth://totp/"));
        assert!(response.otpauth_url.contains("testuser"));
    }

    #[tokio::test]
    async fn test_create_user_empty_username() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/internal/users")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": ""}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_create_user_invalid_json() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/internal/users")
            .header("content-type", "application/json")
            .body(Body::from(r#"invalid json"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        // Axum returns 400 BAD_REQUEST for invalid JSON parsing errors
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_verify_otp_empty_username() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": "", "code": "123456"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: VerifyOtpResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert!(!response.valid);
        assert!(response.message.is_some());
    }

    #[tokio::test]
    async fn test_verify_otp_invalid_code_format() {
        let app = create_test_app();

        // Test with non-numeric code
        let request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": "testuser", "code": "abcdef"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_verify_otp_wrong_length_code() {
        let app = create_test_app();

        // Test with wrong length code
        let request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": "testuser", "code": "12345"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_verify_otp_not_implemented() {
        let app = create_test_app();

        // Test valid format but not implemented
        let request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": "testuser", "code": "123456"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        // Should return NOT_IMPLEMENTED until database integration is done
        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
    }
}
