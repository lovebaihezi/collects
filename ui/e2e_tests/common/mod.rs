//! Common utilities for e2e tests.
//!
//! This module provides test context and helpers for running end-to-end tests
//! that connect to real backend services.

use collects_ui::CollectsApp;
use collects_ui::state::State;
use egui_kittest::Harness;

/// E2E test context that connects to real backend services.
///
/// Unlike integration tests that use mock servers, e2e tests connect to
/// actual deployed services to verify the full user flow.
pub struct E2eTestCtx<'a> {
    harness: Harness<'a, CollectsApp>,
}

impl<'a> E2eTestCtx<'a> {
    /// Creates a new e2e test context configured for a specific environment.
    ///
    /// The environment URL is determined by the cargo feature flags:
    /// - `env_test` -> https://collects-test.lqxclqxc.com
    /// - `env_test_internal` -> https://collects-test-internal.lqxclqxc.com
    ///
    /// For e2e tests, we typically use the `env_test` or `env_test_internal` environments.
    #[allow(unused)]
    pub fn new_app() -> Self {
        let _ = env_logger::builder().is_test(true).try_init();

        let state = State::default();
        let app = CollectsApp::new(state);
        let harness = Harness::new_eframe(|_| app);

        Self { harness }
    }

    /// Creates a new e2e test context with a custom API base URL.
    ///
    /// Use this when you need to test against a specific backend deployment.
    #[allow(unused)]
    pub fn new_app_with_base_url(base_url: impl Into<String>) -> Self {
        let _ = env_logger::builder().is_test(true).try_init();

        let state = State::test(base_url.into());
        let app = CollectsApp::new(state);
        let harness = Harness::new_eframe(|_| app);

        Self { harness }
    }

    /// Gets a mutable reference to the test harness.
    #[allow(unused)]
    pub fn harness_mut(&mut self) -> &mut Harness<'a, CollectsApp> {
        &mut self.harness
    }

    /// Gets a reference to the test harness.
    #[allow(unused)]
    pub fn harness(&self) -> &Harness<'a, CollectsApp> {
        &self.harness
    }
}
