use kittest::Queryable;

use crate::common::TestCtx;

mod common;

#[tokio::test]
async fn test_api_status_with_200() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    // Check for API Status label - may be "Checking..." or already "Healthy"
    let has_checking = harness.query_by_label("API Status: Checking...").is_some();
    let has_healthy = harness.query_by_label("API Status: Healthy").is_some();
    assert!(
        has_checking || has_healthy,
        "'API Status: Checking...' or 'API Status: Healthy' should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    assert!(
        harness.query_by_label("API Status: Healthy").is_some(),
        "'API Status: Healthy' should exist in UI"
    );
}

#[tokio::test]
async fn test_api_status_with_404() {
    let mut ctx = TestCtx::new_app_with_status(404).await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    // Check for API Status label - may be "Checking..." or already resolved
    let has_checking = harness.query_by_label("API Status: Checking...").is_some();
    let has_error = harness.query_by_label("API Health: 404").is_some();
    assert!(
        has_checking || has_error,
        "'API Status: Checking...' or 'API Health: 404' should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    assert!(
        harness.query_by_label("API Health: 404").is_some(),
        "'API Health: 404' should exist in UI"
    );
}

#[tokio::test]
async fn test_api_status_with_500() {
    let mut ctx = TestCtx::new_app_with_status(500).await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    // Check for API Status label - may be "Checking..." or already resolved
    let has_checking = harness.query_by_label("API Status: Checking...").is_some();
    let has_error = harness.query_by_label("API Health: 500").is_some();
    assert!(
        has_checking || has_error,
        "'API Status: Checking...' or 'API Health: 500' should exist in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    assert!(
        harness.query_by_label("API Health: 500").is_some(),
        "'API Health: 500' should exist in UI"
    );
}
