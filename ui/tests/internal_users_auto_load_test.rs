//! Tests for internal users auto-loading behavior.
//!
//! Verifies that:
//! 1. Users are automatically loaded on first panel render
//! 2. No repeat fetch happens unless refresh button is clicked
//!
//! Tests are only compiled when the `env_test_internal` feature is enabled.
//!
//! Note: InternalApiStatus compute also calls /api/internal/users for health checking.
//! So tests need to account for both InternalApiStatus + panel auto-fetch calls.

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

use collects_business::ListUsersResponse;
use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;
use kittest::Queryable;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context for internal users auto-load tests.
struct AutoLoadTestCtx<'a> {
    #[allow(dead_code)]
    mock_server: MockServer,
    harness: Harness<'a, CollectsApp>,
}

impl<'a> AutoLoadTestCtx<'a> {
    /// Get mutable reference to the harness.
    fn harness_mut(&mut self) -> &mut Harness<'a, CollectsApp> {
        &mut self.harness
    }
}

/// Setup test state with mock server and users endpoint mock already mounted.
/// This ensures the mock is ready before the harness is created.
///
/// Note: `users_mock_expect` should account for:
/// - 1 call from InternalApiStatus compute (health check)
/// - Additional calls from panel (auto-fetch, refresh clicks, etc.)
async fn setup_auto_load_test_with_users_mock(
    users_mock_expect: wiremock::Times,
) -> AutoLoadTestCtx<'static> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock the health check endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Mock users list endpoint - mount BEFORE creating state/harness
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(ListUsersResponse {
            users: vec![],
        }))
        .expect(users_mock_expect)
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);
    let app = CollectsApp::new(state);
    let harness = Harness::new_eframe(|_| app);

    AutoLoadTestCtx {
        mock_server,
        harness,
    }
}

/// Test that users are automatically fetched on first panel render.
///
/// This test verifies that when the internal_users_panel is first rendered,
/// it automatically triggers a fetch if no data has been loaded yet.
///
/// Expected calls: 2
/// - 1 from InternalApiStatus compute (health check on app init)
/// - 1 from panel auto-fetch (on first render)
#[tokio::test]
async fn test_auto_fetch_on_first_render() {
    // Expect 2 calls: InternalApiStatus + panel auto-fetch
    let mut ctx = setup_auto_load_test_with_users_mock(2.into()).await;

    let harness = ctx.harness_mut();

    // First frame render - this should trigger auto-fetch from panel
    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Poll for responses - step triggers update which polls
    harness.step();

    // The mock expectation will verify exactly 2 calls were made when the mock server drops
}

/// Test that no repeat fetch happens on subsequent renders.
///
/// This test verifies that after the initial auto-fetch, subsequent renders
/// of the panel do NOT trigger additional fetches unless the refresh button
/// is explicitly clicked.
///
/// Expected calls: 2
/// - 1 from InternalApiStatus compute (health check on app init)
/// - 1 from panel auto-fetch (on first render only, not on subsequent renders)
#[tokio::test]
async fn test_no_repeat_fetch_on_subsequent_renders() {
    // Expect 2 calls: InternalApiStatus + panel auto-fetch (no repeats)
    let mut ctx = setup_auto_load_test_with_users_mock(2.into()).await;

    let harness = ctx.harness_mut();

    // First frame - triggers auto-fetch from panel
    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Second frame - should poll and complete fetch
    harness.step();

    // Third frame - should NOT trigger another fetch
    harness.step();

    // Fourth frame - should NOT trigger another fetch
    harness.step();

    // Fifth frame - should NOT trigger another fetch
    harness.step();

    // The mock expects exactly 2 calls - if more occurred, the test will fail on drop
}

/// Test that clicking the Refresh button triggers a new fetch.
///
/// This test verifies that clicking the refresh button works
/// to trigger additional fetches after the initial auto-fetch.
///
/// Expected calls: 3
/// - 1 from InternalApiStatus compute (health check on app init)
/// - 1 from panel auto-fetch (on first render)
/// - 1 from refresh button click
#[tokio::test]
async fn test_refresh_button_triggers_new_fetch() {
    // Expect 3 calls: InternalApiStatus + panel auto-fetch + refresh click
    let mut ctx = setup_auto_load_test_with_users_mock(3.into()).await;

    let harness = ctx.harness_mut();

    // First frame - triggers auto-fetch from panel
    harness.step();

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Second frame - polls and completes first fetch
    harness.step();

    // Click the refresh button to trigger third fetch
    let refresh_button = harness.query_by_label("🔄 Refresh");
    assert!(
        refresh_button.is_some(),
        "Refresh button should be present"
    );
    refresh_button.unwrap().click();

    // Frame to process the click
    harness.step();

    // Wait for second async response
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Frame to poll second response
    harness.step();

    // The mock expects exactly 3 calls - verification happens on drop
}
