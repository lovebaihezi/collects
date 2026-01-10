//! /v1/share-links endpoint handlers.

use crate::database::{ShareLinkCreate, ShareLinkUpdate, SqlStorage};
use crate::users::routes::AppState;
use crate::users::session_auth::RequireAuth;
use crate::users::storage::UserStorage;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use sha2::{Digest, Sha256};

use super::types::{
    V1ContentShareLinkCreateRequest, V1ErrorResponse, V1GroupShareLinkCreateRequest,
    V1ShareLinkCreateRequest, V1ShareLinkResponse, V1ShareLinkUpdateRequest,
    V1ShareLinksListResponse, parse_share_permission,
};

/// Length of generated share tokens (nanoid).
const SHARE_TOKEN_LENGTH: usize = 21;

/// Default base URL for share links (can be overridden via config in future).
const DEFAULT_SHARE_BASE_URL: &str = "https://app.collects.io";

/// Hash a password using SHA256 (simple hashing for share link passwords).
/// Note: For user passwords, we'd use bcrypt, but share link passwords
/// are typically shorter-lived and this is simpler.
fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Verify a password against a hash.
pub fn verify_password(password: &str, hash: &str) -> bool {
    hash_password(password) == hash
}

/// Generate a unique share token using nanoid.
fn generate_share_token() -> String {
    nanoid::nanoid!(SHARE_TOKEN_LENGTH)
}

/// Parse an ISO 8601 datetime string.
fn parse_datetime(s: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc))
}

/// List share links for the authenticated user.
///
/// GET /v1/share-links
pub async fn v1_share_links_list<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
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

    match state.sql_storage.share_links_list_for_owner(user.id).await {
        Ok(rows) => {
            let share_links: Vec<V1ShareLinkResponse> = rows
                .into_iter()
                .map(|row| V1ShareLinkResponse::from_row(row, DEFAULT_SHARE_BASE_URL))
                .collect();
            (
                StatusCode::OK,
                Json(V1ShareLinksListResponse { share_links }),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list share links: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to list share links",
                )),
            )
                .into_response()
        }
    }
}

/// Create a new share link.
///
/// POST /v1/share-links
pub async fn v1_share_links_create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Json(payload): Json<V1ShareLinkCreateRequest>,
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

    // Parse permission
    let permission = match parse_share_permission(&payload.permission) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(format!(
                    "Invalid permission: {}. Must be 'view' or 'download'",
                    payload.permission
                ))),
            )
                .into_response();
        }
    };

    // Parse expiration date if provided
    let expires_at = if let Some(ref expires_str) = payload.expires_at {
        match parse_datetime(expires_str) {
            Some(dt) => Some(dt),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(
                        "Invalid expires_at format. Use ISO 8601 (e.g., 2024-12-31T23:59:59Z)",
                    )),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    // Hash password if provided
    let password_hash = payload.password.as_ref().map(|p| hash_password(p));

    // Generate unique token
    let token = generate_share_token();

    let input = ShareLinkCreate {
        owner_id: user.id,
        token,
        name: payload.name,
        permission,
        password_hash,
        max_access_count: payload.max_access_count,
        expires_at,
    };

    match state.sql_storage.share_links_create(input).await {
        Ok(row) => (
            StatusCode::CREATED,
            Json(V1ShareLinkResponse::from_row(row, DEFAULT_SHARE_BASE_URL)),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to create share link: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to create share link",
                )),
            )
                .into_response()
        }
    }
}

/// Get a specific share link by ID.
///
/// GET /v1/share-links/{id}
pub async fn v1_share_links_get<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
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

    // Parse share link ID
    let share_link_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid share link ID format")),
            )
                .into_response();
        }
    };

    match state
        .sql_storage
        .share_links_get(share_link_id, user.id)
        .await
    {
        Ok(Some(row)) => (
            StatusCode::OK,
            Json(V1ShareLinkResponse::from_row(row, DEFAULT_SHARE_BASE_URL)),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Share link not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get share link: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get share link")),
            )
                .into_response()
        }
    }
}

/// Update a share link.
///
/// PATCH /v1/share-links/{id}
pub async fn v1_share_links_update<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
    Json(payload): Json<V1ShareLinkUpdateRequest>,
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

    // Parse share link ID
    let share_link_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid share link ID format")),
            )
                .into_response();
        }
    };

    // Parse permission if provided
    let permission = if let Some(ref perm_str) = payload.permission {
        match parse_share_permission(perm_str) {
            Some(p) => Some(p),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(format!(
                        "Invalid permission: {}. Must be 'view' or 'download'",
                        perm_str
                    ))),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    // Parse expiration date if provided
    let expires_at = match &payload.expires_at {
        Some(Some(expires_str)) => match parse_datetime(expires_str) {
            Some(dt) => Some(Some(dt)),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(
                        "Invalid expires_at format. Use ISO 8601 (e.g., 2024-12-31T23:59:59Z)",
                    )),
                )
                    .into_response();
            }
        },
        Some(None) => Some(None), // Explicitly clear expiration
        None => None,             // No change
    };

    // Handle password update
    // Empty string = remove password, non-empty = set new password hash
    let password_hash = payload.password.as_ref().map(|p| {
        if p.is_empty() {
            None // Remove password
        } else {
            Some(hash_password(p)) // Set new password
        }
    });

    let input = ShareLinkUpdate {
        name: payload.name,
        permission,
        password_hash,
        expires_at,
        max_access_count: payload.max_access_count,
        is_active: payload.is_active,
    };

    match state
        .sql_storage
        .share_links_update(share_link_id, user.id, input)
        .await
    {
        Ok(Some(row)) => (
            StatusCode::OK,
            Json(V1ShareLinkResponse::from_row(row, DEFAULT_SHARE_BASE_URL)),
        )
            .into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Share link not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to update share link: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to update share link",
                )),
            )
                .into_response()
        }
    }
}

/// Delete a share link.
///
/// DELETE /v1/share-links/{id}
pub async fn v1_share_links_delete<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(id): Path<String>,
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

    // Parse share link ID
    let share_link_id = match uuid::Uuid::parse_str(&id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid share link ID format")),
            )
                .into_response();
        }
    };

    match state
        .sql_storage
        .share_links_delete(share_link_id, user.id)
        .await
    {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(V1ErrorResponse::not_found("Share link not found")),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to delete share link: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to delete share link",
                )),
            )
                .into_response()
        }
    }
}

/// Create a share link and attach it to a content item.
///
/// POST /v1/contents/{id}/share-link
pub async fn v1_contents_share_link_create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(content_id): Path<String>,
    Json(payload): Json<V1ContentShareLinkCreateRequest>,
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
    let content_uuid = match uuid::Uuid::parse_str(&content_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid content ID format")),
            )
                .into_response();
        }
    };

    // Verify content exists and belongs to user
    match state.sql_storage.contents_get(content_uuid).await {
        Ok(Some(content)) => {
            if content.user_id != user.id {
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

    // Parse permission
    let permission = match parse_share_permission(&payload.permission) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(format!(
                    "Invalid permission: {}. Must be 'view' or 'download'",
                    payload.permission
                ))),
            )
                .into_response();
        }
    };

    // Parse expiration date if provided
    let expires_at = if let Some(ref expires_str) = payload.expires_at {
        match parse_datetime(expires_str) {
            Some(dt) => Some(dt),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(
                        "Invalid expires_at format. Use ISO 8601 (e.g., 2024-12-31T23:59:59Z)",
                    )),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    // Hash password if provided
    let password_hash = payload.password.as_ref().map(|p| hash_password(p));

    // Generate unique token
    let token = generate_share_token();

    let input = ShareLinkCreate {
        owner_id: user.id,
        token,
        name: payload.name,
        permission,
        password_hash,
        max_access_count: payload.max_access_count,
        expires_at,
    };

    // Create share link
    let share_link = match state.sql_storage.share_links_create(input).await {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("Failed to create share link: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to create share link",
                )),
            )
                .into_response();
        }
    };

    // Attach to content
    if let Err(e) = state
        .sql_storage
        .content_shares_attach_link(content_uuid, share_link.id, user.id)
        .await
    {
        tracing::error!("Failed to attach share link to content: {:?}", e);
        // Clean up the share link we just created
        let _ = state
            .sql_storage
            .share_links_delete(share_link.id, user.id)
            .await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(V1ErrorResponse::internal_error(
                "Failed to attach share link to content",
            )),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(V1ShareLinkResponse::from_row(
            share_link,
            DEFAULT_SHARE_BASE_URL,
        )),
    )
        .into_response()
}

/// Create a share link and attach it to a group.
///
/// POST /v1/groups/{id}/share-link
pub async fn v1_groups_share_link_create<S, U>(
    State(state): State<AppState<S, U>>,
    auth: RequireAuth,
    Path(group_id): Path<String>,
    Json(payload): Json<V1GroupShareLinkCreateRequest>,
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

    // Parse group ID
    let group_uuid = match uuid::Uuid::parse_str(&group_id) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request("Invalid group ID format")),
            )
                .into_response();
        }
    };

    // Verify group exists and belongs to user
    match state.sql_storage.groups_get(group_uuid).await {
        Ok(Some(group)) => {
            if group.user_id != user.id {
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

    // Parse permission
    let permission = match parse_share_permission(&payload.permission) {
        Some(p) => p,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(V1ErrorResponse::bad_request(format!(
                    "Invalid permission: {}. Must be 'view' or 'download'",
                    payload.permission
                ))),
            )
                .into_response();
        }
    };

    // Parse expiration date if provided
    let expires_at = if let Some(ref expires_str) = payload.expires_at {
        match parse_datetime(expires_str) {
            Some(dt) => Some(dt),
            None => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(
                        "Invalid expires_at format. Use ISO 8601 (e.g., 2024-12-31T23:59:59Z)",
                    )),
                )
                    .into_response();
            }
        }
    } else {
        None
    };

    // Hash password if provided
    let password_hash = payload.password.as_ref().map(|p| hash_password(p));

    // Generate unique token
    let token = generate_share_token();

    let input = ShareLinkCreate {
        owner_id: user.id,
        token,
        name: payload.name,
        permission,
        password_hash,
        max_access_count: payload.max_access_count,
        expires_at,
    };

    // Create share link
    let share_link = match state.sql_storage.share_links_create(input).await {
        Ok(row) => row,
        Err(e) => {
            tracing::error!("Failed to create share link: {:?}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error(
                    "Failed to create share link",
                )),
            )
                .into_response();
        }
    };

    // Attach to group
    if let Err(e) = state
        .sql_storage
        .group_shares_attach_link(group_uuid, share_link.id, user.id)
        .await
    {
        tracing::error!("Failed to attach share link to group: {:?}", e);
        // Clean up the share link we just created
        let _ = state
            .sql_storage
            .share_links_delete(share_link.id, user.id)
            .await;
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(V1ErrorResponse::internal_error(
                "Failed to attach share link to group",
            )),
        )
            .into_response();
    }

    (
        StatusCode::CREATED,
        Json(V1ShareLinkResponse::from_row(
            share_link,
            DEFAULT_SHARE_BASE_URL,
        )),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hash_password() {
        let password = "secret123";
        let hash = hash_password(password);

        // Hash should be consistent
        assert_eq!(hash, hash_password(password));

        // Hash should be 64 hex characters (SHA256)
        assert_eq!(hash.len(), 64);

        // Different passwords should have different hashes
        assert_ne!(hash, hash_password("different"));
    }

    #[test]
    fn test_verify_password() {
        let password = "secret123";
        let hash = hash_password(password);

        assert!(verify_password(password, &hash));
        assert!(!verify_password("wrong", &hash));
    }

    #[test]
    fn test_generate_share_token() {
        let token = generate_share_token();
        assert_eq!(token.len(), SHARE_TOKEN_LENGTH);

        // Tokens should be unique
        let token2 = generate_share_token();
        assert_ne!(token, token2);
    }

    #[test]
    fn test_parse_datetime() {
        // Valid RFC 3339 formats
        assert!(parse_datetime("2024-12-31T23:59:59Z").is_some());
        assert!(parse_datetime("2024-12-31T23:59:59+00:00").is_some());
        assert!(parse_datetime("2024-01-01T00:00:00-05:00").is_some());

        // Invalid formats
        assert!(parse_datetime("2024-12-31").is_none());
        assert!(parse_datetime("not a date").is_none());
        assert!(parse_datetime("").is_none());
    }
}
