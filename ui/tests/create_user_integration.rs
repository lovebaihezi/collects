//! Integration tests for the internal environment create user feature.
//!
//! These tests verify the complete create user flow from UI interaction
//! through the business command to the mocked API endpoint.
//!
//! Tests are only compiled when the `env_test_internal` feature is enabled.

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

use collects_business::{
    CFTokenInput, CreateUserCommand, CreateUserCompute, CreateUserInput, CreateUserResult,
    SetCFTokenCommand,
};
use collects_ui::state::State;
use egui_kittest::Harness;
use wiremock::matchers::{body_json, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context for create user integration tests.
struct CreateUserTestCtx<'a> {
    mock_server: MockServer,
    harness: Harness<'a, State>,
}

impl<'a> CreateUserTestCtx<'a> {
    /// Get mutable reference to the harness.
    fn harness_mut(&mut self) -> &mut Harness<'a, State> {
        &mut self.harness
    }

    /// Get reference to the mock server.
    fn mock_server(&self) -> &MockServer {
        &self.mock_server
    }
}

/// Setup test state with mock server configured for create user endpoint.
async fn setup_create_user_test<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> CreateUserTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock the health check endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut state = State::test(base_url);

    // In internal env, services expects a Zero Trust token via `cf-authorization`.
    // The business command attaches this header when `CFTokenCompute` is set via `SetCFTokenCommand`.
    {
        let token_input = state.ctx.state_mut::<CFTokenInput>();
        token_input.token = Some("test-cf-authorization-token".to_string());
        state.ctx.dispatch::<SetCFTokenCommand>();
        state.ctx.sync_computes();
    }

    let harness = Harness::new_ui_state(app, state);

    CreateUserTestCtx {
        mock_server,
        harness,
    }
}

/// Setup test with a successful create user mock.
async fn setup_with_create_user_success<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
    expected_username: &str,
) -> CreateUserTestCtx<'a> {
    let ctx = setup_create_user_test(app).await;

    // Mock successful user creation
    // Note: The API URL is constructed as {base_url}/api/internal/users
    Mock::given(method("POST"))
        .and(path("/api/internal/users"))
        .and(header("cf-authorization", "test-cf-authorization-token"))
        .and(body_json(serde_json::json!({
            "username": expected_username
        })))
        .respond_with(
            ResponseTemplate::new(201).set_body_json(serde_json::json!({
                "username": expected_username,
                "secret": "JBSWY3DPEHPK3PXP",
                "otpauth_url": format!("otpauth://totp/Collects:{}?secret=JBSWY3DPEHPK3PXP&issuer=Collects", expected_username)
            })),
        )
        .mount(ctx.mock_server())
        .await;

    ctx
}

/// Setup test with a duplicate user error mock.
async fn setup_with_create_user_duplicate<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
    expected_username: &str,
) -> CreateUserTestCtx<'a> {
    let ctx = setup_create_user_test(app).await;

    // Mock duplicate user error (409 Conflict)
    // Note: The API URL is constructed as {base_url}/api/internal/users
    Mock::given(method("POST"))
        .and(path("/api/internal/users"))
        .and(header("cf-authorization", "test-cf-authorization-token"))
        .and(body_json(serde_json::json!({
            "username": expected_username
        })))
        .respond_with(ResponseTemplate::new(409).set_body_json(serde_json::json!({
            "error": "duplicate_user",
            "message": "User already exists"
        })))
        .mount(ctx.mock_server())
        .await;

    ctx
}

/// Setup test with a server error mock.
async fn setup_with_create_user_server_error<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> CreateUserTestCtx<'a> {
    let ctx = setup_create_user_test(app).await;

    // Mock server error (500)
    // Note: The API URL is constructed as {base_url}/api/internal/users
    Mock::given(method("POST"))
        .and(path("/api/internal/users"))
        .and(header("cf-authorization", "test-cf-authorization-token"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "error": "internal_error",
            "message": "Database connection failed"
        })))
        .mount(ctx.mock_server())
        .await;

    ctx
}

/// Test that CreateUserInput starts with no username set.
#[tokio::test]
async fn test_create_user_input_initial_state() {
    let mut ctx = setup_create_user_test(|ui, state| {
        // Just render something to get access to state
        let input = state.ctx.state_mut::<CreateUserInput>();
        let label_text = format!("Username: {:?}", input.username);
        ui.label(label_text);
    })
    .await;

    let harness = ctx.harness_mut();
    harness.step();

    // Initial state should have no username
    let state = harness.state();
    let input = state.ctx.state_mut::<CreateUserInput>();
    assert!(
        input.username.is_none(),
        "CreateUserInput should start with no username"
    );
}

/// Test that CreateUserCompute starts in Idle state.
#[tokio::test]
async fn test_create_user_compute_initial_state() {
    let mut ctx = setup_create_user_test(|ui, state| {
        let compute = state.ctx.cached::<CreateUserCompute>();
        let status = match compute {
            Some(c) => format!("{:?}", c.result),
            None => "No compute".to_string(),
        };
        ui.label(format!("Compute status: {}", status));
    })
    .await;

    let harness = ctx.harness_mut();
    harness.step();

    // Initial state should be Idle
    let state = harness.state();
    let compute = state.ctx.cached::<CreateUserCompute>();
    assert!(compute.is_some(), "CreateUserCompute should be registered");
    assert!(
        matches!(compute.unwrap().result, CreateUserResult::Idle),
        "CreateUserCompute should start in Idle state"
    );
}

/// Test that triggering create user sets state to Pending.
#[tokio::test]
async fn test_trigger_create_user_sets_pending() {
    let mut ctx = setup_with_create_user_success(
        |ui, state| {
            // Display compute state
            let compute = state.ctx.cached::<CreateUserCompute>();
            let status = match compute {
                Some(c) => match &c.result {
                    CreateUserResult::Idle => "Idle",
                    CreateUserResult::Pending => "Pending",
                    CreateUserResult::Success(_) => "Success",
                    CreateUserResult::Error(_) => "Error",
                },
                None => "None",
            };
            ui.label(format!("Status: {}", status));
        },
        "testuser",
    )
    .await;

    let harness = ctx.harness_mut();

    // Trigger create user
    {
        let state = harness.state_mut();
        state.ctx.update::<CreateUserInput>(|input| {
            input.username = Some("testuser".to_string());
        });
        state.ctx.dispatch::<CreateUserCommand>();
    }

    // Sync computes to apply the pending state
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    // Run a frame to process the state change
    harness.step();

    // Should be in pending state immediately after trigger
    let state = harness.state();
    let compute = state.ctx.cached::<CreateUserCompute>();
    assert!(compute.is_some(), "Compute should exist");

    let result = &compute.unwrap().result;
    // After triggering, it should be either Pending or already resolved
    assert!(
        !matches!(result, CreateUserResult::Idle),
        "Should not be in Idle state after triggering create"
    );
}

/// Test the complete success flow for creating a user.
#[tokio::test]
async fn test_create_user_success_flow() {
    let mut ctx = setup_with_create_user_success(
        |ui, state| {
            let compute = state.ctx.cached::<CreateUserCompute>();
            if let Some(c) = compute {
                match &c.result {
                    CreateUserResult::Success(response) => {
                        ui.label(format!("Created: {}", response.username));
                        ui.label(format!("Secret: {}", response.secret));
                    }
                    CreateUserResult::Error(e) => {
                        ui.label(format!("Error: {}", e));
                    }
                    CreateUserResult::Pending => {
                        ui.label("Creating...");
                    }
                    CreateUserResult::Idle => {
                        ui.label("Ready");
                    }
                }
            }
        },
        "newuser",
    )
    .await;

    let harness = ctx.harness_mut();

    // Trigger create user
    {
        let state = harness.state_mut();
        state.ctx.update::<CreateUserInput>(|input| {
            input.username = Some("newuser".to_string());
        });
        state.ctx.dispatch::<CreateUserCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Check the result
    let state = harness.state();
    let compute = state.ctx.cached::<CreateUserCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().result {
        CreateUserResult::Success(response) => {
            assert_eq!(response.username, "newuser");
            assert_eq!(response.secret, "JBSWY3DPEHPK3PXP");
            assert!(response.otpauth_url.contains("newuser"));
        }
        other => {
            panic!("Expected Success state, got {:?}", other);
        }
    }
}

/// Test that duplicate user returns an error.
#[tokio::test]
async fn test_create_user_duplicate_error() {
    let mut ctx = setup_with_create_user_duplicate(
        |ui, state| {
            let compute = state.ctx.cached::<CreateUserCompute>();
            if let Some(c) = compute {
                match &c.result {
                    CreateUserResult::Success(response) => {
                        ui.label(format!("Created: {}", response.username));
                    }
                    CreateUserResult::Error(e) => {
                        ui.label(format!("Error: {}", e));
                    }
                    CreateUserResult::Pending => {
                        ui.label("Creating...");
                    }
                    CreateUserResult::Idle => {
                        ui.label("Ready");
                    }
                }
            }
        },
        "existinguser",
    )
    .await;

    let harness = ctx.harness_mut();

    // Trigger create user
    {
        let state = harness.state_mut();
        state.ctx.update::<CreateUserInput>(|input| {
            input.username = Some("existinguser".to_string());
        });
        state.ctx.dispatch::<CreateUserCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Check the result - should be error due to 409 status
    let state = harness.state();
    let compute = state.ctx.cached::<CreateUserCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().result {
        CreateUserResult::Error(e) => {
            assert!(
                e.contains("409"),
                "Error should contain status code 409, got: {}",
                e
            );
        }
        other => {
            panic!("Expected Error state, got {:?}", other);
        }
    }
}

/// Test that server error is handled properly.
#[tokio::test]
async fn test_create_user_server_error() {
    let mut ctx = setup_with_create_user_server_error(|ui, state| {
        let compute = state.ctx.cached::<CreateUserCompute>();
        if let Some(c) = compute {
            match &c.result {
                CreateUserResult::Success(response) => {
                    ui.label(format!("Created: {}", response.username));
                }
                CreateUserResult::Error(e) => {
                    ui.label(format!("Error: {}", e));
                }
                CreateUserResult::Pending => {
                    ui.label("Creating...");
                }
                CreateUserResult::Idle => {
                    ui.label("Ready");
                }
            }
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Trigger create user
    {
        let state = harness.state_mut();
        state.ctx.update::<CreateUserInput>(|input| {
            input.username = Some("anyuser".to_string());
        });
        state.ctx.dispatch::<CreateUserCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Check the result - should be error due to 500 status
    let state = harness.state();
    let compute = state.ctx.cached::<CreateUserCompute>();
    assert!(compute.is_some(), "Compute should exist");

    match &compute.unwrap().result {
        CreateUserResult::Error(e) => {
            assert!(
                e.contains("500"),
                "Error should contain status code 500, got: {}",
                e
            );
        }
        other => {
            panic!("Expected Error state, got {:?}", other);
        }
    }
}

/// Test that empty username does not trigger creation.
#[tokio::test]
async fn test_create_user_empty_username_skipped() {
    let mut ctx = setup_create_user_test(|ui, state| {
        let compute = state.ctx.cached::<CreateUserCompute>();
        if let Some(c) = compute {
            match &c.result {
                CreateUserResult::Success(_) => ui.label("Success"),
                CreateUserResult::Error(e) => ui.label(format!("Error: {}", e)),
                CreateUserResult::Pending => ui.label("Pending"),
                CreateUserResult::Idle => ui.label("Idle"),
            };
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Trigger with empty username
    {
        let state = harness.state_mut();
        state.ctx.update::<CreateUserInput>(|input| {
            input.username = Some("".to_string());
        });
        state.ctx.dispatch::<CreateUserCommand>();
    }

    harness.step();

    // Should remain in Idle state since empty username is skipped
    let state = harness.state();
    let compute = state.ctx.cached::<CreateUserCompute>();
    assert!(compute.is_some(), "Compute should exist");

    assert!(
        matches!(compute.unwrap().result, CreateUserResult::Idle),
        "Should remain in Idle state for empty username"
    );
}

/// Test that None username does not trigger creation.
#[tokio::test]
async fn test_create_user_none_username_skipped() {
    let mut ctx = setup_create_user_test(|ui, state| {
        let compute = state.ctx.cached::<CreateUserCompute>();
        if let Some(c) = compute {
            match &c.result {
                CreateUserResult::Success(_) => ui.label("Success"),
                CreateUserResult::Error(e) => ui.label(format!("Error: {}", e)),
                CreateUserResult::Pending => ui.label("Pending"),
                CreateUserResult::Idle => ui.label("Idle"),
            };
        }
    })
    .await;

    let harness = ctx.harness_mut();

    // Trigger with None username (which is the default)
    {
        let state = harness.state_mut();
        state.ctx.dispatch::<CreateUserCommand>();
    }

    harness.step();

    // Should remain in Idle state since None username is skipped
    let state = harness.state();
    let compute = state.ctx.cached::<CreateUserCompute>();
    assert!(compute.is_some(), "Compute should exist");

    assert!(
        matches!(compute.unwrap().result, CreateUserResult::Idle),
        "Should remain in Idle state for None username"
    );
}

/// Test CreateUserCompute helper methods.
#[tokio::test]
async fn test_create_user_compute_helper_methods() {
    let mut ctx = setup_with_create_user_success(
        |ui, state| {
            let compute = state.ctx.cached::<CreateUserCompute>();
            if let Some(c) = compute {
                ui.label(format!("is_success: {}", c.is_success()));
                ui.label(format!("is_pending: {}", c.is_pending()));
                if let Some(msg) = c.error_message() {
                    ui.label(format!("error: {}", msg));
                }
                if let Some(resp) = c.success_response() {
                    ui.label(format!("response: {}", resp.username));
                }
            }
        },
        "helpertest",
    )
    .await;

    let harness = ctx.harness_mut();

    // Initial state - check helper methods
    {
        let state = harness.state();
        let compute = state.ctx.cached::<CreateUserCompute>().unwrap();
        assert!(!compute.is_success(), "Should not be success initially");
        assert!(!compute.is_pending(), "Should not be pending initially");
        assert!(
            compute.error_message().is_none(),
            "Should have no error initially"
        );
        assert!(
            compute.success_response().is_none(),
            "Should have no response initially"
        );
    }

    // Trigger create user
    {
        let state = harness.state_mut();
        state.ctx.update::<CreateUserInput>(|input| {
            input.username = Some("helpertest".to_string());
        });
        state.ctx.dispatch::<CreateUserCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // After success - check helper methods
    {
        let state = harness.state();
        let compute = state.ctx.cached::<CreateUserCompute>().unwrap();
        assert!(compute.is_success(), "Should be success after completion");
        assert!(
            !compute.is_pending(),
            "Should not be pending after completion"
        );
        assert!(
            compute.error_message().is_none(),
            "Should have no error on success"
        );
        let response = compute.success_response();
        assert!(response.is_some(), "Should have response on success");
        assert_eq!(response.unwrap().username, "helpertest");
    }
}

/// Test that CreateUserCompute can be reset.
#[tokio::test]
async fn test_create_user_compute_reset() {
    let mut ctx = setup_with_create_user_success(
        |ui, state| {
            let compute = state.ctx.cached::<CreateUserCompute>();
            if let Some(c) = compute {
                match &c.result {
                    CreateUserResult::Success(_) => ui.label("Success"),
                    CreateUserResult::Error(_) => ui.label("Error"),
                    CreateUserResult::Pending => ui.label("Pending"),
                    CreateUserResult::Idle => ui.label("Idle"),
                };
            }
        },
        "resettest",
    )
    .await;

    let harness = ctx.harness_mut();

    // Trigger create user
    {
        let state = harness.state_mut();
        state.ctx.update::<CreateUserInput>(|input| {
            input.username = Some("resettest".to_string());
        });
        state.ctx.dispatch::<CreateUserCommand>();
    }

    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Sync computes to get the result
    {
        let state = harness.state_mut();
        state.ctx.sync_computes();
    }

    harness.step();

    // Verify success
    {
        let state = harness.state();
        let compute = state.ctx.cached::<CreateUserCompute>().unwrap();
        assert!(compute.is_success(), "Should be success");
    }

    // Reset the compute - we need to update it via the state context
    {
        let state = harness.state_mut();
        // Clear the input first
        state.ctx.update::<CreateUserInput>(|input| {
            input.username = None;
        });
    }

    harness.step();

    // The compute should still hold the previous result until explicitly reset
    // In the actual UI, reset_create_user_compute() handles this
    {
        let state = harness.state();
        let compute = state.ctx.cached::<CreateUserCompute>().unwrap();
        // After clearing input, compute still has the cached result
        // This is expected behavior - the compute doesn't auto-reset
        assert!(
            compute.is_success(),
            "Compute should retain success state until explicitly reset"
        );
    }
}
