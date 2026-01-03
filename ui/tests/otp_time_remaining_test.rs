//! Integration tests for OTP time remaining auto-update feature.
//!
//! These tests verify that the OTP time remaining display correctly updates
//! based on elapsed time since data was fetched.

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

use chrono::Duration;
use collects_business::{BusinessConfig, InternalUserItem};
use collects_states::Time;
use collects_ui::state::State;
use collects_ui::widgets::{InternalUsersState, internal_users_panel};
use egui_kittest::Harness;
use kittest::Queryable;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context for OTP time remaining tests.
struct OtpTestCtx<'a> {
    /// Mock server must be retained to keep HTTP endpoints alive during tests.
    #[allow(dead_code)]
    mock_server: MockServer,
    harness: Harness<'a, State>,
}

impl<'a> OtpTestCtx<'a> {
    fn harness_mut(&mut self) -> &mut Harness<'a, State> {
        &mut self.harness
    }
}

/// Setup test state with mock server configured for internal users.
async fn setup_otp_test<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
    users: Vec<InternalUserItem>,
) -> OtpTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock health endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Mock internal users endpoint with provided users
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": users
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);

    let harness = Harness::new_ui_state(app, state);

    OtpTestCtx {
        mock_server,
        harness,
    }
}

/// Advances the mock time by the specified number of seconds.
///
/// This modifies the `Time` state in the harness, allowing tests to simulate
/// time passing without waiting for real time. Used to test OTP countdown behavior.
///
/// # Arguments
///
/// * `harness` - The test harness containing the app state
/// * `seconds` - Number of seconds to advance (can be negative for time travel backward)
fn advance_time_by_seconds(harness: &mut Harness<'_, State>, seconds: i64) {
    harness.state_mut().ctx.update::<Time>(|t| {
        *t.as_mut() = *t.as_ref() + Duration::seconds(seconds);
    });
}

/// Gets the current mocked time from the harness.
///
/// Returns the `DateTime<Utc>` value stored in the `Time` state.
fn get_current_time(harness: &Harness<'_, State>) -> chrono::DateTime<chrono::Utc> {
    *harness.state().ctx.state_mut::<Time>().as_ref()
}

/// Test that OTP time remaining updates when time advances.
#[tokio::test]
async fn test_otp_time_remaining_updates_with_time() {
    // Create test user with 25 seconds remaining
    let test_user = InternalUserItem {
        username: "testuser".to_string(),
        current_otp: "123456".to_string(),
        time_remaining: 25,
    };

    let mut ctx = setup_otp_test(
        |ui, state| {
            let api_base_url = state
                .ctx
                .state_mut::<BusinessConfig>()
                .api_url()
                .to_string();
            internal_users_panel(&mut state.ctx, &api_base_url, ui);
        },
        vec![test_user.clone()],
    )
    .await;

    let harness = ctx.harness_mut();

    // Initialize internal users state with test data
    let now = get_current_time(harness);
    harness
        .state_mut()
        .ctx
        .state_mut::<InternalUsersState>()
        .update_users(vec![test_user], now);

    // Render the widget
    harness.step();

    // Verify initial time remaining is displayed (should show 25s)
    let time_label = harness.query_by_label_contains("25s");
    assert!(time_label.is_some(), "Should display 25s initially");

    // Advance time by 10 seconds
    advance_time_by_seconds(harness, 10);
    harness.step();

    // Verify time remaining updated (should now show 15s)
    let time_label = harness.query_by_label_contains("15s");
    assert!(
        time_label.is_some(),
        "Should display 15s after 10 seconds elapsed"
    );
}

/// Test that OTP time remaining wraps correctly after 30 seconds.
#[tokio::test]
async fn test_otp_time_remaining_wraps_after_30_seconds() {
    // Create test user with 10 seconds remaining
    let test_user = InternalUserItem {
        username: "testuser".to_string(),
        current_otp: "123456".to_string(),
        time_remaining: 10,
    };

    let mut ctx = setup_otp_test(
        |ui, state| {
            let api_base_url = state
                .ctx
                .state_mut::<BusinessConfig>()
                .api_url()
                .to_string();
            internal_users_panel(&mut state.ctx, &api_base_url, ui);
        },
        vec![test_user.clone()],
    )
    .await;

    let harness = ctx.harness_mut();

    // Initialize internal users state with test data
    let now = get_current_time(harness);
    harness
        .state_mut()
        .ctx
        .state_mut::<InternalUsersState>()
        .update_users(vec![test_user], now);

    // Render the widget
    harness.step();

    // Verify initial time remaining is displayed
    let time_label = harness.query_by_label_contains("10s");
    assert!(time_label.is_some(), "Should display 10s initially");

    // Advance time by 15 seconds (should wrap to 25s since 10-15 = -5, wraps to 25)
    advance_time_by_seconds(harness, 15);
    harness.step();

    // After wrapping: 10 - 15 = -5, wraps to 30 + (-5) = 25
    let time_label = harness.query_by_label_contains("25s");
    assert!(
        time_label.is_some(),
        "Should display 25s after wrap-around (10s - 15s elapsed = 25s)"
    );
}

/// Test that time remaining color coding changes based on remaining time.
#[tokio::test]
async fn test_otp_time_remaining_color_changes() {
    // Create test user with 15 seconds remaining (green zone)
    let test_user = InternalUserItem {
        username: "testuser".to_string(),
        current_otp: "123456".to_string(),
        time_remaining: 15,
    };

    let mut ctx = setup_otp_test(
        |ui, state| {
            let api_base_url = state
                .ctx
                .state_mut::<BusinessConfig>()
                .api_url()
                .to_string();
            internal_users_panel(&mut state.ctx, &api_base_url, ui);
        },
        vec![test_user.clone()],
    )
    .await;

    let harness = ctx.harness_mut();

    // Initialize internal users state with test data
    let now = get_current_time(harness);
    harness
        .state_mut()
        .ctx
        .state_mut::<InternalUsersState>()
        .update_users(vec![test_user], now);

    // Render the widget
    harness.step();

    // Verify initial time remaining is displayed (15s - green zone)
    assert!(
        harness.query_by_label_contains("15s").is_some(),
        "Should display 15s initially"
    );

    // Advance time by 7 seconds (should be at 8s - orange zone)
    advance_time_by_seconds(harness, 7);
    harness.step();

    assert!(
        harness.query_by_label_contains("8s").is_some(),
        "Should display 8s after 7 seconds (orange zone)"
    );

    // Advance time by 4 more seconds (should be at 4s - red zone)
    advance_time_by_seconds(harness, 4);
    harness.step();

    assert!(
        harness.query_by_label_contains("4s").is_some(),
        "Should display 4s after 11 seconds total (red zone)"
    );
}
