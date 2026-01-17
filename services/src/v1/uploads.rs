//! /v1/uploads endpoint handlers.

use crate::database::{ContentsInsert, SqlStorage, SqlStorageError, UploadInsert, Visibility};
use crate::storage::{DEFAULT_PRESIGN_EXPIRY, R2Presigner};
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};

use super::types::{
    V1ContentItem, V1ErrorResponse, V1UploadsCompleteRequest, V1UploadsCompleteResponse,
    V1UploadsInitRequest, V1UploadsInitResponse,
};

/// Initialize an upload.
///
/// This endpoint generates a presigned PUT URL for direct upload to R2.
/// The client should use this URL to upload the file directly, then call
/// `/v1/uploads/complete` to finalize the upload.
#[utoipa::path(
    post,
    path = "/v1/uploads/init",
    tag = "uploads",
    request_body = V1UploadsInitRequest,
    responses(
        (status = 201, description = "Upload initialized", body = V1UploadsInitResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_uploads_init<S, U>(
    State(state): State<AppState<S, U>>,
    presigner: Option<axum::Extension<R2Presigner>>,
    auth: RequireAuth,
    Json(payload): Json<V1UploadsInitRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Resolve user from JWT username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Generate storage key: {user_id}/{uuid}/{filename}
    let upload_uuid = uuid::Uuid::new_v4();
    let storage_key = format!("{}/{}/{}", user.id, upload_uuid, payload.filename);

    // Calculate expiration time (15 minutes from now)
    let expires_at =
        chrono::Utc::now() + chrono::Duration::from_std(DEFAULT_PRESIGN_EXPIRY).unwrap();

    // Create upload record in database
    let upload_input = UploadInsert {
        user_id: user.id,
        storage_backend: "r2".to_owned(),
        storage_profile: "default".to_owned(),
        storage_key: storage_key.clone(),
        content_type: payload.content_type.clone(),
        file_size: payload.file_size as i64,
        expires_at,
    };

    let upload = match state.sql_storage.uploads_create(upload_input).await {
        Ok(upload) => upload,
        Err(e) => {
            tracing::error!("Failed to create upload record: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to initialize upload",
                )),
            )
                .into_response();
        }
    };

    // Generate presigned PUT URL
    let presigned = if let Some(axum::Extension(presigner)) = presigner {
        match presigner
            .presign_put(&storage_key, &payload.content_type, DEFAULT_PRESIGN_EXPIRY)
            .await
        {
            Ok(presigned) => presigned,
            Err(e) => {
                tracing::error!("Failed to generate presigned URL: {:?}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(V1ErrorResponse::internal_error(
                        "Failed to generate upload URL",
                    )),
                )
                    .into_response();
            }
        }
    } else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(V1ErrorResponse::internal_error(
                "R2 storage is not configured",
            )),
        )
            .into_response();
    };

    (
        StatusCode::CREATED,
        Json(V1UploadsInitResponse {
            upload_id: upload.id.to_string(),
            storage_key,
            method: "PUT".to_owned(),
            upload_url: presigned.url,
            expires_at: presigned.expires_at.to_rfc3339(),
        }),
    )
        .into_response()
}

/// Complete an upload after the file has been uploaded to R2.
///
/// This endpoint verifies the file exists in R2, creates the content record,
/// and returns the created content.
#[utoipa::path(
    post,
    path = "/v1/uploads/complete",
    tag = "uploads",
    request_body = V1UploadsCompleteRequest,
    responses(
        (status = 201, description = "Upload completed", body = V1UploadsCompleteResponse),
        (status = 400, description = "Bad request", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 404, description = "Upload not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_uploads_complete<S, U>(
    State(state): State<AppState<S, U>>,
    presigner: Option<axum::Extension<R2Presigner>>,
    auth: RequireAuth,
    Json(payload): Json<V1UploadsCompleteRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Resolve user from JWT username
    let user = match state.user_storage.get_user(auth.username()).await {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(V1ErrorResponse::not_found("User not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get user")),
            )
                .into_response();
        }
    };

    // Parse upload ID
    let upload_id = match uuid::Uuid::parse_str(&payload.upload_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid upload ID format")),
            )
                .into_response();
        }
    };

    // Get upload record
    let upload = match state.sql_storage.uploads_get(upload_id).await {
        Ok(Some(upload)) => upload,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Upload not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get upload: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get upload")),
            )
                .into_response();
        }
    };

    // Verify ownership
    if upload.user_id != user.id {
        return (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Upload not found")),
        )
            .into_response();
    }

    // Check upload status
    if upload.status != "initiated" {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request(format!(
                "Upload is already {}",
                upload.status
            ))),
        )
            .into_response();
    }

    // Check if upload has expired
    if upload.expires_at < chrono::Utc::now() {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Upload has expired")),
        )
            .into_response();
    }

    // Verify file exists in R2 via HEAD request
    let file_exists = if let Some(axum::Extension(ref presigner)) = presigner {
        match presigner.file_exists(&upload.storage_key).await {
            Ok(exists) => exists,
            Err(e) => {
                tracing::error!("Failed to verify file existence: {:?}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(V1ErrorResponse::internal_error(
                        "Failed to verify upload completion",
                    )),
                )
                    .into_response();
            }
        }
    } else {
        return (
            StatusCode::BAD_GATEWAY,
            Json(V1ErrorResponse::internal_error(
                "R2 storage is not configured",
            )),
        )
            .into_response();
    };

    if !file_exists {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request(
                "File not found in storage. Please upload the file first.",
            )),
        )
            .into_response();
    }

    // Mark upload as completed
    match state.sql_storage.uploads_complete(upload_id, user.id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Upload not found")),
            )
                .into_response();
        }
        Err(SqlStorageError::Invalid(msg)) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(msg)),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to complete upload: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to complete upload")),
            )
                .into_response();
        }
    }

    // Create content record
    let title = payload.title.unwrap_or_else(|| {
        // Extract filename from storage key
        upload
            .storage_key
            .split('/')
            .next_back()
            .unwrap_or("Untitled")
            .to_owned()
    });

    let content_input = ContentsInsert {
        user_id: user.id,
        title,
        description: payload.description,
        storage_backend: upload.storage_backend,
        storage_profile: upload.storage_profile,
        storage_key: upload.storage_key,
        content_type: upload.content_type,
        file_size: upload.file_size,
        visibility: Visibility::Private,
        kind: None, // Defaults to "file" for uploaded content
        body: None, // No inline body for uploaded files
    };

    let content = match state.sql_storage.contents_insert(content_input).await {
        Ok(content) => content,
        Err(e) => {
            tracing::error!("Failed to create content: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to create content")),
            )
                .into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(V1UploadsCompleteResponse {
            content: V1ContentItem::from(content),
        }),
    )
        .into_response()
}
