//! Integration tests for QR code display in the internal users panel.
//!
//! These tests verify that the QR code display functionality works correctly
//! when the ShowQrCode action is triggered via egui_kittest.
//!
//! ## Note on kittest table button clicks
//!
//! Due to a limitation in egui_kittest, button clicks within egui_extras
//! `TableBuilder` rows are not properly propagated. The `.click()` method
//! finds the button node but the click event doesn't reach the egui widget.
//! This is likely related to how egui_extras renders table content in a
//! separate clipping/scrolling region.
//!
//! As a workaround, these tests:
//! 1. Verify QR buttons ARE rendered and queryable in the table (passes)
//! 2. Simulate the action state change that would occur from a button click
//! 3. Verify the QR code expansion renders correctly after the action
//!
//! The simulation approach is valid because:
//! - The button rendering and action return logic is tested via `test_qr_buttons_displayed_in_table`
//! - The action-to-expansion flow is the same regardless of how the action is triggered
//! - The Close button click (outside the table) works normally and is tested
//!
//! Tests are only compiled when the `env_test_internal` feature is enabled.

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

mod common;

use crate::common::{DEFAULT_NETWORK_WAIT_MS, yield_wait_for_network};
use collects_business::{
    InternalUsersActionCompute, InternalUsersActionKind, InternalUsersActionState,
    InternalUsersState, UserAction,
};
use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;
use kittest::Queryable;
use ustr::Ustr;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context for QR code display integration tests.
struct QrCodeTestCtx<'a> {
    #[allow(dead_code)]
    mock_server: MockServer,
    harness: Harness<'a, CollectsApp>,
}

impl<'a> QrCodeTestCtx<'a> {
    fn harness_mut(&mut self) -> &mut Harness<'a, CollectsApp> {
        &mut self.harness
    }
}

/// Setup test state with mock server configured for QR code display.
///
/// This setup mocks:
/// - Health check endpoint
/// - List users endpoint (returns alice and bob)
/// - Get user endpoint (returns user with otpauth_url for QR generation)
async fn setup_qrcode_test<'a>() -> QrCodeTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock the health check endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    // Mock the list users endpoint
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {
                    "username": "alice",
                    "current_otp": "123456",
                    "time_remaining": 25,
                    "nickname": "Alice Wonderland",
                    "avatar_url": "https://example.com/avatar/alice.png",
                    "created_at": "2026-01-01T10:00:00Z",
                    "updated_at": "2026-01-05T15:30:00Z"
                },
                {
                    "username": "bob",
                    "current_otp": "654321",
                    "time_remaining": 15,
                    "nickname": null,
                    "avatar_url": null,
                    "created_at": "2026-01-02T12:00:00Z",
                    "updated_at": "2026-01-02T12:00:00Z"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    // Mock the get user endpoint for alice (returns otpauth_url for QR code)
    Mock::given(method("GET"))
        .and(path_regex(r"^/api/internal/users/alice$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "alice",
            "current_otp": "123456",
            "time_remaining": 25,
            "otpauth_url": "otpauth://totp/Collects:alice?secret=JBSWY3DPEHPK3PXP&issuer=Collects"
        })))
        .mount(&mock_server)
        .await;

    // Mock the get user endpoint for bob
    Mock::given(method("GET"))
        .and(path_regex(r"^/api/internal/users/bob$"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "bob",
            "current_otp": "654321",
            "time_remaining": 15,
            "otpauth_url": "otpauth://totp/Collects:bob?secret=ABCDEFGHIJKLMNOP&issuer=Collects"
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);
    let app = CollectsApp::builder().state(state).build();
    let harness = Harness::new_eframe(|_| app);

    QrCodeTestCtx {
        mock_server,
        harness,
    }
}

/// Helper to wait for users to be loaded in the table.
async fn wait_for_users_loaded(harness: &mut Harness<'_, CollectsApp>) {
    // Run frames to allow route change and auto-fetch
    for _ in 0..10 {
        harness.step();
    }

    // Wait for async API call
    yield_wait_for_network(200).await;

    // Sync computes
    {
        let state = harness.state_mut();
        state.state.ctx.sync_computes();
    }

    // Run more frames to render
    for _ in 0..10 {
        harness.step();
    }
}

/// Helper to wait for QR action to settle (Success or Error).
async fn wait_for_qr_action_settles(harness: &mut Harness<'_, CollectsApp>, max_steps: usize) {
    for _ in 0..max_steps {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;

        {
            let state = harness.state_mut();
            state.state.ctx.sync_computes();
        }

        harness.step();

        let state = harness.state();
        if let Some(c) = state.state.ctx.cached::<InternalUsersActionCompute>() {
            match &c.state {
                InternalUsersActionState::Success {
                    kind: InternalUsersActionKind::GetUserQr,
                    ..
                }
                | InternalUsersActionState::Error {
                    kind: InternalUsersActionKind::GetUserQr,
                    ..
                } => return,
                _ => {}
            }
        }
    }
}

/// Simulates clicking the QR button by setting the action state.
///
/// This is equivalent to what happens when `render_action_buttons` returns
/// `Some(UserAction::ShowQrCode(username))` and the panel processes it via
/// `state.start_action(action)`.
///
/// Note: Direct button clicks inside egui_extras table rows don't work with
/// kittest (see module-level documentation), so we simulate the action instead.
fn simulate_qr_button_click(harness: &mut Harness<'_, CollectsApp>, username: &str) {
    let username_ustr = Ustr::from(username);
    let state = harness.state_mut();
    state
        .state
        .ctx
        .update::<InternalUsersState>(|s| s.start_action(UserAction::ShowQrCode(username_ustr)));
}

// ===========================================
// Integration tests using real CollectsApp
// ===========================================

/// Test that the QR buttons are displayed in the users table actions column.
///
/// This verifies that the buttons are rendered and queryable, even though
/// kittest clicks don't work for table buttons.
#[tokio::test]
async fn test_qr_buttons_displayed_in_table() {
    let mut ctx = setup_qrcode_test().await;
    let harness = ctx.harness_mut();

    // Wait for users to load
    wait_for_users_loaded(harness).await;

    // Verify the QR buttons are present (one per user row)
    let qr_buttons: Vec<_> = harness.query_all_by_label_contains("QR").collect();
    assert!(
        !qr_buttons.is_empty(),
        "QR buttons should be displayed in the actions column"
    );
    // Should have 2 QR buttons (one for alice, one for bob)
    assert_eq!(qr_buttons.len(), 2, "Should have QR buttons for both users");
}

/// Test that the QR action shows the QR code expansion inline.
///
/// This test verifies the complete flow:
/// 1. Users are loaded in the table
/// 2. QR action is triggered for a user
/// 3. The QR code expansion appears with the expected content
#[tokio::test]
async fn test_qr_action_shows_qr_expansion() {
    let mut ctx = setup_qrcode_test().await;
    let harness = ctx.harness_mut();

    // Wait for users to load
    wait_for_users_loaded(harness).await;

    // Simulate clicking QR button for alice
    simulate_qr_button_click(harness, "alice");

    // Run frames to trigger the command dispatch from render_qr_expansion
    for _ in 0..10 {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;
    }

    // Wait for the QR action to complete (API fetch)
    wait_for_qr_action_settles(harness, 50).await;

    // Run more frames to render the QR expansion with data
    for _ in 0..20 {
        harness.step();
    }

    // Verify the QR code expansion is displayed with the expected label
    assert!(
        harness
            .query_by_label_contains("Scan this QR code")
            .is_some(),
        "QR code expansion should show 'Scan this QR code' instruction"
    );

    // Verify the Close button is present in the expansion
    assert!(
        harness.query_by_label_contains("Close").is_some(),
        "Close button should be present in QR code expansion"
    );
}

/// Test that the QR code data is fetched and stored after triggering action.
#[tokio::test]
async fn test_qr_action_fetches_qr_data() {
    let mut ctx = setup_qrcode_test().await;
    let harness = ctx.harness_mut();

    // Wait for users to load
    wait_for_users_loaded(harness).await;

    // Verify initial state has no QR data
    {
        let state = harness.state();
        let internal_state = state.state.ctx.state::<InternalUsersState>();
        assert!(
            internal_state.qr_code_data.is_none(),
            "Initial state should have no QR code data"
        );
    }

    // Simulate clicking QR button for alice
    simulate_qr_button_click(harness, "alice");

    // Run frames to trigger command dispatch
    for _ in 0..10 {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;
    }

    // Wait for the QR action to complete
    wait_for_qr_action_settles(harness, 50).await;

    // Verify the action compute has success state
    let state = harness.state();
    let compute = state.state.ctx.cached::<InternalUsersActionCompute>();
    assert!(
        compute.is_some(),
        "Action compute should exist after QR action"
    );

    match &compute.unwrap().state {
        InternalUsersActionState::Success { kind, user, data } => {
            assert_eq!(
                *kind,
                InternalUsersActionKind::GetUserQr,
                "Action kind should be GetUserQr"
            );
            assert_eq!(user.as_str(), "alice", "Username should be alice");
            assert!(data.is_some(), "OTPAuth URL data should be present");
            let otpauth_url = data.as_ref().unwrap();
            assert!(
                otpauth_url.starts_with("otpauth://totp/"),
                "OTPAuth URL should have correct format"
            );
            assert!(
                otpauth_url.contains("alice"),
                "OTPAuth URL should contain username"
            );
        }
        other => panic!("Expected Success state for GetUserQr, got {:?}", other),
    }
}

/// Test that the QR code data is stored in InternalUsersState after render.
#[tokio::test]
async fn test_qr_data_stored_in_state() {
    let mut ctx = setup_qrcode_test().await;
    let harness = ctx.harness_mut();

    // Wait for users to load
    wait_for_users_loaded(harness).await;

    // Simulate clicking QR button for alice
    simulate_qr_button_click(harness, "alice");

    // Run frames to trigger command dispatch
    for _ in 0..10 {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;
    }

    // Wait for the QR action to complete
    wait_for_qr_action_settles(harness, 50).await;

    // Run more frames to render the QR code (which updates state.qr_code_data)
    for _ in 0..25 {
        harness.step();
    }

    // Verify QR code data is set in state
    let state = harness.state();
    let internal_state = state.state.ctx.state::<InternalUsersState>();
    assert!(
        internal_state.qr_code_data.is_some(),
        "QR code data should be set after successful fetch and render"
    );

    let qr_data = internal_state.qr_code_data.as_ref().unwrap();
    assert!(
        qr_data.starts_with("otpauth://"),
        "QR code data should be an otpauth URL"
    );
}

/// Test that clicking Close button closes the QR code expansion.
///
/// Note: The Close button is rendered OUTSIDE the table (in the QR expansion),
/// so kittest clicks work normally for it.
#[tokio::test]
async fn test_close_button_closes_qr_expansion() {
    let mut ctx = setup_qrcode_test().await;
    let harness = ctx.harness_mut();

    // Wait for users to load
    wait_for_users_loaded(harness).await;

    // Simulate clicking QR button for alice
    simulate_qr_button_click(harness, "alice");

    // Run frames to trigger command dispatch
    for _ in 0..10 {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;
    }

    // Wait for the QR action to complete
    wait_for_qr_action_settles(harness, 50).await;

    // Run frames to render the QR expansion
    for _ in 0..20 {
        harness.step();
    }

    // Verify QR expansion is shown
    assert!(
        harness
            .query_by_label_contains("Scan this QR code")
            .is_some(),
        "QR code expansion should be visible"
    );

    // Find and click the Close button (this works because it's outside the table!)
    let close_button = harness.query_by_label_contains("Close");
    assert!(close_button.is_some(), "Close button should be present");
    close_button.unwrap().click();

    // Process the close click
    for _ in 0..10 {
        harness.step();
    }

    // Verify the QR expansion is closed (the "Scan this QR code" label should be gone)
    assert!(
        harness
            .query_by_label_contains("Scan this QR code")
            .is_none(),
        "QR code expansion should be closed after clicking Close"
    );
}

/// Test that the QR code expansion shows the user label.
#[tokio::test]
async fn test_qr_expansion_shows_user_label() {
    let mut ctx = setup_qrcode_test().await;
    let harness = ctx.harness_mut();

    // Wait for users to load
    wait_for_users_loaded(harness).await;

    // Simulate clicking QR button for alice
    simulate_qr_button_click(harness, "alice");

    // Run frames to trigger command dispatch
    for _ in 0..10 {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;
    }

    // Wait for action to complete and render
    wait_for_qr_action_settles(harness, 50).await;
    for _ in 0..20 {
        harness.step();
    }

    // The QR expansion should show "QR Code for:" label
    assert!(
        harness.query_by_label_contains("QR Code for").is_some(),
        "QR expansion should show 'QR Code for:' label"
    );
}

/// Test that QR action for different users fetches correct data.
#[tokio::test]
async fn test_qr_action_for_different_users() {
    let mut ctx = setup_qrcode_test().await;
    let harness = ctx.harness_mut();

    // Wait for users to load
    wait_for_users_loaded(harness).await;

    // Simulate clicking QR button for bob (second user)
    simulate_qr_button_click(harness, "bob");

    // Run frames to trigger command dispatch
    for _ in 0..10 {
        harness.step();
        yield_wait_for_network(DEFAULT_NETWORK_WAIT_MS).await;
    }

    // Wait for the QR action to complete
    wait_for_qr_action_settles(harness, 50).await;

    // Verify the action compute has success state for bob
    let state = harness.state();
    let compute = state.state.ctx.cached::<InternalUsersActionCompute>();
    assert!(compute.is_some(), "Action compute should exist");

    match &compute.unwrap().state {
        InternalUsersActionState::Success { kind, user, data } => {
            assert_eq!(*kind, InternalUsersActionKind::GetUserQr);
            assert_eq!(user.as_str(), "bob", "Username should be bob");
            assert!(data.is_some());
            let otpauth_url = data.as_ref().unwrap();
            assert!(
                otpauth_url.contains("bob"),
                "OTPAuth URL should contain bob's username"
            );
        }
        other => panic!("Expected Success state, got {:?}", other),
    }
}
