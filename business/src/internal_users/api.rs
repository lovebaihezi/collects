//! Internal users API client helpers.
//!
//! This module is part of the business layer: it performs network IO against
//! `/internal/*` endpoints and is intended to be used by Commands / Computes.
//!
//! Notes:
//! - These functions use `reqwest` for async HTTP requests.
//! - They attach the `cf-authorization` header when a Cloudflare Zero Trust token
//!   is available.
//! - They are async-native; call sites can use them directly in async contexts.
//!
//! This module intentionally contains *no egui memory plumbing*; that belongs in
//! UI code. Callers should map results into state/compute updates.

use crate::CLIENT;
use crate::cf_token_compute::CFTokenCompute;
use crate::internal::{
    CreateUserRequest, CreateUserResponse, DeleteUserResponse, GetUserResponse, InternalUserItem,
    ListUsersResponse, RevokeOtpResponse, UpdateProfileRequest, UpdateProfileResponse,
    UpdateUsernameRequest, UpdateUsernameResponse,
};
use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};

/// Minimal error wrapper for API calls.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InternalUsersApiError {
    pub message: String,
}

impl InternalUsersApiError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for InternalUsersApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InternalUsersApiError {}

/// A typed API result.
pub type ApiResult<T> = Result<T, InternalUsersApiError>;

fn build_headers(cf_token: &CFTokenCompute) -> HeaderMap {
    let mut headers = HeaderMap::new();
    if let Some(token) = cf_token.token()
        && let Ok(value) = HeaderValue::from_str(token)
    {
        headers.insert("cf-authorization", value);
    }
    headers
}

fn http_status_error(status: u16) -> InternalUsersApiError {
    InternalUsersApiError::new(format!("API returned status: {status}"))
}

/// GET `/internal/users`
pub async fn list_users(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
) -> ApiResult<Vec<InternalUserItem>> {
    let url = format!("{api_base_url}/internal/users");

    let response = CLIENT
        .get(&url)
        .headers(build_headers(cf_token))
        .send()
        .await
        .map_err(|e| InternalUsersApiError::new(e.to_string()))?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(http_status_error(status));
    }

    let list_response: ListUsersResponse = response.json().await.map_err(|e| {
        InternalUsersApiError::new(format!("Failed to parse ListUsersResponse: {e}"))
    })?;

    Ok(list_response.users)
}

/// GET `/internal/users/{username}`
pub async fn get_user(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
) -> ApiResult<GetUserResponse> {
    let url = format!("{api_base_url}/internal/users/{username}");

    let response = CLIENT
        .get(&url)
        .headers(build_headers(cf_token))
        .send()
        .await
        .map_err(|e| InternalUsersApiError::new(e.to_string()))?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(http_status_error(status));
    }

    response
        .json()
        .await
        .map_err(|e| InternalUsersApiError::new(format!("Failed to parse GetUserResponse: {e}")))
}

/// PUT `/internal/users/{old_username}`
/// Body: `{ "new_username": "..." }`
pub async fn update_username(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    old_username: &str,
    new_username: &str,
) -> ApiResult<UpdateUsernameResponse> {
    let url = format!("{api_base_url}/internal/users/{old_username}");

    let body = UpdateUsernameRequest {
        new_username: new_username.to_string(),
    };

    let mut headers = build_headers(cf_token);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let response = CLIENT
        .put(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| InternalUsersApiError::new(e.to_string()))?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(http_status_error(status));
    }

    response.json().await.map_err(|e| {
        InternalUsersApiError::new(format!("Failed to parse UpdateUsernameResponse: {e}"))
    })
}

/// PUT `/internal/users/{username}/profile`
/// Body: `{ "nickname": <string|null>, "avatar_url": <string|null> }`
pub async fn update_profile(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
    nickname: Option<String>,
    avatar_url: Option<String>,
) -> ApiResult<UpdateProfileResponse> {
    let url = format!("{api_base_url}/internal/users/{username}/profile");

    let body = UpdateProfileRequest {
        nickname,
        avatar_url,
    };

    let mut headers = build_headers(cf_token);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let response = CLIENT
        .put(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| InternalUsersApiError::new(e.to_string()))?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(http_status_error(status));
    }

    response.json().await.map_err(|e| {
        InternalUsersApiError::new(format!("Failed to parse UpdateProfileResponse: {e}"))
    })
}

/// DELETE `/internal/users/{username}`
pub async fn delete_user(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
) -> ApiResult<DeleteUserResponse> {
    let url = format!("{api_base_url}/internal/users/{username}");

    let response = CLIENT
        .delete(&url)
        .headers(build_headers(cf_token))
        .send()
        .await
        .map_err(|e| InternalUsersApiError::new(e.to_string()))?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(http_status_error(status));
    }

    response
        .json()
        .await
        .map_err(|e| InternalUsersApiError::new(format!("Failed to parse DeleteUserResponse: {e}")))
}

/// POST `/internal/users/{username}/revoke`
/// Empty body.
pub async fn revoke_otp(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
) -> ApiResult<RevokeOtpResponse> {
    let url = format!("{api_base_url}/internal/users/{username}/revoke");

    let response = CLIENT
        .post(&url)
        .headers(build_headers(cf_token))
        .send()
        .await
        .map_err(|e| InternalUsersApiError::new(e.to_string()))?;

    let status = response.status().as_u16();
    if status != 200 {
        return Err(http_status_error(status));
    }

    response
        .json()
        .await
        .map_err(|e| InternalUsersApiError::new(format!("Failed to parse RevokeOtpResponse: {e}")))
}

/// POST `/internal/users`
/// Body: `{ "username": "..." }`
///
/// Returns `201 Created` with `CreateUserResponse`.
pub async fn create_user(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
) -> ApiResult<CreateUserResponse> {
    let url = format!("{api_base_url}/internal/users");

    let body = CreateUserRequest {
        username: username.to_string(),
    };

    let mut headers = build_headers(cf_token);
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let response = CLIENT
        .post(&url)
        .headers(headers)
        .json(&body)
        .send()
        .await
        .map_err(|e| InternalUsersApiError::new(e.to_string()))?;

    let status = response.status().as_u16();
    if status != 201 {
        return Err(http_status_error(status));
    }

    response
        .json()
        .await
        .map_err(|e| InternalUsersApiError::new(format!("Failed to parse CreateUserResponse: {e}")))
}
