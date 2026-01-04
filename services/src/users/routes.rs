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
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post, put},
};
use serde::{Deserialize, Serialize};

use super::otp::{
    CreateUserRequest, CreateUserResponse, OtpError, VerifyOtpRequest, VerifyOtpResponse,
    generate_current_otp_with_time, generate_otp_secret, verify_otp,
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
    /// Seconds remaining until the OTP code expires (1-30).
    pub time_remaining: u8,
    /// The user's nickname (optional).
    pub nickname: Option<String>,
    /// The user's avatar URL (optional).
    pub avatar_url: Option<String>,
    /// When the user was created (ISO 8601 format).
    pub created_at: String,
    /// When the user was last updated (ISO 8601 format).
    pub updated_at: String,
}

/// Response for the list users endpoint.
#[derive(Debug, Serialize, Deserialize)]
pub struct ListUsersResponse {
    /// List of users with their current OTP codes.
    pub users: Vec<UserListItem>,
}

/// Response for getting a single user with QR code info.
#[derive(Debug, Serialize, Deserialize)]
pub struct GetUserResponse {
    /// The username.
    pub username: String,
    /// The current OTP code for this user.
    pub current_otp: String,
    /// Seconds remaining until the OTP code expires (1-30).
    pub time_remaining: u8,
    /// The otpauth URL for QR code generation.
    pub otpauth_url: String,
}

/// Request to update a username.
#[derive(Debug, Deserialize)]
pub struct UpdateUsernameRequest {
    /// The new username.
    pub new_username: String,
}

/// Response for updating a username.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUsernameResponse {
    /// The old username.
    pub old_username: String,
    /// The new username.
    pub new_username: String,
}

/// Response for revoking OTP (regenerating secret).
#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeOtpResponse {
    /// The username.
    pub username: String,
    /// The new secret key for OTP generation (base32 encoded).
    pub secret: String,
    /// The otpauth URL for QR code generation.
    pub otpauth_url: String,
}

/// Response for deleting a user.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteUserResponse {
    /// The username that was deleted.
    pub username: String,
    /// Whether the deletion was successful.
    pub deleted: bool,
}

/// Request to update user's profile (nickname and avatar URL).
#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    /// The new nickname (optional, pass null to remove).
    pub nickname: Option<String>,
    /// The new avatar URL (optional, pass null to remove).
    pub avatar_url: Option<String>,
}

/// Response for updating user's profile.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateProfileResponse {
    /// The username.
    pub username: String,
    /// The updated nickname.
    pub nickname: Option<String>,
    /// The updated avatar URL.
    pub avatar_url: Option<String>,
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
        .route("/users/{username}", get(get_user::<S, U>))
        .route("/users/{username}", put(update_username::<S, U>))
        .route("/users/{username}", delete(delete_user::<S, U>))
        .route("/users/{username}/revoke", post(revoke_otp::<S, U>))
        .route("/users/{username}/profile", put(update_profile::<S, U>))
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
                    // Generate current OTP code and time remaining for each user
                    match generate_current_otp_with_time(&user.secret) {
                        Ok((otp, time_remaining)) => Some(UserListItem {
                            username: user.username,
                            current_otp: otp,
                            time_remaining,
                            nickname: user.nickname,
                            avatar_url: user.avatar_url,
                            created_at: user.created_at.to_rfc3339(),
                            updated_at: user.updated_at.to_rfc3339(),
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

/// Handler for getting a single user with QR code information.
///
/// # Request
///
/// GET /internal/users/:username
///
/// # Response
///
/// ```json
/// {
///     "username": "john_doe",
///     "current_otp": "123456",
///     "time_remaining": 25,
///     "otpauth_url": "otpauth://totp/Collects:john_doe?secret=..."
/// }
/// ```
#[tracing::instrument(skip_all, fields(username = %username))]
async fn get_user<S, U>(
    State(state): State<AppState<S, U>>,
    Path(username): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    tracing::info!("Getting user details");

    match state.user_storage.get_user(&username).await {
        Ok(Some(user)) => {
            // Generate current OTP, time remaining, and otpauth URL
            match generate_current_otp_with_time(&user.secret) {
                Ok((current_otp, time_remaining)) => {
                    // Generate otpauth URL using the same function used in create
                    match generate_otp_secret_url(&user.username, &user.secret) {
                        Ok(otpauth_url) => (
                            StatusCode::OK,
                            Json(GetUserResponse {
                                username: user.username,
                                current_otp,
                                time_remaining,
                                otpauth_url,
                            }),
                        )
                            .into_response(),
                        Err(e) => {
                            tracing::error!("Failed to generate otpauth URL: {}", e);
                            let (status, json): (StatusCode, Json<ErrorResponse>) = e.into();
                            (status, json).into_response()
                        }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to generate current OTP: {}", e);
                    let (status, json): (StatusCode, Json<ErrorResponse>) = e.into();
                    (status, json).into_response()
                }
            }
        }
        Ok(None) => {
            tracing::warn!("User not found: {}", username);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::UserNotFound(username).into();
            (status, json).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::StorageError(e.to_string()).into();
            (status, json).into_response()
        }
    }
}

/// Helper to generate otpauth URL from existing secret.
fn generate_otp_secret_url(username: &str, secret_base32: &str) -> Result<String, OtpError> {
    use totp_rs::{Algorithm, Secret, TOTP};

    let secret = Secret::Encoded(secret_base32.to_string());
    let secret_bytes = secret
        .to_bytes()
        .map_err(|e| OtpError::SecretGeneration(e.to_string()))?;

    let totp = TOTP::new(
        Algorithm::SHA1,
        6,  // OTP_DIGITS
        1,  // OTP_SKEW
        30, // OTP_STEP
        secret_bytes,
        Some(super::otp::ISSUER.to_string()),
        username.to_string(),
    )
    .map_err(|e| OtpError::TotpCreation(e.to_string()))?;

    Ok(totp.get_url())
}

/// Handler for updating a username.
///
/// # Request
///
/// PUT /internal/users/:username
///
/// ```json
/// {
///     "new_username": "new_john_doe"
/// }
/// ```
///
/// # Response
///
/// ```json
/// {
///     "old_username": "john_doe",
///     "new_username": "new_john_doe"
/// }
/// ```
#[tracing::instrument(skip_all, fields(username = %username))]
async fn update_username<S, U>(
    State(state): State<AppState<S, U>>,
    Path(username): Path<String>,
    Json(payload): Json<UpdateUsernameRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    tracing::info!("Updating username to: {}", payload.new_username);

    if payload.new_username.is_empty() {
        let (status, json): (StatusCode, Json<ErrorResponse>) =
            UserStorageError::InvalidInput("New username cannot be empty".to_string()).into();
        return (status, json).into_response();
    }

    match state
        .user_storage
        .update_username(&username, &payload.new_username)
        .await
    {
        Ok(_updated_user) => {
            tracing::info!("Successfully updated username");
            (
                StatusCode::OK,
                Json(UpdateUsernameResponse {
                    old_username: username,
                    new_username: payload.new_username,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to update username: {}", e);
            // Parse error type from the message to provide appropriate status
            let error_str = e.to_string();
            let (status, error_type) = if error_str.contains("not found") {
                (StatusCode::NOT_FOUND, "user_not_found")
            } else if error_str.contains("already exists") {
                (StatusCode::CONFLICT, "user_already_exists")
            } else if error_str.contains("Invalid input") || error_str.contains("cannot be empty") {
                (StatusCode::BAD_REQUEST, "invalid_input")
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "storage_error")
            };
            (
                status,
                Json(ErrorResponse {
                    error: error_type.to_string(),
                    message: error_str,
                }),
            )
                .into_response()
        }
    }
}

/// Handler for deleting a user.
///
/// # Request
///
/// DELETE /internal/users/:username
///
/// # Response
///
/// ```json
/// {
///     "username": "john_doe",
///     "deleted": true
/// }
/// ```
#[tracing::instrument(skip_all, fields(username = %username))]
async fn delete_user<S, U>(
    State(state): State<AppState<S, U>>,
    Path(username): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    tracing::info!("Deleting user");

    match state.user_storage.delete_user(&username).await {
        Ok(deleted) => {
            if deleted {
                tracing::info!("Successfully deleted user");
            } else {
                tracing::warn!("User not found for deletion: {}", username);
            }
            (
                StatusCode::OK,
                Json(DeleteUserResponse { username, deleted }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to delete user: {}", e);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::StorageError(e.to_string()).into();
            (status, json).into_response()
        }
    }
}

/// Handler for revoking a user's OTP (regenerating secret).
///
/// # Request
///
/// POST /internal/users/:username/revoke
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
#[tracing::instrument(skip_all, fields(username = %username))]
async fn revoke_otp<S, U>(
    State(state): State<AppState<S, U>>,
    Path(username): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    tracing::info!("Revoking OTP for user");

    // Check if user exists first
    match state.user_storage.user_exists(&username).await {
        Ok(false) => {
            tracing::warn!("User not found: {}", username);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::UserNotFound(username).into();
            return (status, json).into_response();
        }
        Ok(true) => {}
        Err(e) => {
            tracing::error!("Failed to check user existence: {}", e);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::StorageError(e.to_string()).into();
            return (status, json).into_response();
        }
    }

    // Generate new OTP secret
    let (new_secret, otpauth_url) = match generate_otp_secret(&username) {
        Ok(result) => result,
        Err(err) => {
            tracing::warn!("Failed to generate new OTP secret: {}", err);
            let (status, json): (StatusCode, Json<ErrorResponse>) = err.into();
            return (status, json).into_response();
        }
    };

    // Update the user's OTP secret
    match state.user_storage.revoke_otp(&username, &new_secret).await {
        Ok(_updated_user) => {
            tracing::info!("Successfully revoked OTP for user");
            (
                StatusCode::OK,
                Json(RevokeOtpResponse {
                    username,
                    secret: new_secret,
                    otpauth_url,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to revoke OTP: {}", e);
            let (status, json): (StatusCode, Json<ErrorResponse>) =
                UserStorageError::StorageError(e.to_string()).into();
            (status, json).into_response()
        }
    }
}

/// Handler for updating a user's profile (nickname and avatar URL).
///
/// # Request
///
/// PUT /internal/users/:username/profile
///
/// ```json
/// {
///     "nickname": "John",
///     "avatar_url": "https://example.com/avatar.png"
/// }
/// ```
///
/// # Response
///
/// ```json
/// {
///     "username": "john_doe",
///     "nickname": "John",
///     "avatar_url": "https://example.com/avatar.png"
/// }
/// ```
#[tracing::instrument(skip_all, fields(username = %username))]
async fn update_profile<S, U>(
    State(state): State<AppState<S, U>>,
    Path(username): Path<String>,
    Json(payload): Json<UpdateProfileRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    tracing::info!("Updating profile for user");

    // Convert the request format to storage format
    // The request uses Option<String> where Some("value") sets the field and None clears it
    // For the storage, we need Option<Option<String>> where:
    // - None means "don't update"
    // - Some(None) means "clear the field"
    // - Some(Some("value")) means "set to value"
    //
    // In this API, we always update both fields if provided in the request
    let nickname_update = Some(payload.nickname);
    let avatar_url_update = Some(payload.avatar_url);

    match state
        .user_storage
        .update_profile(&username, nickname_update, avatar_url_update)
        .await
    {
        Ok(updated_user) => {
            tracing::info!("Successfully updated profile for user");
            (
                StatusCode::OK,
                Json(UpdateProfileResponse {
                    username: updated_user.username,
                    nickname: updated_user.nickname,
                    avatar_url: updated_user.avatar_url,
                }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to update profile: {}", e);
            let error_str = e.to_string();
            let (status, error_type) = if error_str.contains("not found") {
                (StatusCode::NOT_FOUND, "user_not_found")
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "storage_error")
            };
            (
                status,
                Json(ErrorResponse {
                    error: error_type.to_string(),
                    message: error_str,
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

        // Check that all OTP codes are valid format (6 digits) and time_remaining is valid
        for user in &response.users {
            assert_eq!(user.current_otp.len(), 6);
            assert!(user.current_otp.chars().all(|c| c.is_ascii_digit()));
            assert!(
                (1..=30).contains(&user.time_remaining),
                "time_remaining should be between 1 and 30, got {}",
                user.time_remaining
            );
        }

        // Check usernames
        let usernames: Vec<&str> = response.users.iter().map(|u| u.username.as_str()).collect();
        assert!(usernames.contains(&"alice"));
        assert!(usernames.contains(&"bob"));
    }

    // ===========================================
    // Tests for GET /internal/users/{username}
    // ===========================================

    #[tokio::test]
    async fn test_get_user_success() {
        let (secret, _) = generate_otp_secret("alice").expect("Should generate secret");

        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users(vec![("alice", secret.as_str())]);
        let state = AppState::new(sql_storage, user_storage);

        let app = Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .with_state(state);

        let request = Request::builder()
            .method("GET")
            .uri("/internal/users/alice")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: GetUserResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert_eq!(response.username, "alice");
        assert_eq!(response.current_otp.len(), 6);
        assert!(
            (1..=30).contains(&response.time_remaining),
            "time_remaining should be between 1 and 30, got {}",
            response.time_remaining
        );
        assert!(response.otpauth_url.contains("alice"));
        assert!(response.otpauth_url.starts_with("otpauth://totp/"));
    }

    #[tokio::test]
    async fn test_get_user_not_found() {
        let app = create_test_app();

        let request = Request::builder()
            .method("GET")
            .uri("/internal/users/nonexistent")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // ===========================================
    // Tests for PUT /internal/users/{username}
    // ===========================================

    #[tokio::test]
    async fn test_update_username_success() {
        let (secret, _) = generate_otp_secret("oldname").expect("Should generate secret");

        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users(vec![("oldname", secret.as_str())]);
        let state = AppState::new(sql_storage, user_storage);

        let app = Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .with_state(state);

        let request = Request::builder()
            .method("PUT")
            .uri("/internal/users/oldname")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"new_username": "newname"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: UpdateUsernameResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert_eq!(response.old_username, "oldname");
        assert_eq!(response.new_username, "newname");
    }

    #[tokio::test]
    async fn test_update_username_user_not_found() {
        let app = create_test_app();

        let request = Request::builder()
            .method("PUT")
            .uri("/internal/users/nonexistent")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"new_username": "newname"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_username_empty_new_username() {
        let (secret, _) = generate_otp_secret("alice").expect("Should generate secret");

        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users(vec![("alice", secret.as_str())]);
        let state = AppState::new(sql_storage, user_storage);

        let app = Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .with_state(state);

        let request = Request::builder()
            .method("PUT")
            .uri("/internal/users/alice")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"new_username": ""}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_update_username_duplicate() {
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

        // Try to rename alice to bob (which already exists)
        let request = Request::builder()
            .method("PUT")
            .uri("/internal/users/alice")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"new_username": "bob"}"#))
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }

    // ===========================================
    // Tests for DELETE /internal/users/{username}
    // ===========================================

    #[tokio::test]
    async fn test_delete_user_success() {
        let (secret, _) = generate_otp_secret("todelete").expect("Should generate secret");

        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users(vec![("todelete", secret.as_str())]);
        let state = AppState::new(sql_storage, user_storage);

        let app = Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .with_state(state);

        let request = Request::builder()
            .method("DELETE")
            .uri("/internal/users/todelete")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: DeleteUserResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert_eq!(response.username, "todelete");
        assert!(response.deleted);
    }

    #[tokio::test]
    async fn test_delete_user_not_found() {
        let app = create_test_app();

        let request = Request::builder()
            .method("DELETE")
            .uri("/internal/users/nonexistent")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: DeleteUserResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert_eq!(response.username, "nonexistent");
        assert!(!response.deleted);
    }

    // ===========================================
    // Tests for POST /internal/users/{username}/revoke
    // ===========================================

    #[tokio::test]
    async fn test_revoke_otp_success() {
        let (secret, _) = generate_otp_secret("alice").expect("Should generate secret");

        let sql_storage = MockSqlStorage { is_connected: true };
        let user_storage = MockUserStorage::with_users(vec![("alice", secret.as_str())]);
        let state = AppState::new(sql_storage, user_storage);

        let app = Router::new()
            .nest(
                "/internal",
                internal_routes::<MockSqlStorage, MockUserStorage>(),
            )
            .with_state(state);

        let request = Request::builder()
            .method("POST")
            .uri("/internal/users/alice/revoke")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let response: RevokeOtpResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        assert_eq!(response.username, "alice");
        // New secret should be different from the old one
        assert_ne!(response.secret, secret);
        assert!(response.otpauth_url.contains("alice"));
    }

    #[tokio::test]
    async fn test_revoke_otp_user_not_found() {
        let app = create_test_app();

        let request = Request::builder()
            .method("POST")
            .uri("/internal/users/nonexistent/revoke")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app.oneshot(request).await.expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    // ===========================================
    // Integration test for user management flow
    // ===========================================

    #[tokio::test]
    async fn test_full_user_management_flow() {
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
            .body(Body::from(r#"{"username": "mgmttest"}"#))
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

        let original_secret = create_response.secret.clone();

        // Step 2: Get user details
        let get_request = Request::builder()
            .method("GET")
            .uri("/internal/users/mgmttest")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app
            .clone()
            .oneshot(get_request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        // Step 3: Update username
        let update_request = Request::builder()
            .method("PUT")
            .uri("/internal/users/mgmttest")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"new_username": "renameduser"}"#))
            .expect("Failed to create request");

        let response = app
            .clone()
            .oneshot(update_request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        // Step 4: Verify OTP still works with renamed user
        let valid_code = generate_current_otp(&original_secret).expect("Should generate code");

        let verify_request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"username": "renameduser", "code": "{}"}}"#,
                valid_code
            )))
            .expect("Failed to create request");

        let response = app
            .clone()
            .oneshot(verify_request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        // Step 5: Revoke OTP
        let revoke_request = Request::builder()
            .method("POST")
            .uri("/internal/users/renameduser/revoke")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app
            .clone()
            .oneshot(revoke_request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("Failed to read body");

        let revoke_response: RevokeOtpResponse =
            serde_json::from_slice(&body).expect("Failed to parse response");

        // Verify new secret is different
        assert_ne!(revoke_response.secret, original_secret);

        // Step 6: Verify old OTP code no longer works
        let verify_request = Request::builder()
            .method("POST")
            .uri("/auth/verify-otp")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"username": "renameduser", "code": "{}"}}"#,
                valid_code
            )))
            .expect("Failed to create request");

        let response = app
            .clone()
            .oneshot(verify_request)
            .await
            .expect("Failed to get response");

        // Should be unauthorized because OTP was revoked
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        // Step 7: Delete user
        let delete_request = Request::builder()
            .method("DELETE")
            .uri("/internal/users/renameduser")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app
            .clone()
            .oneshot(delete_request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::OK);

        // Step 8: Verify user no longer exists
        let get_request = Request::builder()
            .method("GET")
            .uri("/internal/users/renameduser")
            .body(Body::empty())
            .expect("Failed to create request");

        let response = app
            .oneshot(get_request)
            .await
            .expect("Failed to get response");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
