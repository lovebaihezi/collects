//! HTTP routes for user management endpoints.
//!
//! This module provides API endpoints for user creation with OTP setup
//! and OTP verification for signin.
//!
//! ## Example: Using ZeroTrustAuth Extractor
//!
//! When Cloudflare Zero Trust is enabled, protected handlers can extract
//! user information from validated JWT tokens:
//!
//! ```rust,ignore
//! use axum::{Json, response::IntoResponse};
//! use collects_services::auth::ZeroTrustAuth;
//! use serde::Serialize;
//!
//! #[derive(Serialize)]
//! struct WhoAmI {
//!     email: String,
//!     subject: String,
//! }
//!
//! async fn whoami_handler(auth: ZeroTrustAuth) -> impl IntoResponse {
//!     Json(WhoAmI {
//!         email: auth.email().unwrap_or("unknown").to_string(),
//!         subject: auth.subject().to_string(),
//!     })
//! }
//! ```

use axum::{
    Json, Router,
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use super::otp::{
    CreateUserRequest, CreateUserResponse, OtpError, VerifyOtpRequest, VerifyOtpResponse,
    generate_current_otp, generate_otp_secret, verify_otp,
};
use super::storage::{UserStorage, UserStorageError};
use crate::database::SqlStorage;

/// Response for listing users with their current OTP codes.
#[derive(Debug, Serialize, Deserialize)]
pub struct UserListItem {
    /// The username.
    pub username: String,
    /// The current OTP code for this user.
    pub current_otp: String,
}

/// Response for the list users endpoint.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListUsersResponse {
    /// List of users with their current OTP codes.
    pub users: Vec<UserListItem>,
}

/// Error response for API endpoints.
#[derive(Debug, Serialize, Deserialize)]
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

impl From<UserStorageError> for (StatusCode, Json<ErrorResponse>) {
    fn from(err: UserStorageError) -> Self {
        let (status, error_type) = match &err {
            UserStorageError::UserAlreadyExists(_) => (StatusCode::CONFLICT, "user_already_exists"),
            UserStorageError::UserNotFound(_) => (StatusCode::NOT_FOUND, "user_not_found"),
            UserStorageError::InvalidInput(_) => (StatusCode::BAD_REQUEST, "invalid_input"),
            UserStorageError::StorageError(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "storage_error")
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

/// Combined application state for routes that need both SQL and User storage.
#[derive(Clone)]
pub struct AppState<S, U> {
    pub sql_storage: S,
    pub user_storage: U,
}

impl<S, U> AppState<S, U> {
    /// Creates a new `AppState` with the given storage implementations.
    pub fn new(sql_storage: S, user_storage: U) -> Self {
        Self {
            sql_storage,
            user_storage,
        }
    }
}

/// Creates the router for user-related internal endpoints.
///
/// These endpoints are intended to be used only in internal environments
/// protected by Cloudflare Zero Trust or similar access control.
///
/// # Type Parameters
///
/// * `S` - SQL storage implementation
/// * `U` - User storage implementation for storing user secrets
pub fn internal_routes<S, U>() -> Router<AppState<S, U>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/users", post(create_user::<S, U>))
        .route("/users", get(list_users::<S, U>))
}

/// Creates the router for authentication endpoints.
///
/// # Type Parameters
///
/// * `S` - SQL storage implementation
/// * `U` - User storage implementation for retrieving user secrets
pub fn auth_routes<S, U>() -> Router<AppState<S, U>>
where
    S: SqlStorage + Clone + Send + Sync + 'static,
    U: UserStorage + Clone + Send + Sync + 'static,
{
    Router::new().route("/verify-otp", post(verify_otp_handler::<S, U>))
}

/// Handler for creating a new user with OTP authentication.
///
/// This handler uses the `UserStorage` trait to persist user data,
/// enabling proper testing and different storage backends.
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
async fn create_user<S, U>(
    State(state): State<AppState<S, U>>,
    Json(payload): Json<CreateUserRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    tracing::info!("Creating user with OTP setup");

    // First, check if user already exists
    match state.user_storage.user_exists(&payload.username).await {
        Ok(true) => {
            tracing::warn!("User already exists: {}", payload.username);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::UserAlreadyExists(payload.username).into();
            return (status, json).into_response();
        }
        Ok(false) => {}
        Err(e) => {
            tracing::error!("Failed to check user existence: {}", e);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::StorageError(e.to_string()).into();
            return (status, json).into_response();
        }
    }

    // Generate OTP secret
    let (secret, otpauth_url) = match generate_otp_secret(&payload.username) {
        Ok(result) => result,
        Err(err) => {
            tracing::warn!("Failed to generate OTP secret: {}", err);
            let (status, json): (StatusCode, Json<ErrorResponse>) = err.into();
            return (status, json).into_response();
        }
    };

    // Store user in storage
    match state
        .user_storage
        .create_user(&payload.username, &secret)
        .await
    {
        Ok(_stored_user) => {
            tracing::info!("Successfully created user and stored OTP secret");

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
        Err(e) => {
            tracing::error!("Failed to store user: {}", e);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::StorageError(e.to_string()).into();
            (status, json).into_response()
        }
    }
}

/// Handler for listing all users with their current OTP codes.
///
/// This endpoint is intended for internal use only and should be protected
/// by Zero Trust or similar access control.
///
/// # Request
///
/// GET /internal/users
///
/// # Response
///
/// ```json
/// {
///     "users": [
///         {
///             "username": "john_doe",
///             "current_otp": "123456"
///         }
///     ]
/// }
/// ```
#[tracing::instrument(skip_all)]
async fn list_users<S, U>(State(state): State<AppState<S, U>>) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    tracing::info!("Listing all users with current OTP codes");

    match state.user_storage.list_users().await {
        Ok(users) => {
            let user_items: Vec<UserListItem> = users
                .into_iter()
                .filter_map(|user| {
                    // Generate current OTP code for each user
                    match generate_current_otp(&user.secret) {
                        Ok(otp) => Some(UserListItem {
                            username: user.username,
                            current_otp: otp,
                        }),
                        Err(e) => {
                            tracing::warn!(
                                "Failed to generate OTP for user {}: {}",
                                user.username,
                                e
                            );
                            // Skip users with invalid secrets
                            None
                        }
                    }
                })
                .collect();

            tracing::info!("Listed {} users", user_items.len());
            (
                StatusCode::OK,
                Json(ListUsersResponse { users: user_items }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list users: {}", e);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::StorageError(e.to_string()).into();
            (status, json).into_response()
        }
    }
}

/// Handler for verifying an OTP code.
///
/// This handler uses the `UserStorage` trait to retrieve the user's secret,
/// enabling proper testing and different storage backends.
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
async fn verify_otp_handler<S, U>(
    State(state): State<AppState<S, U>>,
    Json(payload): Json<VerifyOtpRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    tracing::info!("Verifying OTP code");

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
    let is_valid_format =
        payload.code.len() == 6 && payload.code.bytes().all(|b| b.is_ascii_digit());

    if !is_valid_format {
        return (
            StatusCode::BAD_REQUEST,
            Json(VerifyOtpResponse {
                valid: false,
                message: Some("Invalid OTP code format. Code must be 6 digits.".to_string()),
            }),
        )
            .into_response();
    }

    // Look up the user's secret from storage
    let secret = match state.user_storage.get_user_secret(&payload.username).await {
        Ok(Some(secret)) => secret,
        Ok(None) => {
            tracing::warn!("User not found: {}", payload.username);
            return (
                StatusCode::UNAUTHORIZED,
                Json(VerifyOtpResponse {
                    valid: false,
                    message: Some("Invalid username or code".to_string()),
                }),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to retrieve user secret: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(VerifyOtpResponse {
                    valid: false,
                    message: Some("Internal server error".to_string()),
                }),
            )
                .into_response();
        }
    };

    // Verify the OTP code against the stored secret
    match verify_otp(&secret, &payload.code) {
        Ok(true) => {
            tracing::info!("OTP verification successful");
            (
                StatusCode::OK,
                Json(VerifyOtpResponse {
                    valid: true,
                    message: None,
                }),
            )
                .into_response()
        }
        Ok(false) => {
            tracing::warn!("OTP verification failed - invalid code");
            (
                StatusCode::UNAUTHORIZED,
                Json(VerifyOtpResponse {
                    valid: false,
                    message: Some("Invalid username or code".to_string()),
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("OTP verification error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(VerifyOtpResponse {
                    valid: false,
                    message: Some("Internal server error".to_string()),
                }),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::users::storage::MockUserStorage;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[derive(Clone)]
    struct MockSqlStorage {
        is_connected: bool,
    }

    impl SqlStorage for MockSqlStorage {
        async fn is_connected(&self) -> bool {
            self.is_connected
        }
    }

    fn create_test_app() -> Router {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let state = AppState::new(sql_storage, user_storage);

        Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .nest("/auth", auth_routes::<MockSqlStorage, MockUserStorage>())
            .with_state(state)
    }

    fn create_test_app_with_users(users: Vec<(&str, &str)>) -> Router {
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users(users);
        let state = AppState::new(sql_storage, user_storage);

        Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .nest("/auth", auth_routes::<MockSqlStorage, MockUserStorage>())
            .with_state(state)
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
    async fn test_create_user_duplicate() {
        let app = create_test_app_with_users(vec![("existinguser", "SECRET123")]);

        let request = Request::builder()
            .method("POST")
            .uri("/internal/users")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": "existinguser"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::CONFLICT);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: ErrorResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert_eq!(response.error, "user_already_exists");
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
    async fn test_verify_otp_user_not_found() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"username": "nonexistent", "code": "123456"}"#,
            ))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: VerifyOtpResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert!(!response.valid);
    }

    #[tokio::test]
    async fn test_verify_otp_valid_code() {
        use crate::users::otp::generate_current_otp;

        // Create a user and get their secret
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();

        // First create a user to get a valid secret
        let (secret, _) = generate_otp_secret("testuser").expect("Should generate secret");
        user_storage
            .create_user("testuser", &secret)
            .await
            .expect("Should create user");

        // Generate a valid OTP code
        let valid_code = generate_current_otp(&secret).expect("Should generate code");

        let state = AppState::new(sql_storage, user_storage);

        let app = Router::new()
            .nest("/auth", auth_routes::<MockSqlStorage, MockUserStorage>())
            .with_state(state);

        let request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"username": "testuser", "code": "{}"}}"#,
                valid_code
            )))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: VerifyOtpResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert!(response.valid);
    }

    #[tokio::test]
    async fn test_verify_otp_invalid_code() {
        // Create a user with a known secret
        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();

        let (secret, _) = generate_otp_secret("testuser").expect("Should generate secret");
        user_storage
            .create_user("testuser", &secret)
            .await
            .expect("Should create user");

        let state = AppState::new(sql_storage, user_storage);

        let app = Router::new()
            .nest("/auth", auth_routes::<MockSqlStorage, MockUserStorage>())
            .with_state(state);

        // Use an invalid code (all zeros is statistically almost never valid)
        let request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": "testuser", "code": "000000"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        // Should be unauthorized (invalid code)
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_full_user_flow() {
        use crate::users::otp::generate_current_otp;

        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::new();
        let state = AppState::new(sql_storage, user_storage.clone());

        let app = Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .nest("/auth", auth_routes::<MockSqlStorage, MockUserStorage>())
            .with_state(state);

        // Step 1: Create a user
        let create_request = Request::builder()
            .method("POST")
            .uri("/internal/users")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username": "flowtest"}"#))
            .expect("Failed to create request");

        let response = app
            .clone()
            .oneshot(create_request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let create_response: CreateUserResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        // Step 2: Generate a valid OTP code using the secret
        let valid_code =
            generate_current_otp(&create_response.secret).expect("Should generate code");

        // Step 3: Verify the OTP code
        let verify_request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"username": "flowtest", "code": "{}"}}"#,
                valid_code
            )))
            .expect("Failed to create request");

        let response = app
            .oneshot(verify_request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let verify_response: VerifyOtpResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert!(verify_response.valid);
    }

    #[tokio::test]
    async fn test_list_users_empty() {
        let app = create_test_app();

        let request = Request::builder()
            .method("GET")
            .uri("/internal/users")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: ListUsersResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert!(response.users.is_empty());
    }

    #[tokio::test]
    async fn test_list_users_with_users() {
        // Create users with valid OTP secrets
        let (secret1, _) = generate_otp_secret("alice").expect("Should generate secret");
        let (secret2, _) = generate_otp_secret("bob").expect("Should generate secret");

        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users(vec![
            ("alice", secret1.as_str()),
            ("bob", secret2.as_str()),
        ]);
        let state = AppState::new(sql_storage, user_storage);

        let app = Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .with_state(state);

        let request = Request::builder()
            .method("GET")
            .uri("/internal/users")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: ListUsersResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert_eq!(response.users.len(), 2);

        // Check that all OTP codes are valid format (6 digits)
        for user in &response.users {
            assert_eq!(user.current_otp.len(), 6);
            assert!(user.current_otp.chars().all(|c| c.is_ascii_digit()));
        }

        // Check usernames
        let usernames: Vec<&str> = response.users.iter().map(|u| u.username.as_str()).collect();
        assert!(usernames.contains(&"alice"));
        assert!(usernames.contains(&"bob"));
    }
}
