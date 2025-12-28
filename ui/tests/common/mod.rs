use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;
use wiremock::Mock;
use wiremock::matchers::{body_json, method, path};
use wiremock::{MockServer, ResponseTemplate};

pub struct TestCtx<'a, T = State> {
    _mock_server: MockServer,
    harness: Harness<'a, T>,
}

impl<'a, T> TestCtx<'a, T> {
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
    pub async fn new_app() -> Self {
        let (mock_server, state) = setup_test_state().await;
        let app = CollectsApp::new(state);
        let harness = Harness::new_eframe(|_| app);

        Self {
            _mock_server: mock_server,
            harness,
        }
    }

    pub async fn new_app_with_status(status_code: u16) -> Self {
        let (mock_server, state) = setup_test_state_with_status(status_code).await;
        let app = CollectsApp::new(state);
        let harness = Harness::new_eframe(|_| app);

        Self {
            _mock_server: mock_server,
            harness,
        }
    }

    pub async fn new_app_with_auth(valid_username: &str, valid_otp: &str) -> Self {
        let (mock_server, state) = setup_test_state_with_auth(valid_username, valid_otp).await;
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
        .respond_with(ResponseTemplate::new(status_code))
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();

    let state = State::test(base_url);

    (mock_server, state)
}

/// Sets up test state with auth endpoint mocking
async fn setup_test_state_with_auth(
    valid_username: &str,
    valid_otp: &str,
) -> (MockServer, State) {
    let mock_server = MockServer::start().await;

    // Mock health endpoint
    Mock::given(method("GET"))
        .and(path("/api/is-health"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&mock_server)
        .await;

    // Mock successful OTP verification
    Mock::given(method("POST"))
        .and(path("/api/auth/verify-otp"))
        .and(body_json(serde_json::json!({
            "username": valid_username,
            "code": valid_otp
        })))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "valid": true })),
        )
        .mount(&mock_server)
        .await;

    // Mock failed OTP verification (any other credentials)
    Mock::given(method("POST"))
        .and(path("/api/auth/verify-otp"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "valid": false })),
        )
        .mount(&mock_server)
        .await;

    let base_url = mock_server.uri();

    let state = State::test(base_url);

    (mock_server, state)
}
