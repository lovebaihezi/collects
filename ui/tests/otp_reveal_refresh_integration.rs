//! Integration tests for on-demand OTP reveal/refresh behavior in internal users panel.
//!
//! Why we use a full `CollectsApp` harness here:
//! - Internal-users refresh/OTP fetch are dispatched as Commands and are flushed end-of-frame.
//! - `CollectsApp::update()` performs the canonical end-of-frame flow:
//!   `sync_computes()` -> `flush_commands()` -> `sync_computes()`.
//! - This makes the test accurately reflect production behavior and avoids the “compute stuck Idle”
//!   issue that can happen when rendering the panel directly without the app loop.
//!
//! Why we still simulate “Reveal/Hide” instead of clicking the table button:
//! - egui_kittest has a known limitation: clicks inside egui_extras `TableBuilder` rows often
//!   do not reach the widget (node is found, but event doesn't propagate).
//! - This is the same approach used by `qrcode_display_integration.rs`.
//!
//! Scenario locked down (the regression you described):
//! - List-users provides a baseline OTP code + time_remaining (e.g. "111111", 18s).
//! - Revealing should fetch on-demand OTP (e.g. "222222", 7s) and the UI should prefer it
//!   while revealed across multiple frames (not flash once then revert).
//! - Hiding should return to "••••••" and stop showing the OTP code.
#![cfg(any(feature = "env_internal", feature = "env_test_internal"))]

mod common;

use crate::common::yield_wait_for_network;

use chrono::Duration;
use collects_business::{
    GetUserOtpCommand, InternalUsersActionCompute, InternalUsersActionInput,
    InternalUsersActionState, InternalUsersListUsersCompute, InternalUsersListUsersResult,
    InternalUsersState, RefreshInternalUsersCommand,
};
use collects_states::Time;
use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;
use kittest::Queryable;
use ustr::Ustr;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Time to wait for async API responses in tests (milliseconds).
const API_RESPONSE_WAIT_MS: u64 = 25;

/// Helper: run frames and give async I/O time to complete.
async fn pump_frames(harness: &mut Harness<'_, CollectsApp>, frames: usize) {
    for _ in 0..frames {
        harness.step();
        yield_wait_for_network(API_RESPONSE_WAIT_MS).await;
    }
}

/// Wait until the list-users compute reaches Loaded state.
async fn wait_for_users_loaded(harness: &mut Harness<'_, CollectsApp>) {
    // Ensure we actually trigger a refresh of the internal users table.
    //
    // In internal builds, the route should already enqueue one, but it’s cheap to be explicit here
    // and it makes the test less brittle to routing changes.
    harness
        .state_mut()
        .state
        .ctx
        .enqueue_command::<RefreshInternalUsersCommand>();

    const MAX_POLL_FRAMES: usize = 120;
    for _ in 0..MAX_POLL_FRAMES {
        harness.step();
        yield_wait_for_network(API_RESPONSE_WAIT_MS).await;

        if let Some(c) = harness
            .state()
            .state
            .ctx
            .cached::<InternalUsersListUsersCompute>()
        {
            if matches!(&c.result, InternalUsersListUsersResult::Loaded(_)) {
                return;
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
        "InternalUsersListUsersCompute did not reach Loaded after {MAX_POLL_FRAMES} frames. Current result: {:?}",
        c.result
    );
}

/// Advance the app time state by a number of seconds.
fn advance_time_by_seconds(harness: &mut Harness<'_, CollectsApp>, seconds: i64) {
    harness.state_mut().state.ctx.update::<Time>(|t| {
        *t.as_mut() = *t.as_ref() + Duration::seconds(seconds);
    });
}

/// Simulate the state transitions that a "Reveal" table button would normally trigger.
///
/// UI behavior in `panel.rs` + `row.rs`:
/// - Toggle reveal in `InternalUsersState` (sets deadline etc)
/// - Enqueue `GetUserOtpCommand` with `InternalUsersActionInput.username = <user>`
///
/// Note: We must still respect the end-of-frame command flushing model, which is why we use
/// `CollectsApp` harness. The app loop will flush the enqueued command.
fn simulate_reveal_and_fetch_otp(harness: &mut Harness<'_, CollectsApp>, username: &str) {
    let username_ustr = Ustr::from(username);

    let now = *harness.state().state.ctx.state::<Time>().as_ref();
    harness
        .state_mut()
        .state
        .ctx
        .update::<InternalUsersState>(|s| s.toggle_otp_visibility_at(username_ustr, now));

    harness
        .state_mut()
        .state
        .ctx
        .update::<InternalUsersActionInput>(|input| {
            input.username = Some(username_ustr);
            input.new_username = None;
            input.nickname = None;
            input.avatar_url = None;
        });

    harness
        .state_mut()
        .state
        .ctx
        .enqueue_command::<GetUserOtpCommand>();
}

/// Simulate Hide behavior (toggle revealed -> hidden).
fn simulate_hide(harness: &mut Harness<'_, CollectsApp>, username: &str) {
    let username_ustr = Ustr::from(username);

    let now = *harness.state().state.ctx.state::<Time>().as_ref();
    harness
        .state_mut()
        .state
        .ctx
        .update::<InternalUsersState>(|s| s.toggle_otp_visibility_at(username_ustr, now));
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
async fn test_reveal_fetches_on_demand_otp_and_persists_across_frames() {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Health
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    // Baseline list-users payload (what the table initially renders from).
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {
                    "username": "alice",
                    "current_otp": "111111",
                    "time_remaining": 18,
                    "nickname": null,
                    "avatar_url": null,
                    "created_at": "2026-01-01T10:00:00Z",
                    "updated_at": "2026-01-01T10:00:00Z"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    // On-demand get_user endpoint used by `GetUserOtpCommand`.
    Mock::given(method("GET"))
        .and(path("/api/internal/users/alice"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "alice",
            "current_otp": "222222",
            "time_remaining": 7,
            "nickname": null,
            "avatar_url": null,
            "otpauth_url": "otpauth://totp/test?secret=TEST",
            "created_at": "2026-01-01T10:00:00Z",
            "updated_at": "2026-01-01T10:00:00Z"
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut harness = make_app_harness(base_url);

    // Drive frames to let app route + initial refresh settle.
    wait_for_users_loaded(&mut harness).await;

    // Simulate Reveal + on-demand fetch.
    simulate_reveal_and_fetch_otp(&mut harness, "alice");

    // Pump frames so the command can complete and the UI can render it.
    pump_frames(&mut harness, 10).await;

    // While revealed, the UI should prefer on-demand OTP/time.
    assert!(
        harness.query_by_label_contains("222222").is_some(),
        "UI should display on-demand OTP code when revealed"
    );
    assert!(
        harness.query_by_label_contains("7s").is_some(),
        "UI should display on-demand time_remaining when revealed"
    );

    // The on-demand OTP time_remaining now counts down based on fetched_at timestamp.
    // After revealing, advancing Time should cause the displayed time to decrease.
    //
    // Drive a few more frames to catch accidental reset-to-idle behavior.
    pump_frames(&mut harness, 3).await;

    // OTP code should remain the on-demand one while revealed.
    assert!(
        harness.query_by_label_contains("222222").is_some(),
        "OTP code should remain the on-demand value while revealed (should not revert to list-users value)"
    );

    // Time remaining should still be from on-demand fetch (not reverted to list-users).
    // Note: it may have counted down slightly during pump_frames, so we check it's still around 7s or less.
    assert!(
        harness.query_by_label_contains("7s").is_some()
            || harness.query_by_label_contains("6s").is_some()
            || harness.query_by_label_contains("5s").is_some(),
        "Time remaining should be from on-demand fetch (not reverted to list-users 18s)"
    );

    // Compute should still be in Otp state (not immediately reset), otherwise you get a one-frame flash.
    let action_compute = harness
        .state()
        .state
        .ctx
        .cached::<InternalUsersActionCompute>()
        .expect("InternalUsersActionCompute should be cached after reveal");

    assert!(
        matches!(action_compute.state(), InternalUsersActionState::Otp { .. }),
        "Action compute should remain Otp across frames; if it snaps back to Idle, UI will revert to list-users OTP/time"
    );

    // Advance time and verify the countdown behavior.
    //
    // After advancing Time, the displayed time_remaining should count down accordingly.
    // This ensures the fix for live countdown is working.
    advance_time_by_seconds(&mut harness, 3);
    pump_frames(&mut harness, 5).await;

    assert!(
        harness.query_by_label_contains("222222").is_some(),
        "On-demand OTP should still be displayed across later frames while revealed"
    );

    // After 3 more seconds (total ~6s since fetch), time should have counted down from 7s to around 1-4s
    // (depending on exact timing during pump_frames)
    assert!(
        harness.query_by_label_contains("4s").is_some()
            || harness.query_by_label_contains("3s").is_some()
            || harness.query_by_label_contains("2s").is_some()
            || harness.query_by_label_contains("1s").is_some(),
        "On-demand time_remaining should count down as time advances (expected ~4s or less after 3s advance)"
    );

    // Ensure we don't see the old snapshot value
    assert!(
        harness.query_by_label_contains("7s").is_none(),
        "Should not show the original 7s snapshot after time has advanced"
    );

    // And explicitly ensure we are not showing the original list-users OTP code.
    assert!(
        harness.query_by_label_contains("111111").is_none(),
        "UI should not fall back to list-users OTP while revealed after reval"
    );
}

/// Test that on-demand OTP time_remaining counts down as time advances.
///
/// This is the expected behavior: after revealing, the displayed time should
/// count down (7s -> 6s -> 5s...) as the app's Time state advances.
///
/// Currently this test should FAIL because on-demand OTP time_remaining is
/// stored as a static snapshot and not computed relative to fetch time.
#[tokio::test]
async fn test_revealed_otp_time_counts_down_as_time_advances() {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Health
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    // List users
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {
                    "username": "alice",
                    "current_otp": "111111",
                    "time_remaining": 18,
                    "nickname": null,
                    "avatar_url": null,
                    "created_at": "2026-01-01T10:00:00Z",
                    "updated_at": "2026-01-01T10:00:00Z"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    // On-demand get_user: returns OTP with 15 seconds remaining
    Mock::given(method("GET"))
        .and(path("/api/internal/users/alice"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "alice",
            "current_otp": "222222",
            "time_remaining": 15,
            "nickname": null,
            "avatar_url": null,
            "otpauth_url": "otpauth://totp/test?secret=TEST",
            "created_at": "2026-01-01T10:00:00Z",
            "updated_at": "2026-01-01T10:00:00Z"
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut harness = make_app_harness(base_url);

    wait_for_users_loaded(&mut harness).await;

    // Reveal + fetch on-demand OTP
    simulate_reveal_and_fetch_otp(&mut harness, "alice");
    pump_frames(&mut harness, 10).await;

    // Initially should show 15s (the on-demand fetch result)
    assert!(
        harness.query_by_label_contains("15s").is_some(),
        "Initially should display on-demand time_remaining of 15s"
    );

    // Advance time by 3 seconds
    advance_time_by_seconds(&mut harness, 3);
    pump_frames(&mut harness, 3).await;

    // After 3 seconds, should show 12s (15 - 3 = 12)
    assert!(
        harness.query_by_label_contains("12s").is_some(),
        "After 3 seconds, time_remaining should count down from 15s to 12s"
    );
    assert!(
        harness.query_by_label_contains("15s").is_none(),
        "Should no longer show the old 15s value"
    );

    // Advance another 5 seconds (total 8 seconds elapsed)
    advance_time_by_seconds(&mut harness, 5);
    pump_frames(&mut harness, 3).await;

    // Should now show 7s (15 - 8 = 7)
    assert!(
        harness.query_by_label_contains("7s").is_some(),
        "After 8 seconds total, time_remaining should be 7s"
    );

    // OTP code should still be the on-demand one
    assert!(
        harness.query_by_label_contains("222222").is_some(),
        "OTP code should remain the on-demand value while counting down"
    );
}

#[tokio::test]
async fn test_hide_hides_otp_again_after_reveal() {
    let _ = env_logger::builder().is_test(true).try_init();
    let mock_server = MockServer::start().await;

    // Health
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    // List users
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {
                    "username": "alice",
                    "current_otp": "111111",
                    "time_remaining": 18,
                    "nickname": null,
                    "avatar_url": null,
                    "created_at": "2026-01-01T10:00:00Z",
                    "updated_at": "2026-01-01T10:00:00Z"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    // On-demand get_user
    Mock::given(method("GET"))
        .and(path("/api/internal/users/alice"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "username": "alice",
            "current_otp": "222222",
            "time_remaining": 7,
            "nickname": null,
            "avatar_url": null,
            "otpauth_url": "otpauth://totp/test?secret=TEST",
            "created_at": "2026-01-01T10:00:00Z",
            "updated_at": "2026-01-01T10:00:00Z"
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let mut harness = make_app_harness(base_url);

    wait_for_users_loaded(&mut harness).await;

    // Reveal + fetch
    simulate_reveal_and_fetch_otp(&mut harness, "alice");
    pump_frames(&mut harness, 10).await;

    assert!(
        harness.query_by_label_contains("222222").is_some(),
        "Sanity check: on-demand OTP displayed after reveal"
    );

    // Hide and ensure OTP is hidden again.
    simulate_hide(&mut harness, "alice");
    pump_frames(&mut harness, 5).await;

    assert!(
        harness.query_by_label("••••••").is_some(),
        "After Hide, OTP should be hidden again"
    );
    assert!(
        harness.query_by_label_contains("222222").is_none(),
        "After Hide, on-demand OTP should not be visible"
    );
}
