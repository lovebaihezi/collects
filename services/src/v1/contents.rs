//! /v1/contents endpoint handlers.

use crate::database::{
    ContentStatus, ContentsListParams, ContentsUpdate, SqlStorage, SqlStorageError,
};
use crate::storage::{ContentDisposition, DEFAULT_PRESIGN_EXPIRY, R2Presigner};
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};

use super::types::{
    V1ContentItem, V1ContentsListQuery, V1ContentsListResponse, V1ContentsUpdateRequest,
    V1ErrorResponse, V1ViewUrlRequest, V1ViewUrlResponse, parse_visibility,
};

/// List contents for the authenticated user.
///
/// GET /v1/contents?limit=50&offset=0&status=active
pub async fn v1_contents_list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Query(query): Query<V1ContentsListQuery>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
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

    // Parse status filter
    let status = query.status.as_deref().and_then(|s| match s {
        "active" => Some(ContentStatus::Active),
        "archived" => Some(ContentStatus::Archived),
        "trashed" => Some(ContentStatus::Trashed),
        _ => None,
    });

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let params = ContentsListParams {
        limit,
        offset,
        status,
    };

    match state
        .sql_storage
        .contents_list_for_user(user.id, params)
        .await
    {
        Ok(rows) => {
            let items: Vec<V1ContentItem> = rows.into_iter().map(V1ContentItem::from).collect();
            let total = items.len();
            (
                StatusCode::OK,
                Json(V1ContentsListResponse { items, total }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list contents: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to list contents")),
            )
                .into_response()
        }
    }
}

/// Get a specific content item by ID.
///
/// GET /v1/contents/:id
pub async fn v1_contents_get<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
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

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            // Verify ownership
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(V1ContentItem::from(row))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get content: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get content")),
            )
                .into_response()
        }
    }
}

/// Update content metadata.
///
/// PATCH /v1/contents/:id
pub async fn v1_contents_update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1ContentsUpdateRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
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

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Parse visibility if provided
    let visibility = match payload.visibility.as_deref() {
        Some(v) => match parse_visibility(v) {
            Some(vis) => Some(vis),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(format!(
                        "Invalid visibility: {}",
                        v
                    ))),
                )
                    .into_response();
            }
        },
        None => None,
    };

    let changes = ContentsUpdate {
        title: payload.title,
        description: payload.description,
        visibility,
    };

    match state
        .sql_storage
        .contents_update_metadata(content_id, user.id, changes)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1ContentItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not found")),
        )
            .into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(V1ErrorResponse {
                error: "forbidden".to_string(),
                message: "You do not have permission to update this content".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to update content: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to update content")),
            )
                .into_response()
        }
    }
}

/// Move content to trash.
///
/// POST /v1/contents/:id/trash
pub async fn v1_contents_trash<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_contents_set_status::<S, U>(state, auth, id, ContentStatus::Trashed).await
}

/// Restore content from trash.
///
/// POST /v1/contents/:id/restore
pub async fn v1_contents_restore<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_contents_set_status::<S, U>(state, auth, id, ContentStatus::Active).await
}

/// Archive content.
///
/// POST /v1/contents/:id/archive
pub async fn v1_contents_archive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_contents_set_status::<S, U>(state, auth, id, ContentStatus::Archived).await
}

/// Unarchive content (restore to active).
///
/// POST /v1/contents/:id/unarchive
pub async fn v1_contents_unarchive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    v1_contents_set_status::<S, U>(state, auth, id, ContentStatus::Active).await
}

/// Helper function to set content status.
async fn v1_contents_set_status<S, U>(
    state: AppState<S, U>,
    auth: RequireAuth,
    id: String,
    new_status: ContentStatus,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Get user ID from username
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

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    let now = chrono::Utc::now();

    match state
        .sql_storage
        .contents_set_status(content_id, user.id, new_status, now)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1ContentItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not found")),
        )
            .into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(V1ErrorResponse {
                error: "forbidden".to_string(),
                message: "You do not have permission to modify this content".to_string(),
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to set content status: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to update content status",
                )),
            )
                .into_response()
        }
    }
}

/// Get a view URL for content.
///
/// POST /v1/contents/:id/view-url
///
/// This endpoint generates a presigned GET URL for viewing/downloading content
/// from R2 storage. The URL is valid for 15 minutes by default.
pub async fn v1_contents_view_url<S, U>(
    State(state): State<AppState<S, U>>,
    presigner: Option<axum::Extension<R2Presigner>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1ViewUrlRequest>,
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

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Get content and verify ownership
    let content = match state.sql_storage.contents_get(content_id).await {
        Ok(Some(content)) => content,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Content not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get content: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get content")),
            )
                .into_response();
        }
    };

    // Verify ownership
    if content.user_id != user.id {
        return (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not found")),
        )
            .into_response();
    }

    // Parse disposition
    let disposition = match ContentDisposition::try_from(payload.disposition.as_str()) {
        Ok(d) => d,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(
                    "Invalid disposition. Must be 'inline' or 'attachment'",
                )),
            )
                .into_response();
        }
    };

    // Generate presigned GET URL
    let presigned = if let Some(axum::Extension(presigner)) = presigner {
        match presigner
            .presign_get(&content.storage_key, disposition, DEFAULT_PRESIGN_EXPIRY)
            .await
        {
            Ok(presigned) => presigned,
            Err(e) => {
                tracing::error!("Failed to generate presigned URL: {:?}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(V1ErrorResponse::internal_error(
                        "Failed to generate view URL",
                    )),
                )
                    .into_response();
            }
        }
    } else {
        // Test mode: return mock URL
        let expires_at =
            chrono::Utc::now() + chrono::Duration::from_std(DEFAULT_PRESIGN_EXPIRY).unwrap();
        let disp = match disposition {
            ContentDisposition::Inline => "inline",
            ContentDisposition::Attachment => "attachment",
        };
        crate::storage::PresignedUrl {
            url: format!(
                "https://test.r2.example.com/{}?mock=true&disposition={}",
                content.storage_key, disp
            ),
            expires_at,
        }
    };

    (
        StatusCode::OK,
        Json(V1ViewUrlResponse {
            url: presigned.url,
            expires_at: presigned.expires_at.to_rfc3339(),
        }),
    )
        .into_response()
}
