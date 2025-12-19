use collects_business::{ApiConfig, ApiStatus};
use collects_states::{StateCtx, Time};
use collects_ui::widgets::api_status::api_status;
use egui_kittest::Harness;
use kittest::Queryable;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};
use std::cell::RefCell;
use std::rc::Rc;

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
    state_ctx.add_state(ApiConfig::new(mock_url));
    state_ctx.record_compute(ApiStatus::default());

    let state_ctx = Rc::new(RefCell::new(state_ctx));
    let state_ctx_clone = state_ctx.clone();

    // 3. Setup kittest Harness
    let mut harness = Harness::new_ui(move |ui| {
        let ctx = state_ctx_clone.borrow();
        api_status(&ctx, ui);
    });

    // 4. Initial Render
    state_ctx.borrow_mut().run_computed();
    state_ctx.borrow_mut().sync_computes();
    harness.run();

    // Verify initial state
    // Use get_by_label, which panics if not found (proving existence)
    harness.get_by_label("API Status: Checking...");

    // 5. Wait for Async Update
    let mut success = false;
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        state_ctx.borrow_mut().sync_computes();
        state_ctx.borrow_mut().run_computed();
        harness.run();

        // Check if "Healthy" appeared
        // query_by_label returns Option, so is_some() is sufficient
        if harness.query_by_label("API Status: Healthy").is_some() {
            success = true;
            break;
        }
    }

    assert!(success, "Failed to get Healthy status within timeout");
}
