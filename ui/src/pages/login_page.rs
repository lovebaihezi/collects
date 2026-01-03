//! Login page for unauthenticated users.
//!
//! Displays the login form centered on the screen.

use crate::{state::State, widgets};
use egui::{Response, Ui};

/// Renders the login page with a centered login form.
pub fn login_page(state: &mut State, ui: &mut Ui) -> Response {
    widgets::login_widget(&mut state.ctx, ui)
}

#[cfg(test)]
#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
mod login_page_test {
    use kittest::Queryable;

    use crate::test_utils::TestCtx;

    #[tokio::test]
    async fn test_login_page_renders_login_form() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::login_page(state, ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // Login form should be visible when not authenticated
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Username field should be displayed on login page"
        );
        assert!(
            harness.query_by_label_contains("OTP Code").is_some(),
            "OTP Code field should be displayed on login page"
        );
        assert!(
            harness.query_by_label_contains("Login").is_some(),
            "Login button should be displayed on login page"
        );
    }
}
