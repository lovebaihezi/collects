//! /v1/public endpoint handlers for unauthenticated share access.

use crate::database::{ShareLinkRow, SqlStorage};
use crate::storage::{ContentDisposition, DEFAULT_PRESIGN_EXPIRY, R2Presigner};
use crate::users::routes::AppState;
use crate::users::storage::UserStorage;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};

use super::share_links::verify_password;
use super::types::{
    V1ErrorResponse, V1PublicShareResponse, V1PublicViewUrlRequest, V1PublicViewUrlResponse,
};

/// Validate a share link is accessible (active, not expired, access count not exceeded).
fn validate_share_link(share_link: &ShareLinkRow) -> Result<(), (StatusCode, &'static str)> {
    // Check if link is active
    if !share_link.is_active {
        return Err((StatusCode::GONE, "Share link is no longer active"));
    }

    // Check expiration
    if let Some(expires_at) = share_link.expires_at
        && expires_at < chrono::Utc::now()
    {
        return Err((StatusCode::GONE, "Share link has expired"));
    }

    // Check access count
    if let Some(max_count) = share_link.max_access_count
        && share_link.access_count >= max_count
    {
        return Err((
            StatusCode::GONE,
            "Share link has reached maximum access count",
        ));
    }

    Ok(())
}

/// Get public share metadata.
///
/// GET /v1/public/share/{token}
///
/// Returns metadata about the shared content without requiring authentication.
/// If the share link is password protected, the response will indicate that
/// a password is required.
pub async fn v1_public_share_get<S, U>(
    State(state): State<AppState<S, U>>,
    Path(token): Path<String>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Try to find content shared with this token
    match state.sql_storage.contents_get_by_share_token(&token).await {
        Ok(Some((content, share_link))) => {
            // Validate share link
            if let Err((status, message)) = validate_share_link(&share_link) {
                return (status, Json(V1ErrorResponse::bad_request(message))).into_response();
            }

            (
                StatusCode::OK,
                Json(V1PublicShareResponse {
                    content_type: "content".to_string(),
                    title: content.title,
                    description: content.description,
                    permission: share_link.permission,
                    file_count: None,
                    requires_password: share_link.password_hash.is_some(),
                }),
            )
                .into_response()
        }
        Ok(None) => {
            // Try to find group shared with this token
            match state.sql_storage.groups_get_by_share_token(&token).await {
                Ok(Some((group, share_link, file_count))) => {
                    // Validate share link
                    if let Err((status, message)) = validate_share_link(&share_link) {
                        return (status, Json(V1ErrorResponse::bad_request(message)))
                            .into_response();
                    }

                    (
                        StatusCode::OK,
                        Json(V1PublicShareResponse {
                            content_type: "group".to_string(),
                            title: group.name,
                            description: group.description,
                            permission: share_link.permission,
                            file_count: Some(file_count),
                            requires_password: share_link.password_hash.is_some(),
                        }),
                    )
                        .into_response()
                }
                Ok(None) => (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Share link not found")),
                )
                    .into_response(),
                Err(e) => {
                    tracing::error!("Failed to get group by share token: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(V1ErrorResponse::internal_error("Failed to get share")),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get content by share token: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get share")),
            )
                .into_response()
        }
    }
}

/// Get a view URL for shared content.
///
/// POST /v1/public/share/{token}/view-url
///
/// Returns a presigned URL for viewing/downloading shared content.
/// If the share link is password protected, the password must be provided.
pub async fn v1_public_share_view_url<S, U>(
    State(state): State<AppState<S, U>>,
    presigner: Option<axum::Extension<R2Presigner>>,
    Path(token): Path<String>,
    Json(payload): Json<V1PublicViewUrlRequest>,
) -> impl IntoResponse
where
    S: SqlStorage,
    U: UserStorage,
{
    // Try to find content shared with this token
    let result = state.sql_storage.contents_get_by_share_token(&token).await;

    match result {
        Ok(Some((content, share_link))) => {
            // Validate share link
            if let Err((status, message)) = validate_share_link(&share_link) {
                return (status, Json(V1ErrorResponse::bad_request(message))).into_response();
            }

            // Verify password if required
            if let Some(ref password_hash) = share_link.password_hash {
                match &payload.password {
                    Some(password) if verify_password(password, password_hash) => {
                        // Password correct, continue
                    }
                    Some(_) => {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(V1ErrorResponse::bad_request("Invalid password")),
                        )
                            .into_response();
                    }
                    None => {
                        return (
                            StatusCode::UNAUTHORIZED,
                            Json(V1ErrorResponse::bad_request(
                                "Password required for this share link",
                            )),
                        )
                            .into_response();
                    }
                }
            }

            // Check permission - view-url requires at least view permission
            // (both "view" and "download" allow viewing)

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

            // If permission is "view" only, force inline disposition
            let final_disposition = if share_link.permission == "view" {
                ContentDisposition::Inline
            } else {
                disposition
            };

            // Increment access count
            if let Err(e) = state
                .sql_storage
                .share_links_increment_access(share_link.id)
                .await
            {
                tracing::warn!("Failed to increment share link access count: {:?}", e);
                // Continue anyway - access count is best-effort
            }

            // Generate presigned URL
            let presigned = if let Some(axum::Extension(presigner)) = presigner {
                match presigner
                    .presign_get(
                        &content.storage_key,
                        final_disposition,
                        DEFAULT_PRESIGN_EXPIRY,
                    )
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
                let expires_at = chrono::Utc::now()
                    + chrono::Duration::from_std(DEFAULT_PRESIGN_EXPIRY).unwrap();
                let disp = match final_disposition {
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
                Json(V1PublicViewUrlResponse {
                    url: presigned.url,
                    expires_at: presigned.expires_at.to_rfc3339(),
                }),
            )
                .into_response()
        }
        Ok(None) => {
            // Content not found - check if it's a group share
            // Group shares don't support view-url directly (would need to list items)
            match state.sql_storage.groups_get_by_share_token(&token).await {
                Ok(Some(_)) => (
                    StatusCode::BAD_REQUEST,
                    Json(V1ErrorResponse::bad_request(
                        "Cannot get view URL for group shares. Use the group contents endpoint instead.",
                    )),
                )
                    .into_response(),
                Ok(None) => (
                    StatusCode::NOT_FOUND,
                    Json(V1ErrorResponse::not_found("Share link not found")),
                )
                    .into_response(),
                Err(e) => {
                    tracing::error!("Failed to get group by share token: {:?}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(V1ErrorResponse::internal_error("Failed to get share")),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            tracing::error!("Failed to get content by share token: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(V1ErrorResponse::internal_error("Failed to get share")),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn make_share_link(
        is_active: bool,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
        max_access_count: Option<i32>,
        access_count: i32,
    ) -> ShareLinkRow {
        ShareLinkRow {
            id: Uuid::new_v4(),
            owner_id: Uuid::new_v4(),
            token: "test-token".to_string(),
            name: None,
            permission: "view".to_string(),
            password_hash: None,
            max_access_count,
            access_count,
            expires_at,
            is_active,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_validate_share_link_active() {
        let link = make_share_link(true, None, None, 0);
        assert!(validate_share_link(&link).is_ok());
    }

    #[test]
    fn test_validate_share_link_inactive() {
        let link = make_share_link(false, None, None, 0);
        let result = validate_share_link(&link);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::GONE);
    }

    #[test]
    fn test_validate_share_link_expired() {
        let yesterday = chrono::Utc::now() - chrono::Duration::days(1);
        let link = make_share_link(true, Some(yesterday), None, 0);
        let result = validate_share_link(&link);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::GONE);
    }

    #[test]
    fn test_validate_share_link_not_expired() {
        let tomorrow = chrono::Utc::now() + chrono::Duration::days(1);
        let link = make_share_link(true, Some(tomorrow), None, 0);
        assert!(validate_share_link(&link).is_ok());
    }

    #[test]
    fn test_validate_share_link_max_access_reached() {
        let link = make_share_link(true, None, Some(5), 5);
        let result = validate_share_link(&link);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().0, StatusCode::GONE);
    }

    #[test]
    fn test_validate_share_link_max_access_not_reached() {
        let link = make_share_link(true, None, Some(5), 4);
        assert!(validate_share_link(&link).is_ok());
    }
}
