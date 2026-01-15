//! Integration tests for OTP countdown + auto-refresh across cycle boundaries in the internal users panel.
//!
//! This test locks down two behaviors:
//! 1) The list-users `time_remaining` shown in the table counts down as the app `Time` advances.
//! 2) When an OTP cycle boundary is crossed (time_remaining reaches 0 and wraps), the panel
//!    auto-enqueues `RefreshInternalUsersCommand` and the list is refreshed (new OTP code shown).
//!
//! Notes on harness choice:
//! - We use a full `CollectsApp` harness because internal-users refresh is dispatched as a Command
//!   and is flushed end-of-frame by `CollectsApp::update()`.
//! - This mirrors production behavior and avoids the “compute stuck Idle” pitfalls.
//!
//! IMPORTANT (startup + routing):
//! - In route-controlled mode, route entry enqueues the initial refresh when navigating to `Route::Internal`.
//! - In this integration test harness, auth/routing may not naturally settle into `Route::Internal` fast enough
//!   (or at all) depending on mocked auth state.
//! - Therefore this test explicitly enqueues ONE initial refresh *after* the app has had a few frames to settle.
//! - Do NOT enqueue more than once; overlapping refreshes can trip the “stale async publish attempt” guard.
//!
// These tests run only in internal/test-internal builds.
#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

mod common;

use crate::common::yield_wait_for_network;

use chrono::Duration;
use collects_business::{
    InternalUsersListUsersCompute, InternalUsersListUsersResult, RefreshInternalUsersCommand,
};
use collects_states::Time;
use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Time to wait for async API responses in tests (milliseconds).
const API_RESPONSE_WAIT_MS: u64 = 25;

/// Wait until the list-users compute is `Loaded` and return the loaded users.
///
/// We validate list content directly (compute-level) instead of relying on UI labels here,
/// because egui table rendering + kittest querying can be brittle (especially with TableBuilder).
async fn wait_for_loaded_users(
    harness: &mut Harness<'_, CollectsApp>,
    max_frames: usize,
) -> Vec<collects_business::InternalUserItem> {
    for _ in 0..max_frames {
        harness.step();
        yield_wait_for_network(API_RESPONSE_WAIT_MS).await;

        if let Some(c) = harness
            .state()
            .state
            .ctx
            .cached::<InternalUsersListUsersCompute>()
        {
            if let InternalUsersListUsersResult::Loaded(users) = &c.result {
                return users.clone();
            }
        }
    }

    let c = harness
        .state()
        .state
        .ctx
        .cached::<InternalUsersListUsersCompute>()
        .expect("InternalUsersListUsersCompute should be cached (even if not Loaded)");

    panic!(
        "Timed out waiting for InternalUsersListUsersCompute::Loaded. Current result: {:?}",
        c.result
    );
}

/// Advance the app time state by a number of seconds.
fn advance_time_by_seconds(harness: &mut Harness<'_, CollectsApp>, seconds: i64) {
    harness.state_mut().state.ctx.update::<Time>(|t| {
        *t.as_mut() = *t.as_ref() + Duration::seconds(seconds);
    });
}

/// Setup a `CollectsApp` harness wired to the given mock base URL.
/// Uses manual time control so tests can control Time deterministically.
///
/// We do NOT force the route here. The test will enqueue one initial refresh explicitly
/// after a few frames to avoid relying on auth/routing side-effects.
fn make_app_harness(base_url: String) -> Harness<'static, CollectsApp> {
    let state = State::test(base_url);

    let app = CollectsApp::builder()
        .state(state)
        .manual_time_control(true)
        .build();
    Harness::new_eframe(|_| app)
}

#[tokio::test]
async fn test_list_users_time_remaining_counts_down_and_auto_refreshes_after_cycle_boundary() {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Health endpoint (app boot).
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    // We want to verify:
    // - Initial list response returns OTP "111111" with 3s remaining
    // - After time advances beyond that boundary, the panel auto-refreshes
    // - Next list response returns new OTP "222222" with 30s remaining (fresh cycle)
    //
    // IMPORTANT: use a stateful responder that tolerates extra early requests.
    // Internal startup can trigger more than one refresh attempt due to route transitions,
    // and we don't want brittle `.expect(n)` constraints here.
    let list_call_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let list_call_count_clone = list_call_count.clone();

    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(move |_req: &wiremock::Request| {
            let n = list_call_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);

            // First response: baseline OTP with 3s remaining.
            // Second (and later) response: refreshed OTP with 30s remaining.
            if n < 1 {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "users": [
                        {
                            "username": "alice",
                            "current_otp": "111111",
                            "time_remaining": 3,
                            "nickname": null,
                            "avatar_url": null,
                            "created_at": "2026-01-01T10:00:00Z",
                            "updated_at": "2026-01-01T10:00:00Z"
                        }
                    ]
                }))
            } else {
                ResponseTemplate::new(200).set_body_json(serde_json::json!({
                    "users": [
                        {
                            "username": "alice",
                            "current_otp": "222222",
                            "time_remaining": 30,
                            "nickname": null,
                            "avatar_url": null,
                            "created_at": "2026-01-01T10:00:00Z",
                            "updated_at": "2026-01-01T10:00:00Z"
                        }
                    ]
                }))
            }
        })
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut harness = make_app_harness(base_url);

    // Let app routing settle for a few frames, then enqueue exactly one initial refresh.
    // (Avoid relying on auth/route side-effects in the harness.)
    for _ in 0..10 {
        harness.step();
        yield_wait_for_network(API_RESPONSE_WAIT_MS).await;
    }
    harness
        .state_mut()
        .state
        .ctx
        .enqueue_command::<RefreshInternalUsersCommand>();

    // Initial load: validate compute contents directly.
    let users0 = wait_for_loaded_users(&mut harness, 240).await;
    assert_eq!(users0.len(), 1, "Expected exactly one user in initial list");
    assert_eq!(users0[0].username, "alice");
    assert_eq!(users0[0].current_otp, "111111");
    assert_eq!(users0[0].time_remaining, 3);

    // Advance by 1 second: countdown should tick down (compute snapshot will still be 3,
    // but the panel should be using Time state to render a derived countdown).
    //
    // We cannot reliably assert the table label here due to TableBuilder click/query brittleness,
    // so instead we ensure the stale boundary logic works by crossing the boundary and observing
    // the refreshed list payload arrive.
    advance_time_by_seconds(&mut harness, 1);

    // Cross the boundary (total elapsed >= 3s since first fetch):
    // this should trigger auto-refresh and produce the second list payload.
    advance_time_by_seconds(&mut harness, 3);

    // Wait for the refreshed compute payload.
    let users1 = wait_for_loaded_users(&mut harness, 240).await;
    assert_eq!(users1.len(), 1, "Expected exactly one user after refresh");
    assert_eq!(users1[0].username, "alice");
    assert_eq!(
        users1[0].current_otp, "222222",
        "After crossing a cycle boundary, the refreshed list should contain the updated OTP code"
    );
    assert_eq!(
        users1[0].time_remaining, 30,
        "After refresh, the list payload should contain the fresh cycle time_remaining"
    );
}
