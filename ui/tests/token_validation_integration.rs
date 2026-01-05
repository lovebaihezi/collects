//! Integration tests for the token validation flow.
//!
//! These tests verify the token validation flow from UI interaction
//! through the business command to the mocked API endpoint.
//!
//! Tests are only compiled for non-internal builds since internal builds
//! use Zero Trust authentication.

#![cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]

mod common;

use collects_business::{AuthCompute, AuthStatus, PendingTokenValidation, ValidateTokenCommand};
use collects_ui::state::State;
use egui_kittest::Harness;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Time to wait for async API responses in tests (milliseconds).
const API_RESPONSE_WAIT_MS: u64 = 100;

/// Test context for token validation integration tests.
struct TokenValidationTestCtx<'a> {
    mock_server: MockServer,
    harness: Harness<'a, State>,
}

impl<'a> TokenValidationTestCtx<'a> {
    /// Get mutable reference to the harness.
    fn harness_mut(&mut self) -> &mut Harness<'a, State> {
        &mut self.harness
    }

    /// Get reference to the mock server.
    fn mock_server(&self) -> &MockServer {
        &self.mock_server
    }
}

/// Setup test state with mock server.
async fn setup_test<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> TokenValidationTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock the health check endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);

    let harness = Harness::new_ui_state(app, state);

    TokenValidationTestCtx {
        mock_server,
        harness,
    }
}

// ============================================================================
// Token Validation Tests
// ============================================================================

#[tokio::test]
async fn test_validate_token_success() {
    let mut ctx = setup_test(|ui, state| {
        ui.label("Token Validation Test");
        // Just a placeholder UI for testing
        if let Some(auth) = state.ctx.cached::<AuthCompute>() {
            if auth.is_authenticated() {
                ui.label(format!(
                    "Authenticated as: {}",
                    auth.username().unwrap_or("unknown")
                ));
            } else {
                ui.label("Not authenticated");
            }
        }
    })
    .await;

    // Mock successful token validation
    Mock::given(method("POST"))
        .and(path("/api/auth/validate-token"))
        .and(body_json(serde_json::json!({
            "token": "valid-test-token"
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": true,
            "username": "tokenuser"
        })))
        .mount(ctx.mock_server())
        .await;

    let harness = ctx.harness_mut();

    // Set the pending token and dispatch validation command
    {
        let state = harness.state_mut();
        state.ctx.update::<PendingTokenValidation>(|pending| {
            pending.token = Some("valid-test-token".to_string());
        });
        state.ctx.dispatch::<ValidateTokenCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(API_RESPONSE_WAIT_MS)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Verify authentication state
    {
        let state = harness.state();
        let compute = state.ctx.cached::<AuthCompute>().unwrap();
        assert!(
            compute.is_authenticated(),
            "Should be authenticated after successful token validation"
        );
        assert_eq!(
            compute.username(),
            Some("tokenuser"),
            "Should have correct username from token"
        );
        assert_eq!(
            compute.token(),
            Some("valid-test-token"),
            "Should preserve the validated token"
        );
    }
}

#[tokio::test]
async fn test_validate_token_invalid() {
    let mut ctx = setup_test(|ui, state| {
        ui.label("Token Validation Test");
        if let Some(auth) = state.ctx.cached::<AuthCompute>() {
            if auth.is_authenticated() {
                ui.label("Authenticated");
            } else {
                ui.label("Not authenticated");
            }
        }
    })
    .await;

    // Mock invalid token validation (401 response)
    Mock::given(method("POST"))
        .and(path("/api/auth/validate-token"))
        .and(body_json(serde_json::json!({
            "token": "invalid-token"
        })))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "valid": false,
            "message": "Invalid or expired token"
        })))
        .mount(ctx.mock_server())
        .await;

    let harness = ctx.harness_mut();

    // Set the pending token and dispatch validation command
    {
        let state = harness.state_mut();
        state.ctx.update::<PendingTokenValidation>(|pending| {
            pending.token = Some("invalid-token".to_string());
        });
        state.ctx.dispatch::<ValidateTokenCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(API_RESPONSE_WAIT_MS)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Verify authentication state - should NOT be authenticated
    {
        let state = harness.state();
        let compute = state.ctx.cached::<AuthCompute>().unwrap();
        assert!(
            !compute.is_authenticated(),
            "Should NOT be authenticated after invalid token validation"
        );
        assert!(
            matches!(compute.status, AuthStatus::NotAuthenticated),
            "Status should be NotAuthenticated"
        );
    }
}

#[tokio::test]
async fn test_validate_token_expired() {
    let mut ctx = setup_test(|ui, state| {
        ui.label("Token Validation Test");
        if let Some(auth) = state.ctx.cached::<AuthCompute>() {
            if auth.is_authenticated() {
                ui.label("Authenticated");
            } else {
                ui.label("Not authenticated");
            }
        }
    })
    .await;

    // Mock expired token validation
    Mock::given(method("POST"))
        .and(path("/api/auth/validate-token"))
        .and(body_json(serde_json::json!({
            "token": "expired-token"
        })))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "valid": false,
            "message": "Token expired"
        })))
        .mount(ctx.mock_server())
        .await;

    let harness = ctx.harness_mut();

    // Set the pending token and dispatch validation command
    {
        let state = harness.state_mut();
        state.ctx.update::<PendingTokenValidation>(|pending| {
            pending.token = Some("expired-token".to_string());
        });
        state.ctx.dispatch::<ValidateTokenCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(API_RESPONSE_WAIT_MS)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Verify authentication state - should NOT be authenticated
    {
        let state = harness.state();
        let compute = state.ctx.cached::<AuthCompute>().unwrap();
        assert!(
            !compute.is_authenticated(),
            "Should NOT be authenticated after expired token validation"
        );
    }
}

#[tokio::test]
async fn test_validate_token_no_token() {
    let mut ctx = setup_test(|ui, state| {
        ui.label("Token Validation Test");
        if let Some(auth) = state.ctx.cached::<AuthCompute>() {
            if auth.is_authenticated() {
                ui.label("Authenticated");
            } else {
                ui.label("Not authenticated");
            }
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Dispatch validation command without setting a token
    {
        let state = harness.state_mut();
        // Ensure no token is pending
        state.ctx.update::<PendingTokenValidation>(|pending| {
            pending.token = None;
        });
        state.ctx.dispatch::<ValidateTokenCommand>();
    }

    harness.step();

    // Sync computes to get the result (should be immediate since no API call)
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Verify authentication state - should NOT be authenticated
    {
        let state = harness.state();
        let compute = state.ctx.cached::<AuthCompute>().unwrap();
        assert!(
            !compute.is_authenticated(),
            "Should NOT be authenticated when no token provided"
        );
        assert!(
            matches!(compute.status, AuthStatus::NotAuthenticated),
            "Status should be NotAuthenticated"
        );
    }
}

#[tokio::test]
async fn test_validate_token_server_error() {
    let mut ctx = setup_test(|ui, state| {
        ui.label("Token Validation Test");
        if let Some(auth) = state.ctx.cached::<AuthCompute>() {
            if auth.is_authenticated() {
                ui.label("Authenticated");
            } else {
                ui.label("Not authenticated");
            }
        }
    })
    .await;

    // Mock server error (500 response)
    Mock::given(method("POST"))
        .and(path("/api/auth/validate-token"))
        .respond_with(ResponseTemplate::new(500))
        .mount(ctx.mock_server())
        .await;

    let harness = ctx.harness_mut();

    // Set the pending token and dispatch validation command
    {
        let state = harness.state_mut();
        state.ctx.update::<PendingTokenValidation>(|pending| {
            pending.token = Some("some-token".to_string());
        });
        state.ctx.dispatch::<ValidateTokenCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(API_RESPONSE_WAIT_MS)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Verify authentication state - should NOT be authenticated (fail gracefully)
    {
        let state = harness.state();
        let compute = state.ctx.cached::<AuthCompute>().unwrap();
        assert!(
            !compute.is_authenticated(),
            "Should NOT be authenticated after server error"
        );
    }
}

#[tokio::test]
async fn test_pending_token_validation_initial_state() {
    let mut ctx = setup_test(|ui, state| {
        ui.label("Token Validation Test");
        // Check pending token state
        let pending = state.ctx.state_mut::<PendingTokenValidation>();
        if pending.token.is_some() {
            ui.label("Has pending token");
        } else {
            ui.label("No pending token");
        }
    })
    .await;

    let harness = ctx.harness_mut();
    harness.step();

    // The initial state should have no pending token - verified by UI label
    // We can check by dispatching the command and seeing it sets NotAuthenticated
    {
        let state = harness.state_mut();
        state.ctx.dispatch::<ValidateTokenCommand>();
        state.ctx.sync_computes();
    }

    harness.step();

    {
        let state = harness.state();
        let compute = state.ctx.cached::<AuthCompute>().unwrap();
        assert!(
            !compute.is_authenticated(),
            "Should be NotAuthenticated when no token is set"
        );
    }
}
