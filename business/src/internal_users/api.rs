//! Internal users API client helpers.
//!
//! This module is part of the business layer: it performs network IO against
//! `/internal/*` endpoints and is intended to be used by Commands / Computes.
//!
//! Notes:
//! - These functions use `ehttp` (WASM-friendly).
//! - They attach the `cf-authorization` header when a Cloudflare Zero Trust token
//!   is available.
//! - They are callback-based; call sites are responsible for scheduling UI
//!   repaint (if needed) and/or updating state/computes via `Updater::set()`.
//!
//! This module intentionally contains *no egui memory plumbing*; that belongs in
//! UI code. Callers should map results into state/compute updates.

use crate::cf_token_compute::CFTokenCompute;
use crate::internal::{
    CreateUserRequest, CreateUserResponse, DeleteUserResponse, GetUserResponse, InternalUserItem,
    ListUsersResponse, RevokeOtpResponse, UpdateProfileRequest, UpdateProfileResponse,
    UpdateUsernameRequest, UpdateUsernameResponse,
};

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

fn attach_cf_token_if_present(headers: &mut ehttp::Headers, cf_token: &CFTokenCompute) {
    if let Some(token) = cf_token.token() {
        headers.insert("cf-authorization", token);
    }
}

fn json_parse_error(what: &str, err: serde_json::Error) -> InternalUsersApiError {
    InternalUsersApiError::new(format!("{what}: {err}"))
}

fn http_status_error(status: u16) -> InternalUsersApiError {
    InternalUsersApiError::new(format!("API returned status: {status}"))
}

/// GET `/internal/users`
pub fn list_users(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    on_complete: impl FnOnce(ApiResult<Vec<InternalUserItem>>) + 'static + Send,
) {
    let url = format!("{api_base_url}/internal/users");
    let mut request = ehttp::Request::get(&url);
    attach_cf_token_if_present(&mut request.headers, cf_token);

    ehttp::fetch(request, move |result| match result {
        Ok(response) => {
            if response.status != 200 {
                on_complete(Err(http_status_error(response.status)));
                return;
            }

            match serde_json::from_slice::<ListUsersResponse>(&response.bytes) {
                Ok(list_response) => on_complete(Ok(list_response.users)),
                Err(e) => on_complete(Err(json_parse_error(
                    "Failed to parse ListUsersResponse",
                    e,
                ))),
            }
        }
        Err(err) => on_complete(Err(InternalUsersApiError::new(err.to_string()))),
    });
}

/// GET `/internal/users/{username}`
pub fn get_user(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
    on_complete: impl FnOnce(ApiResult<GetUserResponse>) + 'static + Send,
) {
    let url = format!("{api_base_url}/internal/users/{username}");
    let mut request = ehttp::Request::get(&url);
    attach_cf_token_if_present(&mut request.headers, cf_token);

    ehttp::fetch(request, move |result| match result {
        Ok(response) => {
            if response.status != 200 {
                on_complete(Err(http_status_error(response.status)));
                return;
            }

            match serde_json::from_slice::<GetUserResponse>(&response.bytes) {
                Ok(v) => on_complete(Ok(v)),
                Err(e) => on_complete(Err(json_parse_error("Failed to parse GetUserResponse", e))),
            }
        }
        Err(err) => on_complete(Err(InternalUsersApiError::new(err.to_string()))),
    });
}

/// PUT `/internal/users/{old_username}`
/// Body: `{ "new_username": "..." }`
pub fn update_username(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    old_username: &str,
    new_username: &str,
    on_complete: impl FnOnce(ApiResult<UpdateUsernameResponse>) + 'static + Send,
) {
    let url = format!("{api_base_url}/internal/users/{old_username}");

    let body = match serde_json::to_vec(&UpdateUsernameRequest {
        new_username: new_username.to_string(),
    }) {
        Ok(body) => body,
        Err(e) => {
            on_complete(Err(json_parse_error(
                "Failed to serialize UpdateUsernameRequest",
                e,
            )));
            return;
        }
    };

    let mut request = ehttp::Request::post(&url, body);
    request.method = "PUT".to_string();
    request.headers.insert("Content-Type", "application/json");
    attach_cf_token_if_present(&mut request.headers, cf_token);

    ehttp::fetch(request, move |result| match result {
        Ok(response) => {
            if response.status != 200 {
                on_complete(Err(http_status_error(response.status)));
                return;
            }

            match serde_json::from_slice::<UpdateUsernameResponse>(&response.bytes) {
                Ok(v) => on_complete(Ok(v)),
                Err(e) => on_complete(Err(json_parse_error(
                    "Failed to parse UpdateUsernameResponse",
                    e,
                ))),
            }
        }
        Err(err) => on_complete(Err(InternalUsersApiError::new(err.to_string()))),
    });
}

/// PUT `/internal/users/{username}/profile`
/// Body: `{ "nickname": <string|null>, "avatar_url": <string|null> }`
pub fn update_profile(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
    nickname: Option<String>,
    avatar_url: Option<String>,
    on_complete: impl FnOnce(ApiResult<UpdateProfileResponse>) + 'static + Send,
) {
    let url = format!("{api_base_url}/internal/users/{username}/profile");

    let body = match serde_json::to_vec(&UpdateProfileRequest {
        nickname,
        avatar_url,
    }) {
        Ok(body) => body,
        Err(e) => {
            on_complete(Err(json_parse_error(
                "Failed to serialize UpdateProfileRequest",
                e,
            )));
            return;
        }
    };

    let mut request = ehttp::Request::post(&url, body);
    request.method = "PUT".to_string();
    request.headers.insert("Content-Type", "application/json");
    attach_cf_token_if_present(&mut request.headers, cf_token);

    ehttp::fetch(request, move |result| match result {
        Ok(response) => {
            if response.status != 200 {
                on_complete(Err(http_status_error(response.status)));
                return;
            }

            match serde_json::from_slice::<UpdateProfileResponse>(&response.bytes) {
                Ok(v) => on_complete(Ok(v)),
                Err(e) => on_complete(Err(json_parse_error(
                    "Failed to parse UpdateProfileResponse",
                    e,
                ))),
            }
        }
        Err(err) => on_complete(Err(InternalUsersApiError::new(err.to_string()))),
    });
}

/// DELETE `/internal/users/{username}`
pub fn delete_user(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
    on_complete: impl FnOnce(ApiResult<DeleteUserResponse>) + 'static + Send,
) {
    let url = format!("{api_base_url}/internal/users/{username}");
    let mut request = ehttp::Request::get(&url);
    request.method = "DELETE".to_string();
    attach_cf_token_if_present(&mut request.headers, cf_token);

    ehttp::fetch(request, move |result| match result {
        Ok(response) => {
            if response.status != 200 {
                on_complete(Err(http_status_error(response.status)));
                return;
            }

            match serde_json::from_slice::<DeleteUserResponse>(&response.bytes) {
                Ok(v) => on_complete(Ok(v)),
                Err(e) => on_complete(Err(json_parse_error(
                    "Failed to parse DeleteUserResponse",
                    e,
                ))),
            }
        }
        Err(err) => on_complete(Err(InternalUsersApiError::new(err.to_string()))),
    });
}

/// POST `/internal/users/{username}/revoke`
/// Empty body.
pub fn revoke_otp(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
    on_complete: impl FnOnce(ApiResult<RevokeOtpResponse>) + 'static + Send,
) {
    let url = format!("{api_base_url}/internal/users/{username}/revoke");
    let mut request = ehttp::Request::post(&url, Vec::new());
    attach_cf_token_if_present(&mut request.headers, cf_token);

    ehttp::fetch(request, move |result| match result {
        Ok(response) => {
            if response.status != 200 {
                on_complete(Err(http_status_error(response.status)));
                return;
            }

            match serde_json::from_slice::<RevokeOtpResponse>(&response.bytes) {
                Ok(v) => on_complete(Ok(v)),
                Err(e) => on_complete(Err(json_parse_error(
                    "Failed to parse RevokeOtpResponse",
                    e,
                ))),
            }
        }
        Err(err) => on_complete(Err(InternalUsersApiError::new(err.to_string()))),
    });
}

/// POST `/internal/users`
/// Body: `{ "username": "..." }`
///
/// Returns `201 Created` with `CreateUserResponse`.
pub fn create_user(
    api_base_url: &str,
    cf_token: &CFTokenCompute,
    username: &str,
    on_complete: impl FnOnce(ApiResult<CreateUserResponse>) + 'static + Send,
) {
    let url = format!("{api_base_url}/internal/users");

    let body = match serde_json::to_vec(&CreateUserRequest {
        username: username.to_string(),
    }) {
        Ok(body) => body,
        Err(e) => {
            on_complete(Err(json_parse_error(
                "Failed to serialize CreateUserRequest",
                e,
            )));
            return;
        }
    };

    let mut request = ehttp::Request::post(&url, body);
    request.headers.insert("Content-Type", "application/json");
    attach_cf_token_if_present(&mut request.headers, cf_token);

    ehttp::fetch(request, move |result| match result {
        Ok(response) => {
            if response.status != 201 {
                on_complete(Err(http_status_error(response.status)));
                return;
            }

            match serde_json::from_slice::<CreateUserResponse>(&response.bytes) {
                Ok(v) => on_complete(Ok(v)),
                Err(e) => on_complete(Err(json_parse_error(
                    "Failed to parse CreateUserResponse",
                    e,
                ))),
            }
        }
        Err(err) => on_complete(Err(InternalUsersApiError::new(err.to_string()))),
    });
}
