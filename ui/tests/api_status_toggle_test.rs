//! Integration tests for API status toggle functionality (F1 key).
//!
//! These tests verify:
//! 1. API status panel is hidden by default
//! 2. ToggleApiStatusCommand correctly toggles visibility
//! 3. The show_status flag persists through API compute updates

use collects_business::{ApiStatus, ToggleApiStatusCommand};
use collects_ui::state::State;
use egui_kittest::Harness;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context for toggle tests.
struct ToggleTestCtx<'a> {
    _mock_server: MockServer,
    harness: Harness<'a, State>,
}

impl<'a> ToggleTestCtx<'a> {
    fn harness_mut(&mut self) -> &mut Harness<'a, State> {
        &mut self.harness
    }
}

/// Setup test state with mock server.
async fn setup_toggle_test<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> ToggleTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    // Mock the internal users endpoint (needed when internal features are enabled)
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": []
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);

    let harness = Harness::new_ui_state(app, state);

    ToggleTestCtx {
        _mock_server: mock_server,
        harness,
    }
}

// =============================================================================
// TOGGLE FUNCTIONALITY TESTS
// =============================================================================

/// Tests that API status panel is hidden by default.
#[tokio::test]
async fn test_api_status_hidden_by_default() {
    let mut ctx = setup_toggle_test(|_ui, _state| {}).await;
    let harness = ctx.harness_mut();

    harness.step();

    // By default, show_status should be false
    let show_status = harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|api| api.show_status())
        .unwrap_or(true); // Default to true to fail if not found

    assert!(!show_status, "API status should be hidden by default");
}

/// Tests that ToggleApiStatusCommand toggles the visibility from off to on.
#[tokio::test]
async fn test_toggle_shows_api_status() {
    let mut ctx = setup_toggle_test(|_ui, _state| {}).await;
    let harness = ctx.harness_mut();

    harness.step();

    // Verify initially hidden
    let initial = harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|api| api.show_status())
        .unwrap_or(true);
    assert!(!initial, "Should be hidden initially");

    // Dispatch toggle command
    harness.state_mut().ctx.dispatch::<ToggleApiStatusCommand>();
    harness.state_mut().ctx.sync_computes();
    harness.step();

    // Should now be visible
    let after_toggle = harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|api| api.show_status())
        .unwrap_or(false);
    assert!(after_toggle, "API status should be visible after toggle");
}

/// Tests that ToggleApiStatusCommand toggles the visibility from on to off.
#[tokio::test]
async fn test_toggle_hides_api_status() {
    let mut ctx = setup_toggle_test(|_ui, _state| {}).await;
    let harness = ctx.harness_mut();

    harness.step();

    // Toggle on
    harness.state_mut().ctx.dispatch::<ToggleApiStatusCommand>();
    harness.state_mut().ctx.sync_computes();
    harness.step();

    // Verify visible
    let visible = harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|api| api.show_status())
        .unwrap_or(false);
    assert!(visible, "Should be visible after first toggle");

    // Toggle off
    harness.state_mut().ctx.dispatch::<ToggleApiStatusCommand>();
    harness.state_mut().ctx.sync_computes();
    harness.step();

    // Should now be hidden
    let hidden = harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|api| api.show_status())
        .unwrap_or(true);
    assert!(!hidden, "API status should be hidden after second toggle");
}

/// Tests multiple toggles work correctly.
#[tokio::test]
async fn test_multiple_toggles() {
    let mut ctx = setup_toggle_test(|_ui, _state| {}).await;
    let harness = ctx.harness_mut();

    harness.step();

    // Toggle 10 times and verify the state alternates correctly
    for i in 0..10 {
        harness.state_mut().ctx.dispatch::<ToggleApiStatusCommand>();
        harness.state_mut().ctx.sync_computes();
        harness.step();

        let expected = (i + 1) % 2 == 1; // odd iterations: visible, even: hidden
        let actual = harness
            .state()
            .ctx
            .cached::<ApiStatus>()
            .map(|api| api.show_status())
            .unwrap_or(!expected);

        assert_eq!(
            actual,
            expected,
            "After {} toggles, show_status should be {}",
            i + 1,
            expected
        );
    }
}

/// Tests that show_status is preserved when ApiStatus is updated from API response.
#[tokio::test]
async fn test_show_status_preserved_after_compute_update() {
    let mut ctx = setup_toggle_test(|_ui, _state| {}).await;
    let harness = ctx.harness_mut();

    harness.step();

    // Toggle on
    harness.state_mut().ctx.dispatch::<ToggleApiStatusCommand>();
    harness.state_mut().ctx.sync_computes();
    harness.step();

    // Verify visible
    let before_fetch = harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|api| api.show_status())
        .unwrap_or(false);
    assert!(before_fetch, "Should be visible before API fetch");

    // Trigger API compute cycle
    harness.state_mut().ctx.run_all_dirty();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    harness.state_mut().ctx.sync_computes();
    harness.step();

    // show_status should still be true after API response updates ApiStatus
    let after_fetch = harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|api| api.show_status())
        .unwrap_or(false);
    assert!(
        after_fetch,
        "show_status should be preserved after API fetch"
    );
}
