//! /v1/groups endpoint handlers.

use crate::database::{self, SqlStorage, SqlStorageError};
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
    V1ErrorResponse, V1GroupAddContentRequest, V1GroupContentItem, V1GroupContentsListResponse,
    V1GroupCreateRequest, V1GroupItem, V1GroupReorderRequest, V1GroupUpdateRequest,
    V1GroupsListQuery, V1GroupsListResponse,
};

/// List groups for the authenticated user.
#[utoipa::path(
    get,
    path = "/v1/groups",
    tag = "groups",
    params(V1GroupsListQuery),
    responses(
        (status = 200, description = "List of groups", body = V1GroupsListResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_list<S, U>(
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
        "active" => Some(database::GroupStatus::Active),
        "archived" => Some(database::GroupStatus::Archived),
        "trashed" => Some(database::GroupStatus::Trashed),
        _ => None,
    });

    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0).max(0);

    let params = database::GroupsListParams {
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
#[utoipa::path(
    post,
    path = "/v1/groups",
    tag = "groups",
    request_body = V1GroupCreateRequest,
    responses(
        (status = 201, description = "Group created", body = V1GroupItem),
        (status = 400, description = "Bad request", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(request): Json<V1GroupCreateRequest>,
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

    // Validate group name
    let name = request.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Group name cannot be empty")),
        )
            .into_response();
    }

    // Parse visibility
    let visibility = match request.visibility.as_str() {
        "private" => database::Visibility::Private,
        "public" => database::Visibility::Public,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(
                    "Invalid visibility. Must be 'private' or 'public'",
                )),
            )
                .into_response();
        }
    };

    let input = database::GroupCreate {
        user_id: user.id,
        name: name.to_owned(),
        description: request.description,
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

/// Get a specific group by ID.
#[utoipa::path(
    get,
    path = "/v1/groups/{id}",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    responses(
        (status = 200, description = "Group item", body = V1GroupItem),
        (status = 400, description = "Invalid group ID format", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 404, description = "Group not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_get<S, U>(
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
            // Verify ownership
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

/// Update a group.
#[utoipa::path(
    patch,
    path = "/v1/groups/{id}",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    request_body = V1GroupUpdateRequest,
    responses(
        (status = 200, description = "Group updated", body = V1GroupItem),
        (status = 400, description = "Bad request", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 404, description = "Group not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(request): Json<V1GroupUpdateRequest>,
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

    // Validate name if provided
    if let Some(ref name) = request.name
        && name.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Group name cannot be empty")),
        )
            .into_response();
    }

    // Parse visibility if provided
    let visibility = match &request.visibility {
        Some(v) => match v.as_str() {
            "private" => Some(database::Visibility::Private),
            "public" => Some(database::Visibility::Public),
            _ => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(
                        "Invalid visibility. Must be 'private' or 'public'",
                    )),
                )
                    .into_response();
            }
        },
        None => None,
    };

    let changes = database::GroupUpdate {
        name: request.name.map(|n| n.trim().to_owned()),
        description: request.description,
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

/// Helper to set group status.
async fn v1_groups_set_status<S, U>(
    state: &AppState<S, U>,
    user_id: uuid::Uuid,
    group_id: uuid::Uuid,
    new_status: database::GroupStatus,
) -> axum::response::Response
where
    S: SqlStorage,
    U: UserStorage,
{
    let now = chrono::Utc::now();
    match state
        .sql_storage
        .groups_set_status(group_id, user_id, new_status, now)
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
            Json(V1ErrorResponse {
                error: "forbidden".to_owned(),
                message: "You do not have permission to modify this group".to_owned(),
            }),
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

/// Trash a group.
#[utoipa::path(
    post,
    path = "/v1/groups/{id}/trash",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    responses(
        (status = 200, description = "Group trashed", body = V1GroupItem),
        (status = 400, description = "Invalid group ID format", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 403, description = "Forbidden", body = V1ErrorResponse),
        (status = 404, description = "Group not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_trash<S, U>(
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

    v1_groups_set_status(&state, user.id, group_id, database::GroupStatus::Trashed).await
}

/// Restore a group from trash.
#[utoipa::path(
    post,
    path = "/v1/groups/{id}/restore",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    responses(
        (status = 200, description = "Group restored", body = V1GroupItem),
        (status = 400, description = "Invalid group ID format", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 403, description = "Forbidden", body = V1ErrorResponse),
        (status = 404, description = "Group not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_restore<S, U>(
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

    v1_groups_set_status(&state, user.id, group_id, database::GroupStatus::Active).await
}

/// Archive a group.
#[utoipa::path(
    post,
    path = "/v1/groups/{id}/archive",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    responses(
        (status = 200, description = "Group archived", body = V1GroupItem),
        (status = 400, description = "Invalid group ID format", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 403, description = "Forbidden", body = V1ErrorResponse),
        (status = 404, description = "Group not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_archive<S, U>(
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

    v1_groups_set_status(&state, user.id, group_id, database::GroupStatus::Archived).await
}

/// Unarchive a group.
#[utoipa::path(
    post,
    path = "/v1/groups/{id}/unarchive",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    responses(
        (status = 200, description = "Group unarchived", body = V1GroupItem),
        (status = 400, description = "Invalid group ID format", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 403, description = "Forbidden", body = V1ErrorResponse),
        (status = 404, description = "Group not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_unarchive<S, U>(
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

    v1_groups_set_status(&state, user.id, group_id, database::GroupStatus::Active).await
}

/// List contents in a group.
#[utoipa::path(
    get,
    path = "/v1/groups/{id}/contents",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    responses(
        (status = 200, description = "List of group contents", body = V1GroupContentsListResponse),
        (status = 400, description = "Invalid group ID format", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 404, description = "Group not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_contents_list<S, U>(
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
#[utoipa::path(
    post,
    path = "/v1/groups/{id}/contents",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    request_body = V1GroupAddContentRequest,
    responses(
        (status = 204, description = "Content added to group"),
        (status = 400, description = "Bad request", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 404, description = "Group or content not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_contents_add<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(request): Json<V1GroupAddContentRequest>,
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
    let content_id = match uuid::Uuid::parse_str(&request.content_id) {
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

    let sort_order = request.sort_order.unwrap_or(0);

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
#[utoipa::path(
    delete,
    path = "/v1/groups/{id}/contents/{content_id}",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)"),
        ("content_id" = String, Path, description = "Content ID (UUID)")
    ),
    responses(
        (status = 204, description = "Content removed from group"),
        (status = 400, description = "Bad request", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 404, description = "Group or content not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_contents_remove<S, U>(
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

/// Reorder contents in a group.
#[utoipa::path(
    patch,
    path = "/v1/groups/{id}/contents/reorder",
    tag = "groups",
    params(
        ("id" = String, Path, description = "Group ID (UUID)")
    ),
    request_body = V1GroupReorderRequest,
    responses(
        (status = 204, description = "Group contents reordered"),
        (status = 400, description = "Bad request", body = V1ErrorResponse),
        (status = 401, description = "Unauthorized", body = V1ErrorResponse),
        (status = 404, description = "Group not found", body = V1ErrorResponse),
        (status = 500, description = "Internal server error", body = V1ErrorResponse),
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn v1_groups_contents_reorder<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(request): Json<V1GroupReorderRequest>,
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

    // Parse and collect content IDs with sort orders
    let mut items: Vec<(uuid::Uuid, i32)> = Vec::with_capacity(request.items.len());
    for item in &request.items {
        match uuid::Uuid::parse_str(&item.content_id) {
            Ok(content_id) => items.push((content_id, item.sort_order)),
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(format!(
                        "Invalid content ID format: {}",
                        item.content_id
                    ))),
                )
                    .into_response();
            }
        }
    }

    match state
        .sql_storage
        .group_items_reorder(group_id, user.id, &items)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(SqlStorageError::Unauthorized) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Group not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to reorder group contents: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to reorder group contents",
                )),
            )
                .into_response()
        }
    }
}
