//! Handlers for `/v1/tags/*` and `/v1/contents/:id/tags/*` endpoints.

use crate::database::{SqlStorage, TagCreate, TagRow, TagUpdate};
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use crate::v1::types::V1ErrorResponse;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

// =============================================================================
// Types
// =============================================================================

/// A tag item in API responses.
#[derive(Debug, Serialize)]
pub struct V1TagItem {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    pub created_at: String,
}

impl From<TagRow> for V1TagItem {
    fn from(row: TagRow) -> Self {
        Self {
            id: row.id.to_string(),
            name: row.name,
            color: row.color,
            created_at: row.created_at.to_rfc3339(),
        }
    }
}

/// Response for listing tags.
#[derive(Debug, Serialize)]
pub struct V1TagsListResponse {
    pub items: Vec<V1TagItem>,
    pub total: usize,
}

/// Request body for creating a tag.
#[derive(Debug, Deserialize)]
pub struct V1TagCreateRequest {
    pub name: String,
    #[serde(default)]
    pub color: Option<String>,
}

/// Request body for updating a tag.
#[derive(Debug, Deserialize)]
pub struct V1TagUpdateRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub color: Option<Option<String>>,
}

/// Request body for attaching tags to content.
#[derive(Debug, Deserialize)]
pub struct V1ContentTagsAttachRequest {
    pub tag_id: String,
}

// =============================================================================
// Tag Handlers
// =============================================================================

/// List tags for the authenticated user.
///
/// GET /v1/tags
pub async fn list<S, U>(State(state): State<AppState<S, U>>, auth: RequireAuth) -> impl IntoResponse
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

    match state.sql_storage.tags_list_for_user(user.id).await {
        Ok(rows) => {
            let items: Vec<V1TagItem> = rows.into_iter().map(V1TagItem::from).collect();
            let total = items.len();
            (StatusCode::OK, Json(V1TagsListResponse { items, total })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list tags: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to list tags")),
            )
                .into_response()
        }
    }
}

/// Create a new tag.
///
/// POST /v1/tags
pub async fn create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(payload): Json<V1TagCreateRequest>,
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

    // Validate name
    if payload.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Tag name cannot be empty")),
        )
            .into_response();
    }

    let input = TagCreate {
        user_id: user.id,
        name: payload.name,
        color: payload.color,
    };

    match state.sql_storage.tags_create(input).await {
        Ok(row) => (StatusCode::CREATED, Json(V1TagItem::from(row))).into_response(),
        Err(e) => {
            tracing::error!("Failed to create tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to create tag")),
            )
                .into_response()
        }
    }
}

/// Update a tag.
///
/// PATCH /v1/tags/:id
pub async fn update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1TagUpdateRequest>,
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

    // Parse tag ID
    let tag_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid tag ID format")),
            )
                .into_response();
        }
    };

    let changes = TagUpdate {
        name: payload.name,
        color: payload.color,
    };

    match state
        .sql_storage
        .tags_update(user.id, tag_id, changes)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1TagItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Tag not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to update tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to update tag")),
            )
                .into_response()
        }
    }
}

/// Delete a tag.
///
/// DELETE /v1/tags/:id
pub async fn delete<S, U>(
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

    // Parse tag ID
    let tag_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid tag ID format")),
            )
                .into_response();
        }
    };

    match state.sql_storage.tags_delete(user.id, tag_id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Tag not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to delete tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to delete tag")),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Content-Tags Handlers
// =============================================================================

/// List tags for a content item.
///
/// GET /v1/contents/:id/tags
pub async fn list_for_content<S, U>(
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

    // Verify user owns the content
    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
        }
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
    }

    match state
        .sql_storage
        .content_tags_list_for_content(content_id)
        .await
    {
        Ok(rows) => {
            let items: Vec<V1TagItem> = rows.into_iter().map(V1TagItem::from).collect();
            let total = items.len();
            (StatusCode::OK, Json(V1TagsListResponse { items, total })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list tags for content: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to list tags")),
            )
                .into_response()
        }
    }
}

/// Attach a tag to a content item.
///
/// POST /v1/contents/:id/tags
pub async fn attach<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1ContentTagsAttachRequest>,
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

    // Parse tag ID
    let tag_id = match uuid::Uuid::parse_str(&payload.tag_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid tag ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the content
    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
        }
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
    }

    match state
        .sql_storage
        .content_tags_attach(content_id, tag_id)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to attach tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to attach tag")),
            )
                .into_response()
        }
    }
}

/// Detach a tag from a content item.
///
/// DELETE /v1/contents/:id/tags/:tag_id
pub async fn detach<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path((id, tag_id_str)): Path<(String, String)>,
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

    // Parse tag ID
    let tag_id = match uuid::Uuid::parse_str(&tag_id_str) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid tag ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the content
    match state.sql_storage.contents_get(content_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Content not found")),
                )
                    .into_response();
            }
        }
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
    }

    match state
        .sql_storage
        .content_tags_detach(content_id, tag_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Tag not attached to content")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to detach tag: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to detach tag")),
            )
                .into_response()
        }
    }
}
