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

    // We run run compute at the ends of update, which, second step will able to update the api status
    harness.step();

    assert!(
        harness.query_by_label("API Status: Checking...").is_some(),
        "'API Status: Checking...' should exists in UI"
    );

    harness.run_steps(60);

    assert!(
        harness.query_by_label("API Status: Healthy").is_some(),
        "'Api Status: Healthy' should exists in UI"
    );
}
