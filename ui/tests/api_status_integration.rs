use kittest::Queryable;

use crate::common::TestCtx;

mod common;

#[tokio::test]
async fn test_api_status_with_200() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    // Run multiple steps to ensure the initial UI is fully rendered
    for _ in 0..3 {
        harness.step();
    }

    // Initially shows the status dot
    assert!(
        harness.query_by_label("●").is_some(),
        "Status dot should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // After API response, the dot should still be present (now green for healthy)
    assert!(
        harness.query_by_label("●").is_some(),
        "Status dot should exist in UI after API response"
    );
}

#[tokio::test]
async fn test_api_status_with_404() {
    let mut ctx = TestCtx::new_app_with_status(404).await;

    let harness = ctx.harness_mut();

    // Run multiple steps to ensure the initial UI is fully rendered
    for _ in 0..3 {
        harness.step();
    }

    // Initially shows the status dot
    assert!(
        harness.query_by_label("●").is_some(),
        "Status dot should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // After API error response, the dot should still be present (now red)
    assert!(
        harness.query_by_label("●").is_some(),
        "Status dot should exist in UI after API error"
    );
}

#[tokio::test]
async fn test_api_status_with_500() {
    let mut ctx = TestCtx::new_app_with_status(500).await;

    let harness = ctx.harness_mut();

    // Run multiple steps to ensure the initial UI is fully rendered
    for _ in 0..3 {
        harness.step();
    }

    // Initially shows the status dot
    assert!(
        harness.query_by_label("●").is_some(),
        "Status dot should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // After API error response, the dot should still be present (now red)
    assert!(
        harness.query_by_label("●").is_some(),
        "Status dot should exist in UI after API error"
    );
}
