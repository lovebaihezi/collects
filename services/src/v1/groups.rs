//! Handlers for `/v1/groups/*` endpoints.

use crate::database::{
    ContentGroupItemRow, ContentGroupRow, GroupCreate, GroupStatus, GroupUpdate, GroupsListParams,
    SqlStorage, SqlStorageError, Visibility,
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

/// Query parameters for listing groups.
#[derive(Debug, Deserialize, Default)]
pub struct V1GroupsListQuery {
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

/// A group item in API responses.
#[derive(Debug, Serialize)]
pub struct V1GroupItem {
    pub id: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub visibility: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trashed_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl From<ContentGroupRow> for V1GroupItem {
    fn from(row: ContentGroupRow) -> Self {
        Self {
            id: row.id.to_string(),
            name: row.name,
            description: row.description,
            visibility: row.visibility,
            status: row.status,
            trashed_at: row.trashed_at.map(|t| t.to_rfc3339()),
            archived_at: row.archived_at.map(|t| t.to_rfc3339()),
            created_at: row.created_at.to_rfc3339(),
            updated_at: row.updated_at.to_rfc3339(),
        }
    }
}

/// Response for listing groups.
#[derive(Debug, Serialize)]
pub struct V1GroupsListResponse {
    pub items: Vec<V1GroupItem>,
    pub total: usize,
}

/// Request body for creating a group.
#[derive(Debug, Deserialize)]
pub struct V1GroupCreateRequest {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub visibility: Option<String>,
}

/// Request body for updating a group.
#[derive(Debug, Deserialize)]
pub struct V1GroupUpdateRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub description: Option<Option<String>>,
    #[serde(default)]
    pub visibility: Option<String>,
}

/// Request body for adding content to a group.
#[derive(Debug, Deserialize)]
pub struct V1GroupContentsAddRequest {
    pub content_id: String,
    #[serde(default)]
    pub sort_order: Option<i32>,
}

/// A group content item in API responses.
#[derive(Debug, Serialize)]
pub struct V1GroupContentItem {
    pub id: String,
    pub group_id: String,
    pub content_id: String,
    pub sort_order: i32,
    pub added_at: String,
}

impl From<ContentGroupItemRow> for V1GroupContentItem {
    fn from(row: ContentGroupItemRow) -> Self {
        Self {
            id: row.id.to_string(),
            group_id: row.group_id.to_string(),
            content_id: row.content_id.to_string(),
            sort_order: row.sort_order,
            added_at: row.added_at.to_rfc3339(),
        }
    }
}

/// Response for listing group contents.
#[derive(Debug, Serialize)]
pub struct V1GroupContentsListResponse {
    pub items: Vec<V1GroupContentItem>,
    pub total: usize,
}

// =============================================================================
// Group Handlers
// =============================================================================

/// List groups for the authenticated user.
///
/// GET /v1/groups
pub async fn list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Query(query): Query<V1GroupsListQuery>,
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
        "active" => Some(GroupStatus::Active),
        "archived" => Some(GroupStatus::Archived),
        "trashed" => Some(GroupStatus::Trashed),
        _ => None,
    });

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let params = GroupsListParams {
        limit,
        offset,
        status,
    };

    match state
        .sql_storage
        .groups_list_for_user(user.id, params)
        .await
    {
        Ok(rows) => {
            let items: Vec<V1GroupItem> = rows.into_iter().map(V1GroupItem::from).collect();
            let total = items.len();
            (StatusCode::OK, Json(V1GroupsListResponse { items, total })).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list groups: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to list groups")),
            )
                .into_response()
        }
    }
}

/// Create a new group.
///
/// POST /v1/groups
pub async fn create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(payload): Json<V1GroupCreateRequest>,
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
            Json(V1ErrorResponse::bad_request("Name cannot be empty")),
        )
            .into_response();
    }

    // Parse visibility
    let visibility = match payload.visibility.as_deref() {
        Some("private") | None => Visibility::Private,
        Some("public") => Visibility::Public,
        Some("restricted") => Visibility::Restricted,
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
    };

    let input = GroupCreate {
        user_id: user.id,
        name: payload.name,
        description: payload.description,
        visibility,
    };

    match state.sql_storage.groups_create(input).await {
        Ok(row) => (StatusCode::CREATED, Json(V1GroupItem::from(row))).into_response(),
        Err(e) => {
            tracing::error!("Failed to create group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to create group")),
            )
                .into_response()
        }
    }
}

/// Get a group by ID.
///
/// GET /v1/groups/:id
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

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    match state.sql_storage.groups_get(group_id).await {
        Ok(Some(row)) => {
            // Verify user owns the group
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Group not found")),
                )
                    .into_response();
            }
            (StatusCode::OK, Json(V1GroupItem::from(row))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Group not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get group")),
            )
                .into_response()
        }
    }
}

/// Update a group's metadata.
///
/// PATCH /v1/groups/:id
pub async fn update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1GroupUpdateRequest>,
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

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
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

    let changes = GroupUpdate {
        name: payload.name,
        description: payload.description,
        visibility,
    };

    match state
        .sql_storage
        .groups_update_metadata(group_id, user.id, changes)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1GroupItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Group not found")),
        )
            .into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(V1ErrorResponse::forbidden(
                "You do not have permission to update this group",
            )),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to update group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to update group")),
            )
                .into_response()
        }
    }
}

/// Trash a group.
///
/// POST /v1/groups/:id/trash
pub async fn trash<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    set_status(state, auth, id, GroupStatus::Trashed).await
}

/// Restore a group from trash.
///
/// POST /v1/groups/:id/restore
pub async fn restore<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    set_status(state, auth, id, GroupStatus::Active).await
}

/// Archive a group.
///
/// POST /v1/groups/:id/archive
pub async fn archive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    set_status(state, auth, id, GroupStatus::Archived).await
}

/// Unarchive a group.
///
/// POST /v1/groups/:id/unarchive
pub async fn unarchive<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    set_status(state, auth, id, GroupStatus::Active).await
}

/// Helper function to set group status.
async fn set_status<S, U>(
    state: AppState<S, U>,
    auth: RequireAuth,
    id: String,
    new_status: GroupStatus,
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

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    let now = chrono::Utc::now();

    match state
        .sql_storage
        .groups_set_status(group_id, user.id, new_status, now)
        .await
    {
        Ok(Some(row)) => (StatusCode::OK, Json(V1GroupItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Group not found")),
        )
            .into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::FORBIDDEN,
            Json(V1ErrorResponse::forbidden(
                "You do not have permission to modify this group",
            )),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to set group status: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to update group status",
                )),
            )
                .into_response()
        }
    }
}

// =============================================================================
// Group Contents Handlers
// =============================================================================

/// List contents in a group.
///
/// GET /v1/groups/:id/contents
pub async fn contents_list<S, U>(
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

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the group
    match state.sql_storage.groups_get(group_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Group not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Group not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get group: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get group")),
            )
                .into_response();
        }
    }

    match state.sql_storage.group_items_list(group_id).await {
        Ok(rows) => {
            let items: Vec<V1GroupContentItem> =
                rows.into_iter().map(V1GroupContentItem::from).collect();
            let total = items.len();
            (
                StatusCode::OK,
                Json(V1GroupContentsListResponse { items, total }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list group contents: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to list group contents",
                )),
            )
                .into_response()
        }
    }
}

/// Add content to a group.
///
/// POST /v1/groups/:id/contents
pub async fn contents_add<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1GroupContentsAddRequest>,
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

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&payload.content_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the group
    match state.sql_storage.groups_get(group_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Group not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Group not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get group: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get group")),
            )
                .into_response();
        }
    }

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

    let sort_order = payload.sort_order.unwrap_or(0);

    match state
        .sql_storage
        .group_items_add(group_id, content_id, sort_order)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            tracing::error!("Failed to add content to group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to add content to group",
                )),
            )
                .into_response()
        }
    }
}

/// Remove content from a group.
///
/// DELETE /v1/groups/:id/contents/:content_id
pub async fn contents_remove<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path((id, content_id_str)): Path<(String, String)>,
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

    // Parse group ID
    let group_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    // Parse content ID
    let content_id = match uuid::Uuid::parse_str(&content_id_str) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Verify user owns the group
    match state.sql_storage.groups_get(group_id).await {
        Ok(Some(row)) => {
            if row.user_id != user.id {
                return (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Group not found")),
                )
                    .into_response();
            }
        }
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(V1ErrorResponse::not_found("Group not found")),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get group: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get group")),
            )
                .into_response();
        }
    }

    match state
        .sql_storage
        .group_items_remove(group_id, content_id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Content not in group")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to remove content from group: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to remove content from group",
                )),
            )
                .into_response()
        }
    }
}
