use kittest::Queryable;

use crate::common::TestCtx;

mod common;

/// Helper function to trigger tooltip by hovering and running multiple frames
fn trigger_tooltip(harness: &mut egui_kittest::Harness<'_, collects_ui::CollectsApp>) {
    // Some configurations render multiple status dots. Hover the first one.
    if let Some(dot) = harness.query_all_by_label("●").next() {
        dot.hover();
    }
    // Run multiple frames to allow tooltip delay to pass
    harness.run_steps(10);
}

/// Helper function to check if tooltip contains expected text
fn has_tooltip_containing(
    harness: &egui_kittest::Harness<'_, collects_ui::CollectsApp>,
    expected: &str,
) -> bool {
    harness.query_by_label_contains(expected).is_some()
}

#[tokio::test]
async fn test_api_status_with_200() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    // Initially shows the status dot
    assert!(
        harness.query_all_by_label("●").next().is_some(),
        "Status dot should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // After API response, the dot should still be present (now green for healthy)
    assert!(
        harness.query_all_by_label("●").next().is_some(),
        "Status dot should exist in UI after API response"
    );

    // Trigger tooltip and check it shows version (mock server returns "0.1.0+test")
    // After waiting, the API should have responded successfully
    trigger_tooltip(harness);
    assert!(
        has_tooltip_containing(harness, "api:0.1.0+test"),
        "Tooltip should show 'api:0.1.0+test' after successful response"
    );
}

#[tokio::test]
async fn test_api_status_with_404() {
    let mut ctx = TestCtx::new_app_with_status(404).await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    // Initially shows the status dot
    assert!(
        harness.query_all_by_label("●").next().is_some(),
        "Status dot should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // After API error response, the dot should still be present (now red)
    assert!(
        harness.query_all_by_label("●").next().is_some(),
        "Status dot should exist in UI after API error"
    );

    // Trigger tooltip and check it shows error info
    trigger_tooltip(harness);
    assert!(
        has_tooltip_containing(harness, "api("),
        "Tooltip should contain error information after 404"
    );
}

#[tokio::test]
async fn test_api_status_with_500() {
    let mut ctx = TestCtx::new_app_with_status(500).await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    // Initially shows the status dot
    assert!(
        harness.query_all_by_label("●").next().is_some(),
        "Status dot should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // After API error response, the dot should still be present (now red)
    assert!(
        harness.query_all_by_label("●").next().is_some(),
        "Status dot should exist in UI after API error"
    );

    // Trigger tooltip and check it shows error info
    trigger_tooltip(harness);
    assert!(
        has_tooltip_containing(harness, "api("),
        "Tooltip should contain error information after 500"
    );
}
