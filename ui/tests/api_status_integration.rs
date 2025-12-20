use kittest::Queryable;

use crate::common::TestCtx;

mod common;

#[tokio::test]
async fn test_api_status_with_200() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    assert!(
        harness.query_by_label("API Status: Checking...").is_some(),
        "'API Status: Checking...' should exists in UI"
    );

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    assert!(
        harness.query_by_label("API Status: Healthy").is_some(),
        "'Api Status: Healthy' should exists in UI"
    );
}
