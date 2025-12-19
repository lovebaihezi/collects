use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;
use kittest::Queryable;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_api_status_with_200() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();
    let state = State::test(base_url);

    let mut harness: Harness<State> = Harness::new_state(
        |_, state| {
            CollectsApp::new(state);
        },
        state,
    );

    if let Some(n) = harness.query_by_label_contains("API Status") {
        eprintln!("API STATUS {:?}", n);
    }

    assert!(
        harness.query_by_label("API Status: Checking...").is_some(),
        "'API Status: Checking...' should exists in UI"
    );

    harness.step();

    assert!(
        harness.query_by_label("API Status: Healthy").is_some(),
        "'Api Status: Healthy' should exists in UI"
    );
}
