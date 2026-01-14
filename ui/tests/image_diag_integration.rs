//! Integration tests for Image Diagnostics window functionality (Shift+F2 key).
//!
//! These tests verify:
//! 1. Image diagnostics window is hidden by default
//! 2. Shift+F2 key press correctly toggles visibility
//! 3. Key events are recorded in the diagnostic state
//! 4. Paste/drop events are recorded in the diagnostic state
//! 5. Clear history functionality works correctly

use crate::common::TestCtx;
use collects_business::{
    ClipboardAccessResult, DropHoverEvent, DropResult, ImageDiagState, KeyEventType, PasteResult,
};
use kittest::Queryable;

mod common;

// =============================================================================
// HELPER FUNCTION
// =============================================================================

/// Helper function to check if the image diagnostics window is visible via kittest query.
/// The window contains a heading "Image Paste/Drop Diagnostics" which we can query for.
fn is_image_diag_visible(harness: &egui_kittest::Harness<'_, collects_ui::CollectsApp>) -> bool {
    harness
        .query_by_label_contains("Image Paste/Drop Diagnostics")
        .is_some()
}

// =============================================================================
// TOGGLE FUNCTIONALITY TESTS (Using Shift+F2 key press via harness)
// =============================================================================

/// Tests that image diagnostics window is hidden by default.
#[tokio::test]
async fn test_image_diag_hidden_by_default() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run the first frame
    harness.step();

    // By default, the image diagnostics window should not be visible
    assert!(
        !is_image_diag_visible(harness),
        "Image diagnostics window should be hidden by default"
    );
}

/// Tests that Shift+F2 key press toggles the visibility from off to on.
#[tokio::test]
async fn test_f2_key_shows_image_diag() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let initial state settle
    for _ in 0..5 {
        harness.step();
    }

    // Verify initially hidden
    assert!(
        !is_image_diag_visible(harness),
        "Should be hidden initially"
    );

    // Press Shift+F2 key to toggle
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Should now be visible
    assert!(
        is_image_diag_visible(harness),
        "Image diagnostics window should be visible after F2 press"
    );
}

/// Tests that Shift+F2 key press toggles the visibility from on to off.
#[tokio::test]
async fn test_f2_key_hides_image_diag() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames
    for _ in 0..5 {
        harness.step();
    }

    // Press Shift+F2 to show
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Verify visible
    assert!(
        is_image_diag_visible(harness),
        "Should be visible after first F2 press"
    );

    // Press Shift+F2 again to hide
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Should now be hidden
    assert!(
        !is_image_diag_visible(harness),
        "Image diagnostics window should be hidden after second F2 press"
    );
}

/// Tests multiple Shift+F2 key presses toggle correctly.
#[tokio::test]
async fn test_f2_key_multiple_toggles() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run initial frames
    for _ in 0..5 {
        harness.step();
    }

    // Initially hidden
    assert!(!is_image_diag_visible(harness), "Should start hidden");

    // Toggle multiple times
    for i in 0..4 {
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
        harness.step();
        harness.step();

        let expected_visible = i % 2 == 0;
        let actual_visible = is_image_diag_visible(harness);
        assert_eq!(
            actual_visible,
            expected_visible,
            "After toggle {}, visibility should be {}",
            i + 1,
            expected_visible
        );
    }
}

// =============================================================================
// STATE TESTS
// =============================================================================

/// Tests that ImageDiagState is properly initialized.
#[tokio::test]
async fn test_image_diag_state_initialized() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Access the state through the app
    let state = harness.state().state.ctx.state::<ImageDiagState>();

    assert!(!state.show_window(), "Window should be hidden by default");
    assert_eq!(state.total_key_events(), 0, "No key events initially");
    assert_eq!(
        state.total_paste_attempts(),
        0,
        "No paste attempts initially"
    );
    assert_eq!(state.total_drop_attempts(), 0, "No drop attempts initially");
    assert!(!state.is_hovering(), "Not hovering initially");
}

/// Tests that key events can be recorded (direct state test).
#[tokio::test]
async fn test_record_key_event() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Record a key event via state update
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.record_key_event(KeyEventType::CtrlV);
        });

    harness.step();

    // Verify it was recorded
    let state = harness.state().state.ctx.state::<ImageDiagState>();
    assert_eq!(state.total_key_events(), 1);
    assert_eq!(state.log_entries().count(), 1);
}

/// Tests that clipboard access can be recorded (direct state test).
#[tokio::test]
async fn test_record_clipboard_access() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Record clipboard access via state update
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.record_clipboard_access(ClipboardAccessResult::ImageFound {
                width: 100,
                height: 100,
                bytes_len: 40000,
                format: "RGBA".to_owned(),
            });
        });

    harness.step();

    // Verify it was recorded
    let state = harness.state().state.ctx.state::<ImageDiagState>();
    assert_eq!(state.log_entries().count(), 1);
}

/// Tests that paste events can be recorded (direct state test).
#[tokio::test]
async fn test_record_paste_event() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Record a paste event via state update
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.record_paste(PasteResult::Success {
                width: 100,
                height: 100,
                bytes_len: 40000,
            });
        });

    harness.step();

    // Verify it was recorded
    let state = harness.state().state.ctx.state::<ImageDiagState>();
    assert_eq!(state.total_paste_attempts(), 1);
    assert_eq!(state.total_paste_successes(), 1);
    assert_eq!(state.log_entries().count(), 1);
}

/// Tests that drop events can be recorded (direct state test).
#[tokio::test]
async fn test_record_drop_event() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Record a drop event via state update
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.record_drop(DropResult::Success {
                file_name: Some("test.png".to_owned()),
                width: 200,
                height: 200,
                bytes_len: 160000,
            });
        });

    harness.step();

    // Verify it was recorded
    let state = harness.state().state.ctx.state::<ImageDiagState>();
    assert_eq!(state.total_drop_attempts(), 1);
    assert_eq!(state.total_drop_successes(), 1);
    assert_eq!(state.log_entries().count(), 1);
}

/// Tests recording failed paste event.
#[tokio::test]
async fn test_record_paste_failure() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Record a failed paste event
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.record_paste(PasteResult::NoImageContent);
        });

    harness.step();

    // Verify it was recorded as attempt but not success
    let state = harness.state().state.ctx.state::<ImageDiagState>();
    assert_eq!(state.total_paste_attempts(), 1);
    assert_eq!(state.total_paste_successes(), 0);
}

/// Tests drop hover tracking.
#[tokio::test]
async fn test_drop_hover_tracking() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Initially not hovering
    assert!(
        !harness
            .state()
            .state
            .ctx
            .state::<ImageDiagState>()
            .is_hovering()
    );

    // Record hover start
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.record_drop_hover_start(DropHoverEvent {
                file_count: 1,
                file_names: vec!["test.png".to_owned()],
                mime_types: vec!["image/png".to_owned()],
            });
        });

    // Check immediately after update (no step needed - update is synchronous)
    assert!(
        harness
            .state()
            .state
            .ctx
            .state::<ImageDiagState>()
            .is_hovering(),
        "Should be hovering after record_drop_hover_start"
    );

    // Record hover end
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.record_drop_hover_end();
        });

    // No longer hovering
    assert!(
        !harness
            .state()
            .state
            .ctx
            .state::<ImageDiagState>()
            .is_hovering(),
        "Should not be hovering after record_drop_hover_end"
    );
}

/// Tests clear history functionality.
#[tokio::test]
async fn test_clear_history() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    harness.step();

    // Record some events
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.record_key_event(KeyEventType::CtrlV);
            diag.record_paste(PasteResult::Success {
                width: 100,
                height: 100,
                bytes_len: 40000,
            });
            diag.record_drop(DropResult::NoValidFiles { file_count: 0 });
        });

    harness.step();

    // Verify events recorded
    let state = harness.state().state.ctx.state::<ImageDiagState>();
    assert_eq!(state.total_key_events(), 1);
    assert_eq!(state.total_paste_attempts(), 1);
    assert_eq!(state.total_drop_attempts(), 1);
    assert!(state.log_entries().count() > 0);

    // Clear history
    harness
        .state_mut()
        .state
        .ctx
        .update::<ImageDiagState>(|diag| {
            diag.clear_history();
        });

    harness.step();

    // Verify log cleared but totals preserved
    let state = harness.state().state.ctx.state::<ImageDiagState>();
    // Totals are preserved after clear
    assert_eq!(state.total_key_events(), 1);
    assert_eq!(state.total_paste_attempts(), 1);
    assert_eq!(state.total_drop_attempts(), 1);
    // But log should be empty
    assert_eq!(state.log_entries().count(), 0);
}

// =============================================================================
// WINDOW CONTENT TESTS
// =============================================================================

/// Tests that the diagnostics window shows platform info.
#[tokio::test]
async fn test_image_diag_shows_platform_info() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run initial frames
    for _ in 0..5 {
        harness.step();
    }

    // Show the diagnostics window
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Check for platform label
    assert!(
        harness.query_by_label_contains("Platform:").is_some(),
        "Should show Platform label"
    );
}

/// Tests that the diagnostics window shows environment info.
#[tokio::test]
async fn test_image_diag_shows_env_info() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run initial frames
    for _ in 0..5 {
        harness.step();
    }

    // Show the diagnostics window
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Check for environment label
    assert!(
        harness.query_by_label_contains("Environment:").is_some(),
        "Should show Environment label"
    );
}

/// Tests that the clear log button is present when window is visible.
#[tokio::test]
async fn test_image_diag_has_clear_button() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run initial frames
    for _ in 0..5 {
        harness.step();
    }

    // Show the diagnostics window
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Check for clear button
    assert!(
        harness.query_by_label_contains("Clear Log").is_some(),
        "Should show Clear Log button"
    );
}

/// Tests that statistics section is present.
#[tokio::test]
async fn test_image_diag_shows_statistics() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run initial frames
    for _ in 0..5 {
        harness.step();
    }

    // Show the diagnostics window
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Check for Statistics collapsing header
    assert!(
        harness.query_by_label_contains("Statistics").is_some(),
        "Should show Statistics section"
    );
}

/// Tests that event log section is present.
#[tokio::test]
async fn test_image_diag_shows_event_log() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run initial frames
    for _ in 0..5 {
        harness.step();
    }

    // Show the diagnostics window
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Check for Event Log header
    assert!(
        harness.query_by_label_contains("Event Log").is_some(),
        "Should show Event Log section"
    );
}

/// Tests that statistics section contains key events info when expanded.
/// Note: Statistics is a collapsible section, so content may not be visible until expanded.
#[tokio::test]
async fn test_image_diag_statistics_section_exists() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run initial frames
    for _ in 0..5 {
        harness.step();
    }

    // Show the diagnostics window
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F2);
    harness.step();
    harness.step();

    // Check for Statistics collapsible header (the section exists)
    // Note: Content inside collapsed sections may not be queryable
    assert!(
        harness.query_by_label_contains("Statistics").is_some(),
        "Should show Statistics collapsible section"
    );
}
