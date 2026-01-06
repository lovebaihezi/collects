use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;
use wiremock::Mock;
use wiremock::matchers::{method, path};
use wiremock::{MockServer, ResponseTemplate};

pub struct TestCtx<'a, T = State> {
    _mock_server: MockServer,
    harness: Harness<'a, T>,
}

impl<'a, T> TestCtx<'a, T> {
    #[allow(unused)]
    pub fn harness_mut(&mut self) -> &mut Harness<'a, T> {
        &mut self.harness
    }

    #[allow(unused)]
    pub fn harness(&self) -> &Harness<'a, T> {
        &self.harness
    }
}

impl<'a> TestCtx<'a, State> {
    #[allow(unused)]
    pub async fn new(app: impl FnMut(&mut egui::Ui, &mut State) + 'a) -> Self {
        let (mock_server, state) = setup_test_state().await;
        let state = state;
        let harness = Harness::new_ui_state(app, state);

        Self {
            _mock_server: mock_server,
            harness,
        }
    }
}

impl<'a> TestCtx<'a, CollectsApp> {
    #[allow(unused)]
    pub async fn new_app() -> Self {
        let (mock_server, state) = setup_test_state().await;
        let app = CollectsApp::new(state);
        let harness = Harness::new_eframe(|_| app);

        Self {
            _mock_server: mock_server,
            harness,
        }
    }

    #[allow(unused)]
    pub async fn new_app_with_status(status_code: u16) -> Self {
        let (mock_server, state) = setup_test_state_with_status(status_code).await;
        let app = CollectsApp::new(state);
        let harness = Harness::new_eframe(|_| app);

        Self {
            _mock_server: mock_server,
            harness,
        }
    }

    #[allow(unused)]
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    pub async fn new_app_with_users() -> Self {
        let (mock_server, state) = setup_test_state_with_users().await;
        let app = CollectsApp::new(state);
        let harness = Harness::new_eframe(|_| app);

        Self {
            _mock_server: mock_server,
            harness,
        }
    }
}

async fn setup_test_state() -> (MockServer, State) {
    setup_test_state_with_status(200).await
}

async fn setup_test_state_with_status(status_code: u16) -> (MockServer, State) {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(
            ResponseTemplate::new(status_code).insert_header("x-service-version", "0.1.0+test"),
        )
        .mount(&mock_server)
        .await;

    // Mock the internal users endpoint (needed when internal features are enabled)
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": []
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();

    let state = State::test(base_url);

    (mock_server, state)
}

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
async fn setup_test_state_with_users() -> (MockServer, State) {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200).insert_header("x-service-version", "0.1.0+test"))
        .mount(&mock_server)
        .await;

    // Mock the internal users endpoint with sample user data including profile fields
    Mock::given(method("GET"))
        .and(path("/api/internal/users"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "users": [
                {
                    "username": "alice",
                    "current_otp": "123456",
                    "time_remaining": 25,
                    "nickname": "Alice Wonderland",
                    "avatar_url": "https://example.com/avatar/alice.png",
                    "created_at": "2026-01-01T10:00:00Z",
                    "updated_at": "2026-01-05T15:30:00Z"
                },
                {
                    "username": "bob",
                    "current_otp": "654321",
                    "time_remaining": 15,
                    "nickname": null,
                    "avatar_url": null,
                    "created_at": "2026-01-02T12:00:00Z",
                    "updated_at": "2026-01-02T12:00:00Z"
                }
            ]
        })))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();

    let state = State::test(base_url);

    (mock_server, state)
}
