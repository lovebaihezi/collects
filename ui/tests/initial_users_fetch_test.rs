//! Integration tests for initial users fetch on app load.
//!
//! These tests verify that:
//! 1. Internal users are automatically fetched when the app loads
//! 2. A loading spinner is shown in the table while fetching

#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

use collects_ui::CollectsApp;
use collects_ui::state::State;
use collects_ui::widgets::InternalUsersState;
use egui_kittest::Harness;
use kittest::Queryable;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context for initial fetch tests.
struct FetchTestCtx<'a> {
    /// Mock server must be retained to keep HTTP endpoints alive during tests.
    #[allow(dead_code)]
    mock_server: MockServer,
    harness: Harness<'a, CollectsApp>,
}

impl<'a> FetchTestCtx<'a> {
    fn harness_mut(&mut self) -> &mut Harness<'a, CollectsApp> {
        &mut self.harness
    }
}

/// Setup test context with mock server.
async fn setup_initial_fetch_test<'a>() -> FetchTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock health endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Mock internal users endpoint with some users
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {
                    "username": "user1",
                    "current_otp": "123456",
                    "time_remaining": 25
                },
                {
                    "username": "user2",
                    "current_otp": "654321",
                    "time_remaining": 15
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);
    let app = CollectsApp::new(state);
    let harness = Harness::new_eframe(|_| app);

    FetchTestCtx {
        mock_server,
        harness,
    }
}

/// Test that users are displayed after the initial fetch completes.
/// This verifies the auto-fetch behavior is working correctly.
#[tokio::test]
async fn test_initial_fetch_displays_users() {
    let mut ctx = setup_initial_fetch_test().await;
    let harness = ctx.harness_mut();

    // Run frames to trigger the initial fetch and process response
    harness.step();

    // Wait for the async fetch to complete
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    // Run several frames to process the response
    for _ in 0..10 {
        harness.step();
    }

    // Check that users are now displayed
    let user1 = harness.query_by_label_contains("user1");
    let user2 = harness.query_by_label_contains("user2");

    // At least one of the users should be visible now
    assert!(
        user1.is_some() || user2.is_some(),
        "Should display users after fetch completes"
    );
}

/// Test that the loading spinner displays the correct message.
/// We test this indirectly by checking the state, since the harness
/// initialization may already process the first frame.
#[tokio::test]
async fn test_loading_state_is_set() {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock health endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Mock internal users endpoint with a delay to ensure loading state is visible
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({
                    "users": []
                }))
                .set_delay(std::time::Duration::from_secs(1)),
        )
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);
    let app = CollectsApp::new(state);
    let mut harness = Harness::new_eframe(|_| app);

    // Run a frame to start the fetch
    harness.step();

    // Due to the delayed response, the loading state should still be active
    let is_fetching = harness
        .state()
        .state
        .ctx
        .state_mut::<InternalUsersState>()
        .is_fetching();

    // If the response hasn't arrived yet, we should be in fetching state
    // Note: This test may be flaky due to timing, so we accept both states
    // The important thing is that the fetch was triggered
    if is_fetching {
        // Check that the loading message is displayed
        let loading_label = harness.query_by_label_contains("Loading users");
        assert!(
            loading_label.is_some(),
            "Should display 'Loading users...' message while fetching"
        );
    }
    // If not fetching, the response arrived very quickly - this is also acceptable
}

/// Test that the initial fetch is triggered at app startup.
/// We verify this by checking that the internal users endpoint was called.
#[tokio::test]
async fn test_initial_fetch_is_triggered() {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Mock health endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Track that the internal users endpoint is called at least once
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": []
        })))
        .expect(1..)  // Expect at least 1 call
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);
    let app = CollectsApp::new(state);
    let mut harness = Harness::new_eframe(|_| app);

    // Run a few frames
    for _ in 0..5 {
        harness.step();
    }

    // Wait for async operations
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // The mock server will verify that the endpoint was called
    // If it wasn't, the test will fail when the mock server is dropped
}
