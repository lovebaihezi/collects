//! Login page for unauthenticated users.
//!
//! Displays the login form and image preview grid.

use crate::{state::State, widgets};
use egui::{Response, Ui};

/// Renders the login page with a login form and image preview grid.
pub fn login_page(state: &mut State, ui: &mut Ui) -> Response {
    ui.vertical(|ui| {
        widgets::login_widget(&mut state.ctx, ui);

        ui.add_space(16.0);

        // Image preview section (available even before login)
        ui.heading("Image Preview");
        ui.label("Paste images (Ctrl+V) to add them to the grid. Click to maximize.");
        ui.add_space(8.0);

        let image_state = state.ctx.state_mut::<widgets::ImagePreviewState>();
        widgets::image_preview_grid(image_state, ui);
    })
    .response
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
