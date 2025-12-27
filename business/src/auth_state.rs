//! Authentication state for the UI application.
//!
//! This module provides state management for user authentication,
//! including login status and user information.

use collects_states::State;
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Represents the current authentication state of the user.
#[derive(Debug, Clone, Default)]
pub struct AuthState {
    /// The current authentication status.
    pub status: AuthStatus,
    /// The username of the logged-in user.
    pub username: Option<String>,
    /// Error message if login failed.
    pub error: Option<String>,
}

/// Authentication status.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum AuthStatus {
    /// User is not logged in.
    #[default]
    LoggedOut,
    /// Login is in progress.
    LoggingIn,
    /// User is logged in.
    LoggedIn,
    /// Login failed.
    LoginFailed,
}

/// Login form data for the login widget.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoginFormData {
    /// The username entered by the user.
    pub username: String,
    /// The OTP code entered by the user.
    pub otp_code: String,
}

impl AuthState {
    /// Creates a new `AuthState` with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns true if the user is logged in.
    pub fn is_logged_in(&self) -> bool {
        self.status == AuthStatus::LoggedIn
    }

    /// Returns true if a login attempt is in progress.
    pub fn is_logging_in(&self) -> bool {
        self.status == AuthStatus::LoggingIn
    }

    /// Sets the state to logging in.
    pub fn start_login(&mut self) {
        self.status = AuthStatus::LoggingIn;
        self.error = None;
    }

    /// Sets the state to logged in with the given username.
    pub fn login_success(&mut self, username: String) {
        self.status = AuthStatus::LoggedIn;
        self.username = Some(username);
        self.error = None;
    }

    /// Sets the state to login failed with the given error.
    pub fn login_failed(&mut self, error: String) {
        self.status = AuthStatus::LoginFailed;
        self.error = Some(error);
    }

    /// Logs out the user.
    pub fn logout(&mut self) {
        self.status = AuthStatus::LoggedOut;
        self.username = None;
        self.error = None;
    }
}

impl State for AuthState {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_state_default() {
        let state = AuthState::default();
        assert_eq!(state.status, AuthStatus::LoggedOut);
        assert!(state.username.is_none());
        assert!(state.error.is_none());
        assert!(!state.is_logged_in());
        assert!(!state.is_logging_in());
    }

    #[test]
    fn test_auth_state_login_flow() {
        let mut state = AuthState::new();

        // Start login
        state.start_login();
        assert!(state.is_logging_in());
        assert!(!state.is_logged_in());

        // Login success
        state.login_success("testuser".to_string());
        assert!(state.is_logged_in());
        assert!(!state.is_logging_in());
        assert_eq!(state.username, Some("testuser".to_string()));

        // Logout
        state.logout();
        assert!(!state.is_logged_in());
        assert!(state.username.is_none());
    }

    #[test]
    fn test_auth_state_login_failed() {
        let mut state = AuthState::new();

        state.start_login();
        state.login_failed("Invalid OTP".to_string());

        assert_eq!(state.status, AuthStatus::LoginFailed);
        assert_eq!(state.error, Some("Invalid OTP".to_string()));
    }
}
