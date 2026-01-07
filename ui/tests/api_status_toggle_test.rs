//! Integration tests for API status toggle functionality (Shift+F1 key).
//!
//! These tests verify:
//! 1. API status panel is hidden by default
//! 2. Shift+F1 key press correctly toggles visibility via ToggleApiStatusCommand
//! 3. The show_status flag persists through API compute updates
//!
//! Note: Shift+F1 is used instead of F1 to avoid browser default behavior
//! (e.g., Chrome help page) in WASM builds.

use crate::common::TestCtx;
use kittest::Queryable;

mod common;

// =============================================================================
// HELPER FUNCTION
// =============================================================================

/// Helper function to check if the API status panel is visible via kittest query.
/// The panel contains a label "API Status" which we can query for.
fn is_api_status_visible(harness: &egui_kittest::Harness<'_, collects_ui::CollectsApp>) -> bool {
    harness.query_by_label_contains("API Status").is_some()
}

// =============================================================================
// TOGGLE FUNCTIONALITY TESTS (Using F1 key press via harness)
// =============================================================================

/// Tests that API status panel is hidden by default.
#[tokio::test]
async fn test_api_status_hidden_by_default() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run the first frame
    harness.step();

    // By default, the API status panel should not be visible
    assert!(
        !is_api_status_visible(harness),
        "API status panel should be hidden by default"
    );
}

/// Tests that Shift+F1 key press toggles the visibility from off to on.
#[tokio::test]
async fn test_f1_key_shows_api_status() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let initial API fetch complete
    for _ in 0..10 {
        harness.step();
    }
    // Wait for async API fetch to complete
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    for _ in 0..5 {
        harness.step();
    }

    // Verify initially hidden
    assert!(
        !is_api_status_visible(harness),
        "Should be hidden initially"
    );

    // Press Shift+F1 key to toggle
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F1);
    harness.step();
    harness.step();

    // Should now be visible - query for the "API Status" label
    assert!(
        is_api_status_visible(harness),
        "API status panel should be visible after Shift+F1 press"
    );
}

/// Tests that Shift+F1 key press toggles the visibility from on to off.
#[tokio::test]
async fn test_f1_key_hides_api_status() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let initial API fetch complete
    for _ in 0..10 {
        harness.step();
    }
    // Wait for async API fetch to complete
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    for _ in 0..5 {
        harness.step();
    }

    // Verify initially hidden
    assert!(
        !is_api_status_visible(harness),
        "Should be hidden initially"
    );

    // Press Shift+F1 to show
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F1);
    harness.step();
    harness.step();

    // Verify visible
    assert!(
        is_api_status_visible(harness),
        "Should be visible after first Shift+F1 press"
    );

    // Press Shift+F1 again to hide
    harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F1);
    harness.step();
    harness.step();

    // Should now be hidden
    assert!(
        !is_api_status_visible(harness),
        "API status panel should be hidden after second Shift+F1 press"
    );
}

/// Tests multiple Shift+F1 key presses toggle correctly.
#[tokio::test]
async fn test_multiple_f1_toggles() {
    let mut ctx = TestCtx::new_app().await;
    let harness = ctx.harness_mut();

    // Run several frames to let initial API fetch complete
    for _ in 0..10 {
        harness.step();
    }
    // Wait for async API fetch to complete
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    for _ in 0..5 {
        harness.step();
    }

    // Verify initially hidden
    assert!(
        !is_api_status_visible(harness),
        "Should be hidden initially"
    );

    // Toggle 10 times and verify the UI state alternates correctly
    for i in 0..10 {
        harness.key_press_modifiers(egui::Modifiers::SHIFT, egui::Key::F1);
        harness.step();
        harness.step();

        let expected_visible = (i + 1) % 2 == 1; // odd iterations: visible, even: hidden
        let actual_visible = is_api_status_visible(harness);

        assert_eq!(
            actual_visible,
            expected_visible,
            "After {} Shift+F1 presses, API status panel should be {}",
            i + 1,
            if expected_visible {
                "visible"
            } else {
                "hidden"
            }
        );
    }
}

// Note: A test for show_status persistence after API fetch was removed because
// it was inherently racy due to the async nature of the API fetch.
// The show_status preservation is tested in the widget tests where we have more
// control over the async timing.
