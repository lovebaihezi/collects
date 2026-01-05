//! Login state and authentication management.
//!
//! This module provides the login state and authentication flow for the main application.
//! It tracks:
//! - Username input
//! - OTP code input
//! - Authentication status (signed in or not)
//! - Session token (preserved after login)
//!
//! ## Security
//!
//! Authentication is performed by verifying OTP codes against the backend `/auth/verify-otp`
//! endpoint. The backend validates the OTP code using TOTP (Time-based One-Time Password)
//! algorithm against stored user secrets.

use std::any::Any;

use crate::BusinessConfig;
use collects_states::{Command, Compute, ComputeDeps, Dep, State, Updater, assign_impl};
use log::{error, info};
use serde::{Deserialize, Serialize};

/// Request payload for OTP verification.
#[derive(Debug, Clone, Serialize)]
pub struct VerifyOtpRequest {
    /// The username of the user.
    pub username: String,
    /// The OTP code to verify.
    pub code: String,
}

/// Response from OTP verification endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct VerifyOtpResponse {
    /// Whether the OTP code is valid.
    pub valid: bool,
    /// Optional message with details.
    pub message: Option<String>,
    /// Session token for authenticated API calls (present on success).
    pub token: Option<String>,
}

/// Request payload for token validation.
#[derive(Debug, Clone, Serialize)]
pub struct ValidateTokenRequest {
    /// The JWT token to validate.
    pub token: String,
}

/// Response from token validation endpoint.
#[derive(Debug, Clone, Deserialize)]
pub struct ValidateTokenResponse {
    /// Whether the token is valid.
    pub valid: bool,
    /// The username from the token (if valid).
    pub username: Option<String>,
    /// Optional message with details.
    pub message: Option<String>,
}

/// Input state for login form.
///
/// Contains the editable fields for username and OTP.
#[derive(Default, Debug, Clone)]
pub struct LoginInput {
    /// Username entered by the user.
    pub username: String,
    /// OTP code entered by the user.
    pub otp: String,
}

impl State for LoginInput {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Result/status of authentication.
#[derive(Debug, Clone, Default)]
pub enum AuthStatus {
    /// Not authenticated yet.
    #[default]
    NotAuthenticated,
    /// Authentication in progress.
    Authenticating,
    /// Successfully authenticated.
    Authenticated {
        /// The username of the authenticated user.
        username: String,
        /// Session token (preserved for API calls).
        token: Option<String>,
    },
    /// Authentication failed with an error.
    Failed(String),
}

impl AuthStatus {
    /// Check if the user is authenticated.
    pub fn is_authenticated(&self) -> bool {
        matches!(self, Self::Authenticated { .. })
    }

    /// Get the username if authenticated.
    pub fn username(&self) -> Option<&str> {
        match self {
            Self::Authenticated { username, .. } => Some(username.as_str()),
            _ => None,
        }
    }

    /// Get the token if authenticated.
    pub fn token(&self) -> Option<&str> {
        match self {
            Self::Authenticated { token, .. } => token.as_deref(),
            _ => None,
        }
    }
}

/// Compute-shaped cache for authentication status.
///
/// This is intentionally a `Compute` with a no-op `compute()` so it can be read through
/// the normal caching path and updated via `Updater::set(...)` from a command.
#[derive(Default, Debug)]
pub struct AuthCompute {
    pub status: AuthStatus,
}

impl AuthCompute {
    /// Check if the user is authenticated.
    pub fn is_authenticated(&self) -> bool {
        self.status.is_authenticated()
    }

    /// Get the username if authenticated.
    pub fn username(&self) -> Option<&str> {
        self.status.username()
    }

    /// Get the token if authenticated.
    pub fn token(&self) -> Option<&str> {
        self.status.token()
    }

    /// Create an authenticated `AuthCompute` for Zero Trust environments.
    ///
    /// In internal builds, users are authenticated via Cloudflare Zero Trust,
    /// so we skip the login page and treat them as authenticated.
    pub fn zero_trust_authenticated() -> Self {
        Self {
            status: AuthStatus::Authenticated {
                username: "Zero Trust User".to_string(),
                token: None,
            },
        }
    }
}

impl Compute for AuthCompute {
    fn deps(&self) -> ComputeDeps {
        // Cache updated by a command; no derived dependencies.
        const STATE_IDS: [std::any::TypeId; 0] = [];
        const COMPUTE_IDS: [std::any::TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // Intentionally no-op.
        //
        // Auth updates are explicit user actions handled by `LoginCommand`.
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

impl State for AuthCompute {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Extracts an error message from a response, falling back to a default message.
fn extract_error_message(response_bytes: &[u8], default: &str) -> String {
    serde_json::from_slice::<VerifyOtpResponse>(response_bytes)
        .map(|r| r.message.unwrap_or_else(|| default.to_string()))
        .unwrap_or_else(|_| default.to_string())
}

/// Manual-only command that handles login.
///
/// This command verifies user credentials against the backend `/auth/verify-otp` endpoint.
/// The backend validates the OTP code using TOTP algorithm against the stored user secret.
///
/// ## Flow
///
/// 1. Validates that username and OTP are non-empty
/// 2. Sets status to `Authenticating`
/// 3. Makes HTTP POST to `/auth/verify-otp` with username and code
/// 4. On success (valid=true), sets status to `Authenticated`
/// 5. On failure, sets status to `Failed` with error message
///
/// Dispatch explicitly via `ctx.dispatch::<LoginCommand>()`.
#[derive(Default, Debug)]
pub struct LoginCommand;

impl Command for LoginCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let input = deps.get_state_ref::<LoginInput>();
        let config = deps.get_state_ref::<BusinessConfig>();

        let username = input.username.trim().to_string();
        let otp = input.otp.trim().to_string();

        if username.is_empty() {
            info!("LoginCommand: username is empty");
            updater.set(AuthCompute {
                status: AuthStatus::Failed("Username is required".to_string()),
            });
            return;
        }

        if otp.is_empty() {
            info!("LoginCommand: OTP is empty");
            updater.set(AuthCompute {
                status: AuthStatus::Failed("OTP code is required".to_string()),
            });
            return;
        }

        // Validate OTP format: must be 6 digits
        let is_valid_format = otp.len() == 6 && otp.bytes().all(|b| b.is_ascii_digit());
        if !is_valid_format {
            info!("LoginCommand: OTP format invalid");
            updater.set(AuthCompute {
                status: AuthStatus::Failed("OTP code must be 6 digits".to_string()),
            });
            return;
        }

        info!("LoginCommand: verifying OTP for user '{}'", username);

        // Set status to authenticating while we wait for the backend response
        updater.set(AuthCompute {
            status: AuthStatus::Authenticating,
        });

        // Build the request payload
        let url = format!("{}/auth/verify-otp", config.api_url());
        let body = match serde_json::to_vec(&VerifyOtpRequest {
            username: username.clone(),
            code: otp,
        }) {
            Ok(body) => body,
            Err(e) => {
                error!("LoginCommand: Failed to serialize VerifyOtpRequest: {}", e);
                updater.set(AuthCompute {
                    status: AuthStatus::Failed(format!("Internal error: {e}")),
                });
                return;
            }
        };

        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");

        // Make the API call to verify OTP
        ehttp::fetch(request, move |result| match result {
            Ok(response) => {
                if response.status == 200 {
                    // Parse the response
                    match serde_json::from_slice::<VerifyOtpResponse>(&response.bytes) {
                        Ok(verify_response) => {
                            if verify_response.valid {
                                info!(
                                    "LoginCommand: OTP verified successfully for user '{}'",
                                    username
                                );
                                updater.set(AuthCompute {
                                    status: AuthStatus::Authenticated {
                                        username: username.clone(),
                                        // Use the session token returned by the backend
                                        token: verify_response.token,
                                    },
                                });
                            } else {
                                let error_msg = verify_response
                                    .message
                                    .unwrap_or_else(|| "Invalid username or OTP code".to_string());
                                info!("LoginCommand: OTP verification failed: {}", error_msg);
                                updater.set(AuthCompute {
                                    status: AuthStatus::Failed(error_msg),
                                });
                            }
                        }
                        Err(e) => {
                            error!("LoginCommand: Failed to parse VerifyOtpResponse: {}", e);
                            updater.set(AuthCompute {
                                status: AuthStatus::Failed(
                                    "Failed to parse server response".to_string(),
                                ),
                            });
                        }
                    }
                } else if response.status == 400 {
                    // Bad request - likely invalid input format
                    let error_msg =
                        extract_error_message(&response.bytes, "Invalid request format");
                    info!("LoginCommand: Bad request: {}", error_msg);
                    updater.set(AuthCompute {
                        status: AuthStatus::Failed(error_msg),
                    });
                } else if response.status == 401 {
                    // Unauthorized - invalid credentials
                    let error_msg =
                        extract_error_message(&response.bytes, "Invalid username or OTP code");
                    info!("LoginCommand: Authentication failed: {}", error_msg);
                    updater.set(AuthCompute {
                        status: AuthStatus::Failed(error_msg),
                    });
                } else {
                    let error_msg = format!("Server error (status {})", response.status);
                    error!("LoginCommand: {}", error_msg);
                    updater.set(AuthCompute {
                        status: AuthStatus::Failed(error_msg),
                    });
                }
            }
            Err(err) => {
                let error_msg = format!("Network error: {}", err);
                error!("LoginCommand: {}", error_msg);
                updater.set(AuthCompute {
                    status: AuthStatus::Failed(error_msg),
                });
            }
        });
    }
}

/// Manual-only command that handles logout.
///
/// Clears the authentication state.
///
/// Dispatch explicitly via `ctx.dispatch::<LogoutCommand>()`.
#[derive(Default, Debug)]
pub struct LogoutCommand;

impl Command for LogoutCommand {
    fn run(&self, _deps: Dep, updater: Updater) {
        info!("LogoutCommand: user logged out");
        updater.set(AuthCompute {
            status: AuthStatus::NotAuthenticated,
        });
    }
}

/// State for holding a token to validate.
///
/// This is used by `ValidateTokenCommand` to validate a stored token on app startup.
#[derive(Default, Debug, Clone)]
pub struct PendingTokenValidation {
    /// The token to validate.
    pub token: Option<String>,
}

impl State for PendingTokenValidation {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Manual-only command that validates a stored token.
///
/// This command validates a JWT token against the backend `/auth/validate-token` endpoint.
/// It's used to restore authentication state on app startup from persisted storage.
///
/// ## Flow
///
/// 1. Reads the pending token from `PendingTokenValidation`
/// 2. If no token, sets status to `NotAuthenticated`
/// 3. Sets status to `Authenticating`
/// 4. Makes HTTP POST to `/auth/validate-token` with the token
/// 5. On success (valid=true), sets status to `Authenticated` with username and token
/// 6. On failure, sets status to `NotAuthenticated`
///
/// Dispatch explicitly via `ctx.dispatch::<ValidateTokenCommand>()`.
#[derive(Default, Debug)]
pub struct ValidateTokenCommand;

impl Command for ValidateTokenCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let pending = deps.get_state_ref::<PendingTokenValidation>();
        let config = deps.get_state_ref::<BusinessConfig>();

        let token = match &pending.token {
            Some(t) if !t.is_empty() => t.clone(),
            _ => {
                info!("ValidateTokenCommand: no token to validate");
                updater.set(AuthCompute {
                    status: AuthStatus::NotAuthenticated,
                });
                return;
            }
        };

        info!("ValidateTokenCommand: validating stored token");

        // Set status to authenticating while we wait for the backend response
        updater.set(AuthCompute {
            status: AuthStatus::Authenticating,
        });

        // Build the request payload
        let url = format!("{}/auth/validate-token", config.api_url());
        let body = match serde_json::to_vec(&ValidateTokenRequest {
            token: token.clone(),
        }) {
            Ok(body) => body,
            Err(e) => {
                error!(
                    "ValidateTokenCommand: Failed to serialize ValidateTokenRequest: {}",
                    e
                );
                updater.set(AuthCompute {
                    status: AuthStatus::NotAuthenticated,
                });
                return;
            }
        };

        let mut request = ehttp::Request::post(&url, body);
        request.headers.insert("Content-Type", "application/json");

        // Make the API call to validate token
        ehttp::fetch(request, move |result| match result {
            Ok(response) => {
                if response.status == 200 {
                    // Parse the response
                    match serde_json::from_slice::<ValidateTokenResponse>(&response.bytes) {
                        Ok(validate_response) => {
                            if validate_response.valid {
                                // Username must be present for a valid token response
                                match validate_response.username {
                                    Some(username) => {
                                        info!(
                                            "ValidateTokenCommand: token validated successfully for user '{}'",
                                            username
                                        );
                                        updater.set(AuthCompute {
                                            status: AuthStatus::Authenticated {
                                                username,
                                                token: Some(token),
                                            },
                                        });
                                    }
                                    None => {
                                        error!(
                                            "ValidateTokenCommand: token valid but username missing"
                                        );
                                        updater.set(AuthCompute {
                                            status: AuthStatus::NotAuthenticated,
                                        });
                                    }
                                }
                            } else {
                                info!("ValidateTokenCommand: token is invalid");
                                updater.set(AuthCompute {
                                    status: AuthStatus::NotAuthenticated,
                                });
                            }
                        }
                        Err(e) => {
                            error!(
                                "ValidateTokenCommand: Failed to parse ValidateTokenResponse: {}",
                                e
                            );
                            updater.set(AuthCompute {
                                status: AuthStatus::NotAuthenticated,
                            });
                        }
                    }
                } else {
                    info!(
                        "ValidateTokenCommand: token validation failed with status {}",
                        response.status
                    );
                    updater.set(AuthCompute {
                        status: AuthStatus::NotAuthenticated,
                    });
                }
            }
            Err(err) => {
                error!("ValidateTokenCommand: Network error: {}", err);
                updater.set(AuthCompute {
                    status: AuthStatus::NotAuthenticated,
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_trust_authenticated_creates_authenticated_status() {
        let auth = AuthCompute::zero_trust_authenticated();

        assert!(
            auth.is_authenticated(),
            "Zero Trust auth should be authenticated"
        );
        assert_eq!(auth.username(), Some("Zero Trust User"));
        assert_eq!(
            auth.token(),
            None,
            "Zero Trust auth has no token (handled by CF)"
        );
    }

    #[test]
    fn test_auth_compute_default_is_not_authenticated() {
        let auth = AuthCompute::default();

        assert!(
            !auth.is_authenticated(),
            "Default auth should not be authenticated"
        );
        assert_eq!(auth.username(), None);
        assert_eq!(auth.token(), None);
    }

    #[test]
    fn test_auth_status_authenticated() {
        let status = AuthStatus::Authenticated {
            username: "test_user".to_string(),
            token: Some("test_token".to_string()),
        };

        assert!(status.is_authenticated());
        assert_eq!(status.username(), Some("test_user"));
        assert_eq!(status.token(), Some("test_token"));
    }

    #[test]
    fn test_auth_status_not_authenticated() {
        let status = AuthStatus::NotAuthenticated;

        assert!(!status.is_authenticated());
        assert_eq!(status.username(), None);
        assert_eq!(status.token(), None);
    }

    #[test]
    fn test_verify_otp_request_serialization() {
        let request = VerifyOtpRequest {
            username: "testuser".to_string(),
            code: "123456".to_string(),
        };

        let json = serde_json::to_string(&request).expect("Should serialize");
        assert!(json.contains("\"username\":\"testuser\""));
        assert!(json.contains("\"code\":\"123456\""));
    }

    #[test]
    fn test_verify_otp_response_deserialization_valid() {
        let json = r#"{"valid": true}"#;
        let response: VerifyOtpResponse = serde_json::from_str(json).expect("Should deserialize");
        assert!(response.valid);
        assert!(response.message.is_none());
        assert!(response.token.is_none());
    }

    #[test]
    fn test_verify_otp_response_deserialization_valid_with_token() {
        let json = r#"{"valid": true, "token": "test-jwt-token"}"#;
        let response: VerifyOtpResponse = serde_json::from_str(json).expect("Should deserialize");
        assert!(response.valid);
        assert!(response.message.is_none());
        assert_eq!(response.token, Some("test-jwt-token".to_string()));
    }

    #[test]
    fn test_verify_otp_response_deserialization_invalid_with_message() {
        let json = r#"{"valid": false, "message": "Invalid OTP code"}"#;
        let response: VerifyOtpResponse = serde_json::from_str(json).expect("Should deserialize");
        assert!(!response.valid);
        assert_eq!(response.message, Some("Invalid OTP code".to_string()));
        assert!(response.token.is_none());
    }

    #[test]
    fn test_validate_token_request_serialization() {
        let request = ValidateTokenRequest {
            token: "test-jwt-token".to_string(),
        };

        let json = serde_json::to_string(&request).expect("Should serialize");
        assert!(json.contains("\"token\":\"test-jwt-token\""));
    }

    #[test]
    fn test_validate_token_response_deserialization_valid() {
        let json = r#"{"valid": true, "username": "testuser"}"#;
        let response: ValidateTokenResponse =
            serde_json::from_str(json).expect("Should deserialize");
        assert!(response.valid);
        assert_eq!(response.username, Some("testuser".to_string()));
        assert!(response.message.is_none());
    }

    #[test]
    fn test_validate_token_response_deserialization_invalid() {
        let json = r#"{"valid": false, "message": "Token expired"}"#;
        let response: ValidateTokenResponse =
            serde_json::from_str(json).expect("Should deserialize");
        assert!(!response.valid);
        assert!(response.username.is_none());
        assert_eq!(response.message, Some("Token expired".to_string()));
    }

    #[test]
    fn test_pending_token_validation_default() {
        let pending = PendingTokenValidation::default();
        assert!(pending.token.is_none());
    }
}
