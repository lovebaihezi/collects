//! Integration tests for the OTP verification login flow.
//!
//! These tests verify the complete login flow from UI interaction
//! through the business command to the mocked API endpoint.
//!
//! Tests are only compiled for non-internal builds since internal builds
//! use Zero Trust authentication and skip the login form.

#![cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]

use collects_business::{AuthCompute, AuthStatus, LoginCommand, LoginInput};
use collects_ui::state::State;
use egui_kittest::Harness;
use wiremock::matchers::{body_json, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Time to wait for async API responses in tests (milliseconds).
const API_RESPONSE_WAIT_MS: u64 = 100;

/// Test context for login integration tests.
struct LoginTestCtx<'a> {
    mock_server: MockServer,
    harness: Harness<'a, State>,
}

impl<'a> LoginTestCtx<'a> {
    /// Get mutable reference to the harness.
    fn harness_mut(&mut self) -> &mut Harness<'a, State> {
        &mut self.harness
    }

    /// Get reference to the mock server.
    fn mock_server(&self) -> &MockServer {
        &self.mock_server
    }
}

/// Setup test state with mock server configured for login endpoint.
async fn setup_login_test<'a>(app: impl FnMut(&mut egui::Ui, &mut State) + 'a) -> LoginTestCtx<'a> {
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

    LoginTestCtx {
        mock_server,
        harness,
    }
}

/// Setup test with a successful OTP verification mock.
async fn setup_with_verify_otp_success<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
    expected_username: &str,
    expected_code: &str,
) -> LoginTestCtx<'a> {
    let ctx = setup_login_test(app).await;

    // Mock successful OTP verification with token
    Mock::given(method("POST"))
        .and(path("/api/auth/verify-otp"))
        .and(body_json(serde_json::json!({
            "username": expected_username,
            "code": expected_code
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": true,
            "token": "test-session-token-jwt"
        })))
        .mount(ctx.mock_server())
        .await;

    ctx
}

/// Setup test with an invalid OTP code mock.
async fn setup_with_verify_otp_invalid<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
    expected_username: &str,
    expected_code: &str,
) -> LoginTestCtx<'a> {
    let ctx = setup_login_test(app).await;

    // Mock invalid OTP verification (valid: false)
    Mock::given(method("POST"))
        .and(path("/api/auth/verify-otp"))
        .and(body_json(serde_json::json!({
            "username": expected_username,
            "code": expected_code
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "valid": false,
            "message": "Invalid username or code"
        })))
        .mount(ctx.mock_server())
        .await;

    ctx
}

/// Setup test with a user not found mock (401 Unauthorized).
async fn setup_with_verify_otp_unauthorized<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
    expected_username: &str,
    expected_code: &str,
) -> LoginTestCtx<'a> {
    let ctx = setup_login_test(app).await;

    // Mock unauthorized (user not found or invalid credentials)
    Mock::given(method("POST"))
        .and(path("/api/auth/verify-otp"))
        .and(body_json(serde_json::json!({
            "username": expected_username,
            "code": expected_code
        })))
        .respond_with(ResponseTemplate::new(401).set_body_json(serde_json::json!({
            "valid": false,
            "message": "Invalid username or code"
        })))
        .mount(ctx.mock_server())
        .await;

    ctx
}

/// Setup test with a server error mock.
async fn setup_with_verify_otp_server_error<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> LoginTestCtx<'a> {
    let ctx = setup_login_test(app).await;

    // Mock server error (500)
    Mock::given(method("POST"))
        .and(path("/api/auth/verify-otp"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "error": "internal_error",
            "message": "Database connection failed"
        })))
        .mount(ctx.mock_server())
        .await;

    ctx
}

/// Test that LoginInput starts with empty values.
#[tokio::test]
async fn test_login_input_initial_state() {
    let mut ctx = setup_login_test(|ui, state| {
        let input = state.ctx.state::<LoginInput>();
        ui.label(format!("Username: {}", input.username));
        ui.label(format!("OTP: {}", input.otp));
    })
    .await;

    let harness = ctx.harness_mut();
    harness.step();

    // Initial state should have empty values
    let state = harness.state();
    let input = state.ctx.state::<LoginInput>();
    assert!(
        input.username.is_empty(),
        "LoginInput should start with empty username"
    );
    assert!(
        input.otp.is_empty(),
        "LoginInput should start with empty OTP"
    );
}

/// Test that AuthCompute starts in NotAuthenticated state.
#[tokio::test]
async fn test_auth_compute_initial_state() {
    let mut ctx = setup_login_test(|ui, state| {
        let compute = state.ctx.cached::<AuthCompute>();
        let status = match compute {
            Some(c) => format!("{:?}", c.status),
            None => "No compute".to_string(),
        };
        ui.label(format!("Auth status: {}", status));
    })
    .await;

    let harness = ctx.harness_mut();
    harness.step();

    // Initial state should be NotAuthenticated
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "AuthCompute should be registered");
    assert!(
        matches!(compute.unwrap().status, AuthStatus::NotAuthenticated),
        "AuthCompute should start in NotAuthenticated state"
    );
}

/// Test that triggering login with valid credentials sets state to Authenticating then Authenticated.
#[tokio::test]
async fn test_login_success_flow() {
    let mut ctx = setup_with_verify_otp_success(
        |ui, state| {
            let compute = state.ctx.cached::<AuthCompute>();
            if let Some(c) = compute {
                match &c.status {
                    AuthStatus::Authenticated { username, .. } => {
                        ui.label(format!("Authenticated: {}", username));
                    }
                    AuthStatus::Failed(e) => {
                        ui.label(format!("Failed: {}", e));
                    }
                    AuthStatus::Authenticating => {
                        ui.label("Authenticating...");
                    }
                    AuthStatus::NotAuthenticated => {
                        ui.label("Not authenticated");
                    }
                }
            }
        },
        "testuser",
        "123456",
    )
    .await;

    let harness = ctx.harness_mut();

    // Set login credentials and trigger login
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "testuser".to_string();
            input.otp = "123456".to_string();
        });
        state.ctx.dispatch::<LoginCommand>();
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

    // Check the result
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().status {
        AuthStatus::Authenticated { username, .. } => {
            assert_eq!(username, "testuser");
        }
        other => {
            panic!("Expected Authenticated state, got {:?}", other);
        }
    }
}

/// Test that login with invalid OTP returns failed state.
#[tokio::test]
async fn test_login_invalid_otp_flow() {
    let mut ctx = setup_with_verify_otp_invalid(
        |ui, state| {
            let compute = state.ctx.cached::<AuthCompute>();
            if let Some(c) = compute {
                match &c.status {
                    AuthStatus::Authenticated { username, .. } => {
                        ui.label(format!("Authenticated: {}", username));
                    }
                    AuthStatus::Failed(e) => {
                        ui.label(format!("Failed: {}", e));
                    }
                    AuthStatus::Authenticating => {
                        ui.label("Authenticating...");
                    }
                    AuthStatus::NotAuthenticated => {
                        ui.label("Not authenticated");
                    }
                }
            }
        },
        "testuser",
        "000000",
    )
    .await;

    let harness = ctx.harness_mut();

    // Set login credentials with wrong OTP
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "testuser".to_string();
            input.otp = "000000".to_string();
        });
        state.ctx.dispatch::<LoginCommand>();
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

    // Check the result - should be failed
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().status {
        AuthStatus::Failed(e) => {
            assert!(
                e.contains("Invalid") || e.contains("invalid"),
                "Error should mention invalid credentials, got: {}",
                e
            );
        }
        other => {
            panic!("Expected Failed state, got {:?}", other);
        }
    }
}

/// Test that login with non-existent user returns failed state.
#[tokio::test]
async fn test_login_unauthorized_flow() {
    let mut ctx = setup_with_verify_otp_unauthorized(
        |ui, state| {
            let compute = state.ctx.cached::<AuthCompute>();
            if let Some(c) = compute {
                match &c.status {
                    AuthStatus::Authenticated { username, .. } => {
                        ui.label(format!("Authenticated: {}", username));
                    }
                    AuthStatus::Failed(e) => {
                        ui.label(format!("Failed: {}", e));
                    }
                    AuthStatus::Authenticating => {
                        ui.label("Authenticating...");
                    }
                    AuthStatus::NotAuthenticated => {
                        ui.label("Not authenticated");
                    }
                }
            }
        },
        "nonexistent",
        "123456",
    )
    .await;

    let harness = ctx.harness_mut();

    // Set login credentials for non-existent user
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "nonexistent".to_string();
            input.otp = "123456".to_string();
        });
        state.ctx.dispatch::<LoginCommand>();
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

    // Check the result - should be failed with 401 error
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().status {
        AuthStatus::Failed(e) => {
            assert!(
                e.contains("Invalid") || e.contains("invalid"),
                "Error should mention invalid credentials, got: {}",
                e
            );
        }
        other => {
            panic!("Expected Failed state, got {:?}", other);
        }
    }
}

/// Test that server error is handled properly.
#[tokio::test]
async fn test_login_server_error_flow() {
    let mut ctx = setup_with_verify_otp_server_error(|ui, state| {
        let compute = state.ctx.cached::<AuthCompute>();
        if let Some(c) = compute {
            match &c.status {
                AuthStatus::Authenticated { username, .. } => {
                    ui.label(format!("Authenticated: {}", username));
                }
                AuthStatus::Failed(e) => {
                    ui.label(format!("Failed: {}", e));
                }
                AuthStatus::Authenticating => {
                    ui.label("Authenticating...");
                }
                AuthStatus::NotAuthenticated => {
                    ui.label("Not authenticated");
                }
            }
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Set login credentials
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "anyuser".to_string();
            input.otp = "123456".to_string();
        });
        state.ctx.dispatch::<LoginCommand>();
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

    // Check the result - should be error due to 500 status
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().status {
        AuthStatus::Failed(e) => {
            assert!(
                e.contains("500") || e.contains("Server") || e.contains("error"),
                "Error should contain status code 500 or server error, got: {}",
                e
            );
        }
        other => {
            panic!("Expected Failed state, got {:?}", other);
        }
    }
}

/// Test that empty username does not trigger login.
#[tokio::test]
async fn test_login_empty_username_fails() {
    let mut ctx = setup_login_test(|ui, state| {
        let compute = state.ctx.cached::<AuthCompute>();
        if let Some(c) = compute {
            match &c.status {
                AuthStatus::Authenticated { .. } => ui.label("Authenticated"),
                AuthStatus::Failed(e) => ui.label(format!("Failed: {}", e)),
                AuthStatus::Authenticating => ui.label("Authenticating"),
                AuthStatus::NotAuthenticated => ui.label("Not authenticated"),
            };
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Trigger with empty username
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "".to_string();
            input.otp = "123456".to_string();
        });
        state.ctx.dispatch::<LoginCommand>();
    }

    // Sync computes
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Should be in Failed state with username required error
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().status {
        AuthStatus::Failed(e) => {
            assert!(
                e.contains("Username") && e.contains("required"),
                "Error should say username is required, got: {}",
                e
            );
        }
        other => {
            panic!("Expected Failed state for empty username, got {:?}", other);
        }
    }
}

/// Test that empty OTP does not trigger login.
#[tokio::test]
async fn test_login_empty_otp_fails() {
    let mut ctx = setup_login_test(|ui, state| {
        let compute = state.ctx.cached::<AuthCompute>();
        if let Some(c) = compute {
            match &c.status {
                AuthStatus::Authenticated { .. } => ui.label("Authenticated"),
                AuthStatus::Failed(e) => ui.label(format!("Failed: {}", e)),
                AuthStatus::Authenticating => ui.label("Authenticating"),
                AuthStatus::NotAuthenticated => ui.label("Not authenticated"),
            };
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Trigger with empty OTP
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "testuser".to_string();
            input.otp = "".to_string();
        });
        state.ctx.dispatch::<LoginCommand>();
    }

    // Sync computes
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Should be in Failed state with OTP required error
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().status {
        AuthStatus::Failed(e) => {
            assert!(
                e.contains("OTP") && e.contains("required"),
                "Error should say OTP is required, got: {}",
                e
            );
        }
        other => {
            panic!("Expected Failed state for empty OTP, got {:?}", other);
        }
    }
}

/// Test that invalid OTP format (not 6 digits) does not trigger login.
#[tokio::test]
async fn test_login_invalid_otp_format_fails() {
    let mut ctx = setup_login_test(|ui, state| {
        let compute = state.ctx.cached::<AuthCompute>();
        if let Some(c) = compute {
            match &c.status {
                AuthStatus::Authenticated { .. } => ui.label("Authenticated"),
                AuthStatus::Failed(e) => ui.label(format!("Failed: {}", e)),
                AuthStatus::Authenticating => ui.label("Authenticating"),
                AuthStatus::NotAuthenticated => ui.label("Not authenticated"),
            };
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Trigger with invalid OTP format (not 6 digits)
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "testuser".to_string();
            input.otp = "12345".to_string(); // Only 5 digits
        });
        state.ctx.dispatch::<LoginCommand>();
    }

    // Sync computes
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Should be in Failed state with OTP format error
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().status {
        AuthStatus::Failed(e) => {
            assert!(
                e.contains("6 digits"),
                "Error should say OTP must be 6 digits, got: {}",
                e
            );
        }
        other => {
            panic!(
                "Expected Failed state for invalid OTP format, got {:?}",
                other
            );
        }
    }
}

/// Test that non-numeric OTP does not trigger login.
#[tokio::test]
async fn test_login_non_numeric_otp_fails() {
    let mut ctx = setup_login_test(|ui, state| {
        let compute = state.ctx.cached::<AuthCompute>();
        if let Some(c) = compute {
            match &c.status {
                AuthStatus::Authenticated { .. } => ui.label("Authenticated"),
                AuthStatus::Failed(e) => ui.label(format!("Failed: {}", e)),
                AuthStatus::Authenticating => ui.label("Authenticating"),
                AuthStatus::NotAuthenticated => ui.label("Not authenticated"),
            };
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Trigger with non-numeric OTP
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "testuser".to_string();
            input.otp = "abcdef".to_string(); // Non-numeric
        });
        state.ctx.dispatch::<LoginCommand>();
    }

    // Sync computes
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Should be in Failed state with OTP format error
    let state = harness.state();
    let compute = state.ctx.cached::<AuthCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().status {
        AuthStatus::Failed(e) => {
            assert!(
                e.contains("6 digits"),
                "Error should say OTP must be 6 digits, got: {}",
                e
            );
        }
        other => {
            panic!("Expected Failed state for non-numeric OTP, got {:?}", other);
        }
    }
}

/// Test AuthCompute helper methods.
#[tokio::test]
async fn test_auth_compute_helper_methods() {
    let mut ctx = setup_with_verify_otp_success(
        |ui, state| {
            let compute = state.ctx.cached::<AuthCompute>();
            if let Some(c) = compute {
                ui.label(format!("is_authenticated: {}", c.is_authenticated()));
                if let Some(username) = c.username() {
                    ui.label(format!("username: {}", username));
                }
                if let Some(token) = c.token() {
                    ui.label(format!("token: {}", token));
                }
            }
        },
        "helpertest",
        "123456",
    )
    .await;

    let harness = ctx.harness_mut();

    // Initial state - check helper methods
    {
        let state = harness.state();
        let compute = state.ctx.cached::<AuthCompute>().unwrap();
        assert!(
            !compute.is_authenticated(),
            "Should not be authenticated initially"
        );
        assert!(
            compute.username().is_none(),
            "Should have no username initially"
        );
        assert!(compute.token().is_none(), "Should have no token initially");
    }

    // Trigger login
    {
        let state = harness.state_mut();
        state.ctx.update::<LoginInput>(|input| {
            input.username = "helpertest".to_string();
            input.otp = "123456".to_string();
        });
        state.ctx.dispatch::<LoginCommand>();
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

    // After success - check helper methods
    {
        let state = harness.state();
        let compute = state.ctx.cached::<AuthCompute>().unwrap();
        assert!(
            compute.is_authenticated(),
            "Should be authenticated after successful login"
        );
        assert_eq!(
            compute.username(),
            Some("helpertest"),
            "Should have correct username"
        );
        // Token should be present from the backend response
        assert_eq!(
            compute.token(),
            Some("test-session-token-jwt"),
            "Token should be present after successful login"
        );
    }
}
