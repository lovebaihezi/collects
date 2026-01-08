//! Handlers for `/v1/uploads/*` endpoints.

use crate::database::SqlStorage;
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use serde::{Deserialize, Serialize};

/// Request body for initializing an upload.
#[derive(Debug, Deserialize)]
pub struct V1UploadsInitRequest {
    pub filename: String,
    pub content_type: String,
    pub file_size: u64,
}

/// Response from upload initialization.
#[derive(Debug, Serialize)]
pub struct V1UploadsInitResponse {
    pub upload_id: String,
    pub storage_key: String,
    pub method: String,
    pub upload_url: String,
    pub expires_at: String,
}

/// Initialize an upload session.
///
/// POST /v1/uploads/init
///
/// This is currently a stub implementation that returns placeholder values.
/// The actual implementation will generate presigned URLs for direct-to-storage uploads.
pub async fn init<S, U>(
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

/// Request body for generating a view URL.
#[derive(Debug, Deserialize)]
pub struct V1ViewUrlRequest {
    pub disposition: String,
}

/// Response containing a signed view URL.
#[derive(Debug, Serialize)]
pub struct V1ViewUrlResponse {
    pub url: String,
    pub expires_at: String,
}

/// Generate a signed URL for viewing content.
///
/// POST /v1/contents/:id/view-url
///
/// This is currently a stub implementation that returns placeholder values.
/// The actual implementation will generate presigned URLs for content access.
pub async fn view_url<S, U>(
    State(_state): State<AppState<S, U>>,
    auth: RequireAuth,
    axum::extract::Path(_id): axum::extract::Path<String>,
    Json(payload): Json<V1ViewUrlRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Use request fields to avoid dead-code warnings while this is still a stub.
    let _ = (payload.disposition, auth.username());

    (
        StatusCode::OK,
        Json(V1ViewUrlResponse {
            url: "https://example.invalid/view".to_string(),
            expires_at: "1970-01-01T00:00:00Z".to_string(),
        }),
    )
}
