//! Handlers for `/v1/contents/*` endpoints.

use crate::database::{
    ContentRow, ContentStatus, ContentsListParams, ContentsUpdate, SqlStorage, SqlStorageError,
    Visibility,
};
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use crate::v1::types::V1ErrorResponse;
use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

// =============================================================================
// Types
// =============================================================================

/// Query parameters for listing contents.
#[derive(Debug, Deserialize, Default)]
pub struct V1ContentsListQuery {
    /// Maximum number of results to return (default: 50, max: 100)
    #[serde(default)]
    pub limit: Option<i64>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: Option<i64>,
    /// Filter by status: active, archived, trashed
    #[serde(default)]
    pub status: Option<String>,
}

/// A content item in API responses.
#[derive(Debug, Serialize)]
pub struct V1ContentItem {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub storage_backend: String,
    pub storage_profile: String,
    pub storage_key: String,
    pub content_type: String,
    pub file_size: i64,
    pub status: String,
    pub visibility: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ContentRow> for V1ContentItem {
    fn from(row: ContentRow) -> Self {
        Self {
            id: row.id.to_string(),
            title: row.title,
            description: row.description,
            storage_backend: row.storage_backend,
            storage_profile: row.storage_profile,
            storage_key: row.storage_key,
            content_type: row.content_type,
            file_size: row.file_size,
            status: row.status,
            visibility: row.visibility,
            trashed_at: row.trashed_at.map(|t| t.to_rfc3339()),
            archived_at: row.archived_at.map(|t| t.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

/// Response for listing contents.
#[derive(Debug, Serialize)]
pub struct V1ContentsListResponse {
    pub items: Vec<V1ContentItem>,
    pub total: usize,
}

/// Request body for updating content metadata.
#[derive(Debug, Deserialize)]
pub struct V1ContentsUpdateRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub visibility: Option<String>,
}

// =============================================================================
// Handlers
// =============================================================================

/// List contents for the authenticated user.
///
/// GET /v1/contents?limit=50&offset=0&status=active
pub async fn list<S, U>(
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
pub async fn get<S, U>(
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
pub async fn update<S, U>(
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
        Some("private") => Some(Visibility::Private),
        Some("public") => Some(Visibility::Public),
        Some("restricted") => Some(Visibility::Restricted),
        Some(v) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(format!(
                    "Invalid visibility: {}",
                    v
                ))),
            )
                .into_response();
        }
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
            Json(V1ErrorResponse::forbidden(
                "You do not have permission to update this content",
            )),
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

/// Trash a content item.
///
/// POST /v1/contents/:id/trash
pub async fn trash<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    set_status(state, auth, id, ContentStatus::Trashed).await
}

/// Restore a content item from trash.
///
/// POST /v1/contents/:id/restore
pub async fn restore<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    set_status(state, auth, id, ContentStatus::Active).await
}

/// Archive a content item.
///
/// POST /v1/contents/:id/archive
pub async fn archive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    set_status(state, auth, id, ContentStatus::Archived).await
}

/// Unarchive a content item.
///
/// POST /v1/contents/:id/unarchive
pub async fn unarchive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    set_status(state, auth, id, ContentStatus::Active).await
}

/// Helper function to set content status.
async fn set_status<S, U>(
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
            Json(V1ErrorResponse::forbidden(
                "You do not have permission to modify this content",
            )),
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
