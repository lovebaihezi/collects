//! /v1/uploads endpoint handlers.

use crate::database::SqlStorage;
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use super::types::{V1UploadsInitRequest, V1UploadsInitResponse};

/// Initialize an upload.
///
/// POST /v1/uploads/init
pub async fn v1_uploads_init<S, U>(
    State(_state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(payload): Json<V1UploadsInitRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Use request fields to avoid dead-code warnings while this is still a stub.
    let _ = (&payload.content_type, payload.file_size, auth.username());

    let storage_key = format!("uploads/{}", payload.filename);

    (
        StatusCode::CREATED,
        Json(V1UploadsInitResponse {
            upload_id: "00000000-0000-0000-0000-000000000000".to_string(),
            storage_key,
            method: "put".to_string(),
            upload_url: "https://example.invalid/upload".to_string(),
            expires_at: "1970-01-01T00:00:00Z".to_string(),
        }),
    )
}
