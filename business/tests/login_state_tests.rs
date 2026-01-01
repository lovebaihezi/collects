//! Unit tests for login state types and their methods.

use collects_business::{AuthCompute, AuthStatus};

/// Tests for AuthStatus enum
mod auth_status_tests {
    use super::*;

    #[test]
    fn test_auth_status_default_is_not_authenticated() {
        let status = AuthStatus::default();
        assert!(!status.is_authenticated());
    }

    #[test]
    fn test_auth_status_default_has_no_username() {
        let status = AuthStatus::default();
        assert!(status.username().is_none());
    }

    #[test]
    fn test_auth_status_default_has_no_token() {
        let status = AuthStatus::default();
        assert!(status.token().is_none());
    }

    #[test]
    fn test_auth_status_authenticated_is_authenticated() {
        let status = AuthStatus::Authenticated {
            username: "test_user".to_string(),
            token: Some("test_token".to_string()),
        };
        assert!(status.is_authenticated());
    }

    #[test]
    fn test_auth_status_authenticated_returns_username() {
        let status = AuthStatus::Authenticated {
            username: "test_user".to_string(),
            token: Some("test_token".to_string()),
        };
        assert_eq!(status.username(), Some("test_user"));
    }

    #[test]
    fn test_auth_status_authenticated_returns_token() {
        let status = AuthStatus::Authenticated {
            username: "test_user".to_string(),
            token: Some("test_token".to_string()),
        };
        assert_eq!(status.token(), Some("test_token"));
    }

    #[test]
    fn test_auth_status_authenticated_with_no_token() {
        let status = AuthStatus::Authenticated {
            username: "test_user".to_string(),
            token: None,
        };
        assert!(status.is_authenticated());
        assert_eq!(status.username(), Some("test_user"));
        assert!(status.token().is_none());
    }

    #[test]
    fn test_auth_status_authenticating_is_not_authenticated() {
        let status = AuthStatus::Authenticating;
        assert!(!status.is_authenticated());
    }

    #[test]
    fn test_auth_status_authenticating_has_no_username() {
        let status = AuthStatus::Authenticating;
        assert!(status.username().is_none());
    }

    #[test]
    fn test_auth_status_authenticating_has_no_token() {
        let status = AuthStatus::Authenticating;
        assert!(status.token().is_none());
    }

    #[test]
    fn test_auth_status_failed_is_not_authenticated() {
        let status = AuthStatus::Failed("Error message".to_string());
        assert!(!status.is_authenticated());
    }

    #[test]
    fn test_auth_status_failed_has_no_username() {
        let status = AuthStatus::Failed("Error message".to_string());
        assert!(status.username().is_none());
    }

    #[test]
    fn test_auth_status_failed_has_no_token() {
        let status = AuthStatus::Failed("Error message".to_string());
        assert!(status.token().is_none());
    }
}

/// Tests for AuthCompute
mod auth_compute_tests {
    use super::*;

    #[test]
    fn test_auth_compute_default_is_not_authenticated() {
        let compute = AuthCompute::default();
        assert!(!compute.is_authenticated());
    }

    #[test]
    fn test_auth_compute_default_has_no_username() {
        let compute = AuthCompute::default();
        assert!(compute.username().is_none());
    }

    #[test]
    fn test_auth_compute_default_has_no_token() {
        let compute = AuthCompute::default();
        assert!(compute.token().is_none());
    }

    #[test]
    fn test_auth_compute_with_authenticated_status() {
        let compute = AuthCompute {
            status: AuthStatus::Authenticated {
                username: "test_user".to_string(),
                token: Some("test_token".to_string()),
            },
        };
        assert!(compute.is_authenticated());
        assert_eq!(compute.username(), Some("test_user"));
        assert_eq!(compute.token(), Some("test_token"));
    }

    #[test]
    fn test_auth_compute_delegates_to_status() {
        let compute = AuthCompute {
            status: AuthStatus::Failed("error".to_string()),
        };
        assert!(!compute.is_authenticated());
        assert!(compute.username().is_none());
        assert!(compute.token().is_none());
    }
}
