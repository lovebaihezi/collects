//! /v1/me endpoint handler.

use crate::database::SqlStorage;
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use super::types::V1MeResponse;

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
pub async fn v1_me<S, U>(
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
