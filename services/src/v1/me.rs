//! Handler for `/v1/me` endpoint.

use crate::database::SqlStorage;
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::Serialize;

/// Response from the `/v1/me` endpoint containing authenticated user information.
#[derive(Debug, Serialize)]
pub struct V1MeResponse {
    /// The authenticated user's username.
    pub username: String,
    /// Token issued-at timestamp (Unix seconds).
    pub issued_at: i64,
    /// Token expiration timestamp (Unix seconds).
    pub expires_at: i64,
}

/// Get the current authenticated user's information.
///
/// This endpoint requires a valid session JWT token in the Authorization header.
///
/// # Request
///
/// ```text
/// GET /v1/me
/// Authorization: Bearer <session_token>
/// ```
///
/// # Response
///
/// ```json
/// {
///     "username": "alice",
///     "issued_at": 1704067200,
///     "expires_at": 1704153600
/// }
/// ```
///
/// # Errors
///
/// - 401 Unauthorized: Missing or invalid token
pub async fn handler<S, U>(
    State(_state): State<AppState<S, U>>,
    auth: RequireAuth,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    (
        StatusCode::OK,
        Json(V1MeResponse {
            username: auth.username().to_string(),
            issued_at: auth.issued_at(),
            expires_at: auth.expires_at(),
        }),
    )
}
