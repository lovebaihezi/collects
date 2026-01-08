//! /v1/tags endpoint handlers.

use crate::database::{self, SqlStorage, SqlStorageError};
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use super::types::{
    V1ErrorResponse, V1TagCreateRequest, V1TagItem, V1TagUpdateRequest, V1TagsListResponse,
};

/// List tags for the authenticated user.
///
/// GET /v1/tags
pub async fn v1_tags_list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
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
pub async fn v1_tags_create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(request): Json<V1TagCreateRequest>,
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

    // Validate tag name
    let name = request.name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Tag name cannot be empty")),
        )
            .into_response();
    }

    let input = database::TagCreate {
        user_id: user.id,
        name: name.to_string(),
        color: request.color,
    };

    match state.sql_storage.tags_create(input).await {
        Ok(row) => (StatusCode::CREATED, Json(V1TagItem::from(row))).into_response(),
        Err(SqlStorageError::Conflict) => (
            StatusCode::CONFLICT,
            Json(V1ErrorResponse {
                error: "conflict".to_string(),
                message: "A tag with this name already exists".to_string(),
            }),
        )
            .into_response(),
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
pub async fn v1_tags_update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(request): Json<V1TagUpdateRequest>,
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

    // Validate name if provided
    if let Some(ref name) = request.name
        && name.trim().is_empty()
    {
        return (
            StatusCode::BAD_REQUEST,
            Json(V1ErrorResponse::bad_request("Tag name cannot be empty")),
        )
            .into_response();
    }

    let input = database::TagUpdate {
        name: request.name.map(|n| n.trim().to_string()),
        color: request.color,
    };

    match state.sql_storage.tags_update(user.id, tag_id, input).await {
        Ok(Some(row)) => (StatusCode::OK, Json(V1TagItem::from(row))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Tag not found")),
        )
            .into_response(),
        Err(SqlStorageError::Conflict) => (
            StatusCode::CONFLICT,
            Json(V1ErrorResponse {
                error: "conflict".to_string(),
                message: "A tag with this name already exists".to_string(),
            }),
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
pub async fn v1_tags_delete<S, U>(
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
