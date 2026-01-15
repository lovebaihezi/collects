//! Integration test verifying that zero `time_remaining` does NOT cause infinite refresh loops.
//!
//! This test locks down the fix for a bug where:
//! - API returns a user with `time_remaining = 0`
//! - OTP is hidden (not revealed)
//! - The stale check `elapsed >= time_remaining` would immediately be true (0 >= 0)
//! - This caused the panel to enqueue `RefreshInternalUsersCommand` on every UI frame
//!
//! The fix ensures `last_fetch` is updated on every refresh completion (not just initialization),
//! so the stale check compares against the fresh response's fetch time.
//!
//! Test strategy:
//! 1. Mock server returns user with `time_remaining = 0`
//! 2. Wait for the initial load to complete (triggered by app/route initialization)
//! 3. Step several more frames WITHOUT advancing time
//! 4. Assert that no additional refresh requests were made (not continuous re-fetching)
//!
// These tests run only in internal/test-internal builds.
#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

mod common;

use crate::common::yield_wait_for_network;

use collects_business::{
    InternalUsersListUsersCompute, InternalUsersListUsersResult, InternalUsersState,
};
use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Time to wait for async API responses in tests (milliseconds).
const API_RESPONSE_WAIT_MS: u64 = 25;

/// Wait until the list-users compute is `Loaded`.
///
/// After detecting Loaded, steps a few more frames to ensure
/// `ensure_last_fetch_initialized_for_loaded_users` has a chance to run
/// and clear `is_fetching`.
async fn wait_for_loaded(harness: &mut Harness<'_, CollectsApp>, max_frames: usize) -> bool {
    for _ in 0..max_frames {
        harness.step();
        yield_wait_for_network(API_RESPONSE_WAIT_MS).await;

        if let Some(c) = harness
            .state()
            .state
            .ctx
            .cached::<InternalUsersListUsersCompute>()
        {
            if matches!(&c.result, InternalUsersListUsersResult::Loaded(_)) {
                // Step a few more frames to let ensure_last_fetch_initialized_for_loaded_users
                // run and clear is_fetching.
                for _ in 0..5 {
                    harness.step();
                    yield_wait_for_network(5).await;
                }
                return true;
            }
        }
    }
    false
}

/// Setup a `CollectsApp` harness wired to the given mock base URL.
/// Uses manual time control so tests can control Time deterministically.
fn make_app_harness(base_url: String) -> Harness<'static, CollectsApp> {
    let state = State::test(base_url);

    let app = CollectsApp::builder()
        .state(state)
        .manual_time_control(true)
        .build();
    Harness::new_eframe(|_| app)
}

#[tokio::test]
async fn test_zero_time_remaining_does_not_cause_infinite_refresh_loop() {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Health endpoint (app boot).
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    // Track how many times the list-users endpoint is called.
    let list_call_count = Arc::new(AtomicUsize::new(0));
    let list_call_count_clone = list_call_count.clone();

    // Always return a user with time_remaining = 0 (edge case that previously caused infinite loop).
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(move |_req: &wiremock::Request| {
            list_call_count_clone.fetch_add(1, Ordering::SeqCst);

            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {
                        "username": "testuser",
                        "current_otp": "123456",
                        "time_remaining": 0,
                        "nickname": null,
                        "avatar_url": null,
                        "created_at": "2026-01-01T10:00:00Z",
                        "updated_at": "2026-01-01T10:00:00Z"
                    }
                ]
            }))
        })
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut harness = make_app_harness(base_url);

    // Wait for the initial load to complete (app/route automatically triggers refresh).
    let loaded = wait_for_loaded(&mut harness, 120).await;
    assert!(loaded, "Expected list-users compute to reach Loaded state");

    // Verify last_fetch was set (critical for the fix).
    let last_fetch = harness
        .state()
        .state
        .ctx
        .state::<InternalUsersState>()
        .last_fetch;
    assert!(
        last_fetch.is_some(),
        "last_fetch should be set after refresh completes"
    );

    // Verify is_fetching was cleared (critical for the fix).
    let is_fetching = harness
        .state()
        .state
        .ctx
        .state::<InternalUsersState>()
        .is_fetching;
    assert!(
        !is_fetching,
        "is_fetching should be false after refresh completes"
    );

    // Record how many calls have been made so far (includes initial load).
    let calls_after_initial_load = list_call_count.load(Ordering::SeqCst);
    assert!(
        calls_after_initial_load >= 1,
        "Expected at least one API call for initial load"
    );

    // Now step many more frames WITHOUT advancing time.
    // If the bug is present, each frame would trigger another refresh because
    // is_otp_stale(0, now) would always return true.
    for _ in 0..50 {
        harness.step();
        yield_wait_for_network(5).await; // Short wait since we don't expect network activity
    }

    let calls_after_extra_frames = list_call_count.load(Ordering::SeqCst);

    // The fix should prevent additional refreshes.
    // We allow a small margin for any edge-case re-triggers during the initial load stabilization,
    // but the key assertion is that we don't see 50+ additional calls (one per frame).
    let additional_calls = calls_after_extra_frames - calls_after_initial_load;
    assert!(
        additional_calls <= 1,
        "Expected at most 1 additional refresh after initial load, but got {}. \
         This suggests the infinite refresh loop bug is present. \
         Total calls: {}, after initial load: {}",
        additional_calls,
        calls_after_extra_frames,
        calls_after_initial_load
    );
}

#[tokio::test]
async fn test_low_time_remaining_with_hidden_otp_no_immediate_refresh() {
    //! Variant test: time_remaining = 1 (not zero, but very low).
    //! Ensures that even with a low time_remaining, we don't immediately re-fetch
    //! on the very next frame after data arrives.
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    let list_call_count = Arc::new(AtomicUsize::new(0));
    let list_call_count_clone = list_call_count.clone();

    // Return user with time_remaining = 1 (expires in 1 second).
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(move |_req: &wiremock::Request| {
            list_call_count_clone.fetch_add(1, Ordering::SeqCst);

            ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "users": [
                    {
                        "username": "testuser",
                        "current_otp": "654321",
                        "time_remaining": 1,
                        "nickname": null,
                        "avatar_url": null,
                        "created_at": "2026-01-01T10:00:00Z",
                        "updated_at": "2026-01-01T10:00:00Z"
                    }
                ]
            }))
        })
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut harness = make_app_harness(base_url);

    // Wait for initial load to complete.
    let loaded = wait_for_loaded(&mut harness, 120).await;
    assert!(loaded, "Expected list-users compute to reach Loaded state");

    let calls_after_load = list_call_count.load(Ordering::SeqCst);

    // Step a few more frames WITHOUT advancing time.
    // Time hasn't elapsed, so even though time_remaining is 1, we shouldn't re-fetch yet.
    for _ in 0..20 {
        harness.step();
        yield_wait_for_network(5).await;
    }

    let calls_after_extra_frames = list_call_count.load(Ordering::SeqCst);
    let additional_calls = calls_after_extra_frames - calls_after_load;

    assert!(
        additional_calls == 0,
        "Expected no additional refreshes when time hasn't advanced, but got {}",
        additional_calls
    );
}
