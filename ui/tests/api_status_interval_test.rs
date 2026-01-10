//! Integration tests for API status interval and retry behavior.
//!
//! These tests verify:
//! 1. API status should only be checked every 5 minutes
//! 2. On failure, max 3 retries before waiting for the full interval
//! 3. Time mocking works correctly for testing time-dependent behavior
//!
//! ## Architecture
//!
//! Following the state-model.md guidelines:
//! - `ApiStatus` is a **pure cache** (Compute with no-op compute())
//! - `FetchApiStatusCommand` performs the network IO and updates the cache via `Updater`
//! - Tests dispatch the command to trigger fetches

mod common;
use common::yield_wait_for_network;

use chrono::{Duration, Utc};
use collects_business::{APIAvailability, ApiStatus, FetchApiStatusCommand};
use collects_states::Time;
use collects_ui::state::State;
use egui_kittest::Harness;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Test context that exposes the mock server for request verification.
struct ApiStatusTestCtx<'a> {
    mock_server: MockServer,
    harness: Harness<'a, State>,
}

impl<'a> ApiStatusTestCtx<'a> {
    fn harness_mut(&mut self) -> &mut Harness<'a, State> {
        &mut self.harness
    }

    fn mock_server(&self) -> &MockServer {
        &self.mock_server
    }
}

/// Setup test state with a configurable mock server.
async fn setup_api_status_test<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
    status_code: u16,
) -> ApiStatusTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(
            ResponseTemplate::new(status_code).insert_header("x-service-version", "0.1.0+test"),
        )
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

    ApiStatusTestCtx {
        mock_server,
        harness,
    }
}

/// Helper to dispatch FetchApiStatusCommand and wait for async response.
async fn dispatch_fetch_command(harness: &mut Harness<'_, State>) {
    harness
        .state_mut()
        .ctx
        .enqueue_command::<FetchApiStatusCommand>();
    harness.state_mut().ctx.flush_commands();
    // Wait for async HTTP response
    yield_wait_for_network(50).await;
    harness.state_mut().ctx.sync_computes();
}

/// Helper to run compute cycle using the compute-enqueues-command pattern.
/// This simulates the actual app flow where:
/// 1. Compute runs and may enqueue a command via Updater
/// 2. sync_computes() processes the enqueued command request
/// 3. flush_commands() executes the command
/// 4. Wait for async response
/// 5. sync_computes() applies the result
async fn run_compute_cycle(harness: &mut Harness<'_, State>) {
    // Run dirty computes - ApiStatus::compute() will call updater.enqueue_command()
    harness.state_mut().ctx.run_all_dirty();
    // Process enqueued commands from computes
    harness.state_mut().ctx.sync_computes();
    // Execute the commands
    harness.state_mut().ctx.flush_commands();
    // Wait for async HTTP response
    yield_wait_for_network(50).await;
    // Apply results
    harness.state_mut().ctx.sync_computes();
}

/// Helper to advance time by specified minutes using the Time state.
fn advance_time_by_minutes(harness: &mut Harness<'_, State>, minutes: i64) {
    harness.state_mut().ctx.update::<Time>(|t| {
        *t.as_mut() = *t.as_ref() + Duration::minutes(minutes);
    });
}

/// Helper to advance time by specified seconds using the Time state.
fn advance_time_by_seconds(harness: &mut Harness<'_, State>, seconds: i64) {
    harness.state_mut().ctx.update::<Time>(|t| {
        *t.as_mut() = *t.as_ref() + Duration::seconds(seconds);
    });
}

/// Helper to get the current API status availability.
/// Returns Some(true) if available, Some(false) if unavailable, None if unknown.
fn get_api_availability(harness: &Harness<'_, State>) -> Option<bool> {
    harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .and_then(|status| match status.api_availability() {
            APIAvailability::Available { .. } => Some(true),
            APIAvailability::Unavailable { .. } => Some(false),
            APIAvailability::Unknown => None,
        })
}

/// Helper to check if API status is in Unknown state (not yet fetched).
fn is_api_status_unknown(harness: &Harness<'_, State>) -> bool {
    harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|status| matches!(status.api_availability(), APIAvailability::Unknown))
        .unwrap_or(true)
}

/// Helper to check if API status has an error.
fn has_api_error(harness: &Harness<'_, State>) -> bool {
    harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|status| {
            matches!(
                status.api_availability(),
                APIAvailability::Unavailable { .. }
            )
        })
        .unwrap_or(false)
}

/// Helper to check if should_fetch returns true for current state.
fn should_fetch(harness: &Harness<'_, State>) -> bool {
    let now = harness.state().ctx.state::<Time>().as_ref().to_utc();
    harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|status| status.should_fetch(now))
        .unwrap_or(true)
}

/// Count requests received by the mock server for the health endpoint.
async fn count_health_requests(mock_server: &MockServer) -> usize {
    mock_server
        .received_requests()
        .await
        .map(|requests| {
            requests
                .iter()
                .filter(|r| r.url.path() == "/api/is-health")
                .count()
        })
        .unwrap_or(0)
}

// =============================================================================
// 5-MINUTE INTERVAL TESTS
// =============================================================================

#[tokio::test]
async fn test_api_status_initial_fetch() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial state: API status is Unknown (not yet fetched)
    assert!(
        is_api_status_unknown(harness),
        "Should have Unknown API status initially"
    );

    // should_fetch should return true since we haven't fetched yet
    assert!(
        should_fetch(harness),
        "should_fetch should be true initially"
    );

    // Dispatch fetch command - should trigger initial fetch
    dispatch_fetch_command(harness).await;

    // Should have made exactly one request
    let request_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        request_count, 1,
        "Should have made exactly one initial request"
    );

    // Should now have a successful status
    assert_eq!(
        get_api_availability(ctx.harness_mut()),
        Some(true),
        "Should have successful API status after initial fetch"
    );
}

#[tokio::test]
async fn test_api_status_no_refetch_before_5_minutes() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch
    dispatch_fetch_command(harness).await;
    let initial_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(initial_count, 1, "Should have one initial request");

    // Advance time by 4 minutes (less than 5 minute interval)
    advance_time_by_minutes(ctx.harness_mut(), 4);

    // should_fetch should return false (not enough time passed)
    assert!(
        !should_fetch(ctx.harness_mut()),
        "should_fetch should be false before 5 minutes"
    );

    // Dispatch fetch command again - should NOT make a request due to internal check
    dispatch_fetch_command(ctx.harness_mut()).await;

    // Should NOT have made another request
    let count_after_4min = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        count_after_4min, 1,
        "Should NOT refetch before 5 minutes (still 1 request after 4 min)"
    );
}

#[tokio::test]
async fn test_api_status_refetches_after_5_minutes() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch
    dispatch_fetch_command(harness).await;
    let initial_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(initial_count, 1, "Should have one initial request");

    // Advance time by exactly 5 minutes
    advance_time_by_minutes(ctx.harness_mut(), 5);

    // should_fetch should return true
    assert!(
        should_fetch(ctx.harness_mut()),
        "should_fetch should be true after 5 minutes"
    );

    // Dispatch fetch command again
    dispatch_fetch_command(ctx.harness_mut()).await;

    // Should have made another request
    let count_after_5min = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        count_after_5min, 2,
        "Should refetch after 5 minutes (2 requests total)"
    );
}

#[tokio::test]
async fn test_api_status_multiple_intervals() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch
    dispatch_fetch_command(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // First 5-minute interval
    advance_time_by_minutes(ctx.harness_mut(), 5);
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 2);

    // Second 5-minute interval
    advance_time_by_minutes(ctx.harness_mut(), 5);
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 3);

    // Third 5-minute interval
    advance_time_by_minutes(ctx.harness_mut(), 5);
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        4,
        "Should have 4 requests after 3 intervals (initial + 3)"
    );
}

#[tokio::test]
async fn test_api_status_no_refetch_at_4_minutes_59_seconds() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch
    dispatch_fetch_command(harness).await;
    let initial_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(initial_count, 1, "Should have one initial request");

    // Advance time by 4 minutes 59 seconds (just under 5 minute threshold)
    advance_time_by_minutes(ctx.harness_mut(), 4);
    advance_time_by_seconds(ctx.harness_mut(), 59);

    // should_fetch should return false
    assert!(
        !should_fetch(ctx.harness_mut()),
        "should_fetch should be false at 4:59"
    );

    // Dispatch fetch command again
    dispatch_fetch_command(ctx.harness_mut()).await;

    // Should NOT have made another request (4:59 < 5:00)
    let count_after = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        count_after, 1,
        "Should NOT refetch at 4:59 (still 1 request)"
    );
}

// =============================================================================
// RETRY TESTS
// =============================================================================

#[tokio::test]
async fn test_api_status_retry_on_failure() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    // Initial fetch (will fail with 500)
    dispatch_fetch_command(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);
    assert!(has_api_error(ctx.harness_mut()), "Should have error status");

    // should_fetch should return true for retry
    assert!(
        should_fetch(ctx.harness_mut()),
        "should_fetch should be true for retry after error"
    );

    // Second attempt (retry)
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        2,
        "Should retry immediately after failure"
    );
}

#[tokio::test]
async fn test_api_status_max_3_retries() {
    // MAX_RETRY_COUNT = 3 means:
    // - Initial request fails: retry_count becomes 1
    // - Retry 1 fails: retry_count becomes 2
    // - Retry 2 fails: retry_count becomes 3
    // - Now retry_count (3) >= MAX_RETRY_COUNT (3), so should_fetch returns false
    // Total = 3 requests (initial + 2 retries)
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    // Initial fetch (will fail with 500), retry_count becomes 1
    dispatch_fetch_command(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Retry 1, retry_count becomes 2
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 2);

    // Retry 2, retry_count becomes 3 (now at MAX_RETRY_COUNT)
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 3);

    // After hitting max retries (retry_count = 3), should_fetch should return false
    // until the full interval passes
    assert!(
        !should_fetch(ctx.harness_mut()),
        "should_fetch should be false after max retries"
    );

    // Another attempt - should NOT make a request (max retries reached)
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        3,
        "Should stop retrying after max retries (total 3 requests: initial + 2 retries)"
    );
}

/// Setup test state with a dynamic mock server that can change responses.
async fn setup_api_status_test_dynamic<'a>(
    app: impl FnMut(&mut egui::Ui, &mut State) + 'a,
) -> ApiStatusTestCtx<'a> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

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

    ApiStatusTestCtx {
        mock_server,
        harness,
    }
}

#[tokio::test]
async fn test_api_status_retry_resets_on_success() {
    let mut ctx = setup_api_status_test_dynamic(|_ui, _state| {}).await;

    // First, mount a failing mock
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(500))
        .expect(1)
        .mount(ctx.mock_server())
        .await;

    // Initial fetch (will fail)
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert!(
        has_api_error(ctx.harness_mut()),
        "First request should fail"
    );
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Clear mocks and mount a successful one
    ctx.mock_server.reset().await;
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": []
        })))
        .mount(ctx.mock_server())
        .await;
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(ctx.mock_server())
        .await;

    // Retry (should succeed now)
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        get_api_availability(ctx.harness_mut()),
        Some(true),
        "Retry should succeed"
    );

    // After success, should_fetch should be false until interval passes
    assert!(
        !should_fetch(ctx.harness_mut()),
        "should_fetch should be false after success"
    );
}

#[tokio::test]
async fn test_api_status_waits_full_interval_after_max_retries() {
    // MAX_RETRY_COUNT = 3 means initial + 2 retries = 3 total requests
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    // Initial fetch + 2 retries (3 total requests, all fail)
    dispatch_fetch_command(harness).await;
    dispatch_fetch_command(ctx.harness_mut()).await;
    dispatch_fetch_command(ctx.harness_mut()).await;

    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        3,
        "Should have 3 requests (initial + 2 retries)"
    );

    // After max retries, should_fetch should be false
    assert!(
        !should_fetch(ctx.harness_mut()),
        "should_fetch should be false after max retries"
    );

    // Advance time by 2 minutes - should NOT trigger fetch
    advance_time_by_minutes(ctx.harness_mut(), 2);
    assert!(
        !should_fetch(ctx.harness_mut()),
        "should_fetch should still be false after 2 minutes"
    );
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        3,
        "Should NOT refetch after 2 minutes when max retries exhausted"
    );

    // Advance time to 5 minutes total - should NOW trigger fetch
    advance_time_by_minutes(ctx.harness_mut(), 3);
    assert!(
        should_fetch(ctx.harness_mut()),
        "should_fetch should be true after 5 minutes"
    );
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        4,
        "Should refetch after 5 minutes even when max retries were exhausted"
    );
}

// =============================================================================
// TIME MOCKING TESTS
// =============================================================================

#[tokio::test]
async fn test_time_mocking_works() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Get initial time
    let initial_time = harness.state().ctx.state::<Time>().as_ref().to_utc();

    // Advance by 10 minutes
    advance_time_by_minutes(harness, 10);

    // Verify time advanced
    let new_time = harness.state().ctx.state::<Time>().as_ref().to_utc();
    let diff = new_time.signed_duration_since(initial_time);

    assert_eq!(
        diff.num_minutes(),
        10,
        "Time should have advanced by 10 minutes"
    );
}

#[tokio::test]
async fn test_time_can_be_set_to_specific_value() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Set to a specific time
    let specific_time = Utc::now() + Duration::hours(24);
    harness.state_mut().ctx.update::<Time>(|t| {
        *t.as_mut() = specific_time;
    });

    // Verify time was set
    let current_time = harness.state().ctx.state::<Time>().as_ref().to_utc();
    assert_eq!(
        current_time, specific_time,
        "Time should be set to specific value"
    );
}

// =============================================================================
// EDGE CASE TESTS
// =============================================================================

#[tokio::test]
async fn test_api_status_success_does_not_trigger_retry() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch (success)
    dispatch_fetch_command(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);
    assert_eq!(get_api_availability(ctx.harness_mut()), Some(true));

    // should_fetch should be false (success, no retry needed)
    assert!(
        !should_fetch(ctx.harness_mut()),
        "should_fetch should be false after success"
    );

    // Dispatch again - should NOT make a request
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Should NOT retry after success"
    );
}

#[tokio::test]
async fn test_api_status_404_triggers_retry() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 404).await;
    let harness = ctx.harness_mut();

    // Initial fetch (404 error)
    dispatch_fetch_command(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);
    assert!(has_api_error(ctx.harness_mut()), "Should have error status");

    // should_fetch should be true for retry
    assert!(
        should_fetch(ctx.harness_mut()),
        "should_fetch should be true for retry after 404"
    );

    // Retry
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        2,
        "Should retry after 404"
    );
}

#[tokio::test]
async fn test_multiple_command_dispatches_same_minute_no_duplicate_fetch() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch
    dispatch_fetch_command(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Multiple dispatches within the same minute - should NOT make additional requests
    dispatch_fetch_command(ctx.harness_mut()).await;
    dispatch_fetch_command(ctx.harness_mut()).await;
    dispatch_fetch_command(ctx.harness_mut()).await;

    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Multiple dispatches should not cause duplicate fetches"
    );
}

// =============================================================================
// IS_FETCHING FLAG TESTS
// =============================================================================

/// Helper to check if is_fetching is true.
fn is_fetching(harness: &Harness<'_, State>) -> bool {
    harness
        .state()
        .ctx
        .cached::<ApiStatus>()
        .map(|status| status.is_fetching())
        .unwrap_or(false)
}

#[tokio::test]
async fn test_no_duplicate_requests_during_in_flight_fetch() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial state - not fetching
    assert!(!is_fetching(harness), "Should not be fetching initially");

    // Dispatch fetch command
    harness
        .state_mut()
        .ctx
        .enqueue_command::<FetchApiStatusCommand>();
    harness.state_mut().ctx.flush_commands();

    // After command runs but before network response, should be fetching
    // (We need a small delay for the command to set is_fetching = true)
    tokio::time::sleep(tokio::time::Duration::from_millis(5)).await;
    harness.state_mut().ctx.sync_computes();

    // Wait for network response
    yield_wait_for_network(50).await;
    harness.state_mut().ctx.sync_computes();

    // Should have made exactly 1 request
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Should only have 1 request even with multiple compute cycles during in-flight fetch"
    );
}

#[tokio::test]
async fn test_is_fetching_resets_after_successful_response() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Dispatch fetch command and wait for completion
    dispatch_fetch_command(harness).await;

    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // is_fetching should be false after successful response
    assert!(
        !is_fetching(ctx.harness_mut()),
        "is_fetching should be false after successful response"
    );

    // Status should be available
    assert_eq!(
        get_api_availability(ctx.harness_mut()),
        Some(true),
        "API should be available"
    );
}

#[tokio::test]
async fn test_is_fetching_resets_after_failed_response() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    // Dispatch fetch command and wait for completion
    dispatch_fetch_command(harness).await;

    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // is_fetching should be false after failed response
    assert!(
        !is_fetching(ctx.harness_mut()),
        "is_fetching should be false after failed response"
    );

    // Status should show error
    assert!(has_api_error(ctx.harness_mut()), "API should have error");
}

#[tokio::test]
async fn test_rapid_time_updates_no_duplicate_fetch_with_sync() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;

    // Initial fetch
    dispatch_fetch_command(ctx.harness_mut()).await;
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Should have 1 initial request"
    );

    // Simulate rapid time updates (like in the app's update loop)
    // Each second update should NOT trigger a fetch since interval hasn't passed
    for _ in 0..10 {
        advance_time_by_seconds(ctx.harness_mut(), 1);
        ctx.harness_mut().state_mut().ctx.sync_computes();

        // Only dispatch if should_fetch returns true (simulating app behavior)
        if should_fetch(ctx.harness_mut()) {
            dispatch_fetch_command(ctx.harness_mut()).await;
        }
    }

    // Should still have only 1 request
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Rapid time updates should not cause duplicate fetches"
    );
}

// =============================================================================
// SHOULD_FETCH LOGIC TESTS
// =============================================================================

#[tokio::test]
async fn test_should_fetch_true_on_initial_state() {
    let ctx = setup_api_status_test(|_ui, _state| {}, 200).await;

    // Before any fetch, should_fetch should return true
    assert!(
        should_fetch(&ctx.harness),
        "should_fetch should be true on initial state"
    );
}

#[tokio::test]
async fn test_should_fetch_false_immediately_after_success() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    dispatch_fetch_command(harness).await;

    assert!(
        !should_fetch(ctx.harness_mut()),
        "should_fetch should be false immediately after success"
    );
}

#[tokio::test]
async fn test_should_fetch_true_after_failure_for_retry() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    dispatch_fetch_command(harness).await;

    // After failure, should_fetch should return true for retry (if under max retries)
    assert!(
        should_fetch(ctx.harness_mut()),
        "should_fetch should be true after failure for retry"
    );
}

// =============================================================================
// COMPUTE-ENQUEUES-COMMAND PATTERN TESTS
// =============================================================================

/// Tests that the compute properly enqueues the fetch command when should_fetch is true.
/// This verifies the Option 1 architecture: Compute enqueues Command via Updater.
#[tokio::test]
async fn test_compute_enqueues_command_on_initial_fetch() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial state: API status is Unknown (not yet fetched)
    assert!(
        is_api_status_unknown(harness),
        "Should have Unknown API status initially"
    );

    // Run compute cycle using the compute-enqueues-command pattern
    run_compute_cycle(harness).await;

    // Should have made exactly one request
    let request_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        request_count, 1,
        "Compute should have enqueued command which made one request"
    );

    // Should now have a successful status
    assert_eq!(
        get_api_availability(ctx.harness_mut()),
        Some(true),
        "Should have successful API status after compute-triggered fetch"
    );
}

/// Tests that the compute does not enqueue commands when should_fetch is false.
#[tokio::test]
async fn test_compute_does_not_enqueue_when_recently_fetched() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch via compute cycle
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Advance time by 2 minutes (less than 5 minute interval)
    advance_time_by_minutes(ctx.harness_mut(), 2);

    // Run another compute cycle - should NOT trigger fetch
    run_compute_cycle(ctx.harness_mut()).await;

    // Should still have only 1 request
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Compute should not enqueue command when recently fetched"
    );
}

/// Tests that the full app flow works: Time updates trigger compute, which enqueues command.
#[tokio::test]
async fn test_full_app_flow_time_update_triggers_fetch() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Advance time by 5 minutes
    advance_time_by_minutes(ctx.harness_mut(), 5);

    // Run compute cycle - compute should detect interval passed and enqueue command
    run_compute_cycle(ctx.harness_mut()).await;

    // Should have made another request
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        2,
        "Time update should trigger compute which enqueues fetch command"
    );
}
