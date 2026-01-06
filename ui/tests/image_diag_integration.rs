//! Integration tests for image diagnostics window functionality.
//!
//! These tests verify the image diagnostics window works correctly,
//! including the F2 toggle and event recording.

mod common;

use crate::common::TestCtx;
use collects_business::{
    ImageDiagState, ImageEventType, RecordImageEventCommand, ToggleImageDiagCommand,
};
use kittest::Queryable;

#[tokio::test]
async fn test_image_diag_window_hidden_by_default() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let state sync
    for _ in 0..10 {
        harness.step();
    }

    // Image Diagnostics window should not be visible by default
    assert!(
        harness
            .query_by_label_contains("Image Diagnostics")
            .is_none(),
        "Image Diagnostics window should be hidden by default"
    );
}

#[tokio::test]
async fn test_f2_key_shows_image_diag_window() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let state sync
    for _ in 0..10 {
        harness.step();
    }

    // Press F2 to toggle the diagnostics window
    harness.key_press(egui::Key::F2);
    harness.step();
    harness.step();

    // Image Diagnostics window should now be visible
    assert!(
        harness
            .query_by_label_contains("Image Event History")
            .is_some(),
        "Image Diagnostics window should be visible after F2"
    );
}

#[tokio::test]
async fn test_f2_key_toggles_image_diag_window() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let state sync
    for _ in 0..10 {
        harness.step();
    }

    // Press F2 to show
    harness.key_press(egui::Key::F2);
    harness.step();
    harness.step();

    assert!(
        harness
            .query_by_label_contains("Image Event History")
            .is_some(),
        "Window should be visible after first F2"
    );

    // Press F2 again to hide
    harness.key_press(egui::Key::F2);
    harness.step();
    harness.step();

    assert!(
        harness
            .query_by_label_contains("Image Event History")
            .is_none(),
        "Window should be hidden after second F2"
    );
}

#[tokio::test]
async fn test_image_diag_shows_no_events_message() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let state sync
    for _ in 0..10 {
        harness.step();
    }

    // Press F2 to show the diagnostics window
    harness.key_press(egui::Key::F2);
    harness.step();
    harness.step();

    // Should show "No events recorded" message
    assert!(
        harness
            .query_by_label_contains("No events recorded")
            .is_some(),
        "Should show 'No events recorded' when no events have occurred"
    );
}

#[tokio::test]
async fn test_toggle_command_works_directly() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let state sync
    for _ in 0..10 {
        harness.step();
    }

    // Initially hidden
    let initial_show = harness
        .state()
        .state
        .ctx
        .cached::<ImageDiagState>()
        .map(|d| d.show_window())
        .unwrap_or(false);
    assert!(!initial_show, "Should be hidden initially");

    // Dispatch toggle command
    harness
        .state_mut()
        .state
        .ctx
        .dispatch::<ToggleImageDiagCommand>();
    harness.state_mut().state.ctx.sync_computes();
    harness.step();

    // Should now be visible
    let after_toggle = harness
        .state()
        .state
        .ctx
        .cached::<ImageDiagState>()
        .map(|d| d.show_window())
        .unwrap_or(false);
    assert!(after_toggle, "Should be visible after toggle");
}

#[tokio::test]
async fn test_record_image_event_command_adds_event() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let state sync
    for _ in 0..10 {
        harness.step();
    }

    // Initially no events
    let initial_events = harness
        .state()
        .state
        .ctx
        .cached::<ImageDiagState>()
        .map(|d| d.events().len())
        .unwrap_or(0);
    assert_eq!(initial_events, 0, "Should have no events initially");

    // Record an event
    harness
        .state_mut()
        .state
        .ctx
        .record_command(RecordImageEventCommand {
            event_type: ImageEventType::Paste,
            width: 100,
            height: 100,
            bytes: 40000,
        });
    harness
        .state_mut()
        .state
        .ctx
        .dispatch::<RecordImageEventCommand>();
    harness.state_mut().state.ctx.sync_computes();
    harness.step();

    // Should have one event
    let after_record = harness
        .state()
        .state
        .ctx
        .cached::<ImageDiagState>()
        .map(|d| d.events().len())
        .unwrap_or(0);
    assert_eq!(after_record, 1, "Should have one event after recording");
}
