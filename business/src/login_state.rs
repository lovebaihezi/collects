//! Login state and authentication management.
//!
//! This module provides the login state and authentication flow for the main application.
//! It tracks:
//! - Username input
//! - OTP code input
//! - Authentication status (signed in or not)
//! - Session token (preserved after login)

use std::any::Any;

use collects_states::{Command, Compute, ComputeDeps, Dep, State, Updater, assign_impl};
use log::info;

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

/// Manual-only command that handles login.
///
/// For now, this validates that username and OTP are non-empty and sets authenticated status.
/// In the future, this can be extended to make API calls for real authentication.
///
/// Dispatch explicitly via `ctx.dispatch::<LoginCommand>()`.
#[derive(Default, Debug)]
pub struct LoginCommand;

impl Command for LoginCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let input = deps.get_state_ref::<LoginInput>();

        let username = input.username.trim();
        let otp = input.otp.trim();

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

        // For now, we accept any non-empty username and OTP as valid
        // In the future, this would make an API call to verify credentials
        info!("LoginCommand: user '{}' authenticated", username);
        updater.set(AuthCompute {
            status: AuthStatus::Authenticated {
                username: username.to_string(),
                // Token would be received from the API in a real implementation
                token: Some(format!("token-for-{}", username)),
            },
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero_trust_authenticated_creates_authenticated_status() {
        let auth = AuthCompute::zero_trust_authenticated();
        
        assert!(auth.is_authenticated(), "Zero Trust auth should be authenticated");
        assert_eq!(auth.username(), Some("Zero Trust User"));
        assert_eq!(auth.token(), None, "Zero Trust auth has no token (handled by CF)");
    }

    #[test]
    fn test_auth_compute_default_is_not_authenticated() {
        let auth = AuthCompute::default();
        
        assert!(!auth.is_authenticated(), "Default auth should not be authenticated");
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
}
