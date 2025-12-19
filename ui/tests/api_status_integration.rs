use collects_business::{ApiConfig, ApiStatus};
use collects_states::{StateCtx, Time};
use collects_ui::widgets::api_status::api_status;
use egui_kittest::Harness;
use kittest::Queryable;
use std::cell::RefCell;
use std::rc::Rc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_api_status_integration() {
    // 1. Setup Mock Server
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let mock_url = mock_server.uri();
    println!("Mock server running at: {}", mock_url);

    // 2. Setup StateCtx with Mock Configuration
    let mut state_ctx = StateCtx::new();
    state_ctx.add_state(Time::default());
    // Inject the mock URL config BEFORE the compute runs
    // Use the new constructor
    state_ctx.add_state(ApiConfig::new(mock_url));
    state_ctx.record_compute(ApiStatus::default());

    // Wrap state_ctx to share between harness and test driver
    let state_ctx = Rc::new(RefCell::new(state_ctx));
    let state_ctx_clone = state_ctx.clone();

    // 3. Setup kittest Harness
    let mut harness = Harness::new_ui(move |ui| {
        let ctx = state_ctx_clone.borrow();
        api_status(&ctx, ui);
    });

    // 4. Initial Render & Update
    // Run the compute once (initially it will fetch)
    state_ctx.borrow_mut().run_computed();
    state_ctx.borrow_mut().sync_computes();

    // Render the UI
    harness.run();

    // Verify initial state (likely "Checking..." because ehttp fetch is async/background)
    harness
        .query_by_text("API Status: Checking...")
        .assert_exists();

    // 5. Wait for Async Update
    // Poll for success
    let mut success = false;
    for _ in 0..50 {
        // Wait a bit for the http request to finish
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Process updates
        state_ctx.borrow_mut().sync_computes();
        // Rerun compute if dependencies changed
        state_ctx.borrow_mut().run_computed();

        // Render again to update the UI
        harness.run();

        // Check if "Healthy" appeared
        if harness.get_by_text("API Status: Healthy").exists() {
            success = true;
            break;
        }
    }

    assert!(success, "Failed to get Healthy status within timeout");
}
