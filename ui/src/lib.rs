#![warn(clippy::all, rust_2018_idioms)]

pub mod app;
pub mod state;
pub mod utils;
pub mod widgets;

pub use app::CollectsApp;

// TODO: share test utils with integration tests
#[cfg(test)]
pub mod test_utils {
    use egui_kittest::Harness;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    use crate::state::State;

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
            let _ = env_logger::builder().is_test(true).try_init();
            let (mock_server, state) = setup_test_state().await;
            let harness = Harness::new_ui_state(app, state);

            Self {
                _mock_server: mock_server,
                harness,
            }
        }
    }

    async fn setup_test_state() -> (MockServer, State) {
        let mock_server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/is-health"))
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        // Add mock for internal API endpoint when internal features are enabled
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
}
