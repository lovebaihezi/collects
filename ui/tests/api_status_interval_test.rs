//! Integration tests for API status interval and retry behavior.
//!
//! These tests verify:
//! 1. API status should only be checked every 5 minutes
//! 2. On failure, max 3 retries before waiting for the full interval
//! 3. Time mocking works correctly for testing time-dependent behavior

use chrono::{Duration, Utc};
use collects_business::{APIAvailability, ApiStatus};
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

/// Helper to run compute cycle and wait for async response.
async fn run_compute_cycle(harness: &mut Harness<'_, State>) {
    harness.state_mut().ctx.run_all_dirty();
    // Wait for async HTTP response
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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

    // Run compute cycle - should trigger initial fetch
    run_compute_cycle(harness).await;

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
    run_compute_cycle(harness).await;
    let initial_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(initial_count, 1, "Should have one initial request");

    // Advance time by 4 minutes (less than 5 minute interval)
    advance_time_by_minutes(ctx.harness_mut(), 4);

    // Run compute cycle again
    run_compute_cycle(ctx.harness_mut()).await;

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
    run_compute_cycle(harness).await;
    let initial_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(initial_count, 1, "Should have one initial request");

    // Advance time by exactly 5 minutes
    advance_time_by_minutes(ctx.harness_mut(), 5);

    // Run compute cycle again
    run_compute_cycle(ctx.harness_mut()).await;

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
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // First 5-minute interval
    advance_time_by_minutes(ctx.harness_mut(), 5);
    run_compute_cycle(ctx.harness_mut()).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 2);

    // Second 5-minute interval
    advance_time_by_minutes(ctx.harness_mut(), 5);
    run_compute_cycle(ctx.harness_mut()).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 3);

    // Third 5-minute interval
    advance_time_by_minutes(ctx.harness_mut(), 5);
    run_compute_cycle(ctx.harness_mut()).await;
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
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Advance time by 4 minutes and 59 seconds (just under 5 minutes)
    advance_time_by_seconds(ctx.harness_mut(), 4 * 60 + 59);

    // Run compute cycle
    run_compute_cycle(ctx.harness_mut()).await;

    // Should NOT have made another request (still under 5 minutes)
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Should NOT refetch at 4:59 (still 1 request)"
    );

    // Advance by 1 more second to hit 5 minutes
    advance_time_by_seconds(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;

    // Now should have fetched
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        2,
        "Should refetch at exactly 5:00 (2 requests)"
    );
}

// =============================================================================
// RETRY LOGIC TESTS (MAX 3 RETRIES ON FAILURE)
// =============================================================================

#[tokio::test]
async fn test_api_status_retry_on_failure() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    // Initial fetch (will fail with 500)
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);
    assert!(
        has_api_error(ctx.harness_mut()),
        "Should have error status after 500"
    );

    // Advance time by 1 minute (within retry window, before 5 min interval)
    advance_time_by_minutes(ctx.harness_mut(), 1);

    // Run compute cycle - should retry because we have an error and retry_count < 3
    run_compute_cycle(ctx.harness_mut()).await;

    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        2,
        "Should retry after failure (2 requests - initial + 1 retry)"
    );
}

#[tokio::test]
async fn test_api_status_max_3_retries() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    // Initial fetch (will fail) - retry_count becomes 1
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Retry 1 (retry_count becomes 2)
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 2);

    // Retry 2 (retry_count becomes 3)
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 3);

    // Retry 3 - this should be the last retry (retry_count becomes 4, exceeds MAX_RETRY_COUNT=3)
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;
    // After 3 retries, we may or may not get a 4th request depending on exact logic
    let _count_after_retries = count_health_requests(ctx.mock_server()).await;

    // Now advance time but NOT to 5 minutes - should NOT retry anymore
    advance_time_by_minutes(ctx.harness_mut(), 1); // Total: 4 minutes from initial
    run_compute_cycle(ctx.harness_mut()).await;

    let count_before_5min = count_health_requests(ctx.mock_server()).await;

    // The count should not have increased significantly after max retries
    // (exact count depends on implementation, but it should stop retrying)
    assert!(
        count_before_5min <= 4,
        "Should stop retrying after max 3 retries (got {} requests)",
        count_before_5min
    );
}

/// Setup test state with a mock server that can change responses.
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
    // Start with dynamic mock server (no initial health mock)
    let mut ctx = setup_api_status_test_dynamic(|_ui, _state| {}).await;

    // Mount failing mock first
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(2) // Only fail first 2 requests
        .mount(ctx.mock_server())
        .await;

    // Mount success mock (will be used after first 2 fail)
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(ctx.mock_server())
        .await;

    let harness = ctx.harness_mut();

    // Initial fetch (fails)
    run_compute_cycle(harness).await;
    assert!(
        has_api_error(ctx.harness_mut()),
        "First request should fail"
    );

    // One retry (still fails due to up_to_n_times(2))
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;
    assert!(
        has_api_error(ctx.harness_mut()),
        "Second request should still fail"
    );

    // Another retry - should succeed now (3rd request hits success mock)
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;

    // Should now be successful
    assert_eq!(
        get_api_availability(ctx.harness_mut()),
        Some(true),
        "Should have successful status after recovery"
    );
}

#[tokio::test]
async fn test_api_status_waits_full_interval_after_max_retries() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    // Initial fetch (retry_count becomes 1 after this fails)
    run_compute_cycle(harness).await;
    let count_initial = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        count_initial, 1,
        "Should have 1 request after initial fetch"
    );

    // Retry 1 (retry_count becomes 2)
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;

    // Retry 2 (retry_count becomes 3)
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;

    // Retry 3 (retry_count becomes 4, now >= MAX_RETRY_COUNT=3, so no more retries)
    // Note: last_update_time is now set to minute 3
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;

    let count_after_max_retries = count_health_requests(ctx.mock_server()).await;

    // At minute 3, retries should be exhausted
    // Advance 1 more minute to minute 4 - should NOT fetch
    // (only 1 minute since last_update_time at minute 3)
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;

    let count_at_4min = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        count_at_4min, count_after_max_retries,
        "Should NOT fetch at 4 min after exhausting retries (expected {}, got {})",
        count_after_max_retries, count_at_4min
    );

    // Advance 4 more minutes to minute 8 (5 minutes since last_update_time at minute 3)
    // Now should fetch again because 5 min interval from last retry has passed
    advance_time_by_minutes(ctx.harness_mut(), 4);
    run_compute_cycle(ctx.harness_mut()).await;

    let count_after_interval = count_health_requests(ctx.mock_server()).await;
    assert!(
        count_after_interval > count_at_4min,
        "Should fetch after 5 min interval from last retry (expected > {}, got {})",
        count_at_4min,
        count_after_interval
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
    let initial_time = *harness.state().ctx.state_mut::<Time>().as_ref();

    // Advance time
    advance_time_by_minutes(harness, 10);

    // Verify time advanced
    let new_time = *harness.state().ctx.state_mut::<Time>().as_ref();
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

    // Set a specific time
    let specific_time = Utc::now() + Duration::hours(24);
    harness.state_mut().ctx.update::<Time>(|t| {
        *t.as_mut() = specific_time;
    });

    // Verify time was set
    let current_time = *harness.state().ctx.state_mut::<Time>().as_ref();
    assert_eq!(
        current_time, specific_time,
        "Time should be set to specific value"
    );
}

// =============================================================================
// EDGE CASES
// =============================================================================

#[tokio::test]
async fn test_api_status_success_does_not_trigger_retry() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch (success)
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Advance time by 1 minute
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;

    // Should NOT retry on success - only fetch every 5 minutes
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Should NOT retry when status is successful"
    );
}

#[tokio::test]
async fn test_api_status_404_triggers_retry() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 404).await;
    let harness = ctx.harness_mut();

    // Initial fetch (404 error)
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);
    assert!(has_api_error(ctx.harness_mut()), "404 should be an error");

    // Advance time and retry
    advance_time_by_minutes(ctx.harness_mut(), 1);
    run_compute_cycle(ctx.harness_mut()).await;

    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        2,
        "Should retry on 404 error"
    );
}

#[tokio::test]
async fn test_multiple_compute_cycles_same_minute_no_duplicate_fetch() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Run multiple compute cycles without advancing time
    for _ in 0..5 {
        run_compute_cycle(ctx.harness_mut()).await;
    }

    // Should still be only 1 request (no duplicate fetches)
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Multiple compute cycles within same interval should not trigger duplicate fetches"
    );
}

// =============================================================================
// IN-FLIGHT REQUEST TESTS (is_fetching flag behavior)
// =============================================================================

/// Helper to run compute WITHOUT waiting for async response.
/// This simulates the scenario where compute runs again before the first fetch completes.
fn run_compute_only(harness: &mut Harness<'_, State>) {
    harness.state_mut().ctx.run_all_dirty();
    // Note: NO sleep or sync_computes - this simulates in-flight state
}

#[tokio::test]
async fn test_no_duplicate_requests_during_in_flight_fetch() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Start initial fetch WITHOUT waiting for response
    run_compute_only(harness);

    // Immediately trigger another compute cycle (simulating Time update every second)
    // This should NOT trigger another request because is_fetching is true
    run_compute_only(harness);
    run_compute_only(harness);
    run_compute_only(harness);

    // Wait for async response
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    harness.state_mut().ctx.sync_computes();

    // Should only have 1 request despite multiple compute cycles
    let request_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        request_count, 1,
        "Should only have 1 request even with multiple compute cycles during in-flight fetch"
    );
}

#[tokio::test]
async fn test_is_fetching_resets_after_successful_response() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;
    let harness = ctx.harness_mut();

    // Initial fetch
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);

    // Verify status is now available (is_fetching should be false)
    assert_eq!(
        get_api_availability(ctx.harness_mut()),
        Some(true),
        "API should be available after successful fetch"
    );

    // Advance time by 5 minutes
    advance_time_by_minutes(ctx.harness_mut(), 5);

    // Should be able to fetch again (is_fetching was reset to false)
    run_compute_cycle(ctx.harness_mut()).await;

    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        2,
        "Should fetch again after 5 minutes (is_fetching was properly reset)"
    );
}

#[tokio::test]
async fn test_is_fetching_resets_after_failed_response() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 500).await;
    let harness = ctx.harness_mut();

    // Initial fetch (will fail)
    run_compute_cycle(harness).await;
    assert_eq!(count_health_requests(ctx.mock_server()).await, 1);
    assert!(
        has_api_error(ctx.harness_mut()),
        "Should have error after 500"
    );

    // Advance time by 1 minute
    advance_time_by_minutes(ctx.harness_mut(), 1);

    // Should retry (is_fetching was reset to false on error)
    run_compute_cycle(ctx.harness_mut()).await;

    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        2,
        "Should retry after error (is_fetching was properly reset)"
    );
}

#[tokio::test]
async fn test_rapid_time_updates_no_duplicate_fetch_with_sync() {
    let mut ctx = setup_api_status_test(|_ui, _state| {}, 200).await;

    // Initial fetch
    run_compute_cycle(ctx.harness_mut()).await;

    // Verify initial fetch completed
    assert_eq!(
        count_health_requests(ctx.mock_server()).await,
        1,
        "Should have 1 initial request"
    );

    // Simulate rapid Time updates like in the actual app (every second)
    // With sync calls between, this should NOT trigger additional requests
    // because the is_fetching flag will be properly synced
    for _ in 0..10 {
        advance_time_by_seconds(ctx.harness_mut(), 1);
        ctx.harness_mut().state_mut().ctx.run_all_dirty();
        ctx.harness_mut().state_mut().ctx.sync_computes();
    }

    // Should still be only 1 request despite 10 time updates
    // (not enough time passed for another fetch)
    let request_count = count_health_requests(ctx.mock_server()).await;
    assert_eq!(
        request_count, 1,
        "Rapid time updates (within 5 min interval) should not trigger additional requests"
    );

    // Verify the fetch completed successfully
    assert_eq!(
        get_api_availability(ctx.harness_mut()),
        Some(true),
        "API should be available after fetch completes"
    );
}
