use crate::common::TestCtx;

mod common;

#[tokio::test]
async fn test_api_status_with_200() {
    let mut ctx = TestCtx::new_app().await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // The widget should render successfully without errors
    // We can't easily test the drawn circle or tooltip with current kittest capabilities
}

#[tokio::test]
async fn test_api_status_with_404() {
    let mut ctx = TestCtx::new_app_with_status(404).await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // The widget should render successfully with error state
}

#[tokio::test]
async fn test_api_status_with_500() {
    let mut ctx = TestCtx::new_app_with_status(500).await;

    let harness = ctx.harness_mut();

    // Render the first frame
    harness.step();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    harness.step();

    // The widget should render successfully with error state
}
