//! /v1/me endpoint handler.

use crate::database::SqlStorage;
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use super::types::{V1ErrorResponse, V1MeResponse};

/// Get the current authenticated user's information.
#[utoipa::path(
    get,
    path = "/v1/me",
    tag = "me",
    responses(
        (status = 200, description = "Current user information", body = V1MeResponse),
        (status = 401, description = "Unauthorized - missing or invalid token", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
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
            username: auth.username().to_owned(),
            issued_at: auth.issued_at(),
            expires_at: auth.expires_at(),
        }),
    )
}
