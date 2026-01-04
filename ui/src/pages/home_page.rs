//! Home page for authenticated users (non-internal builds).
//!
//! Displays the signed-in header and image preview.

use crate::{state::State, widgets};
use collects_business::AuthCompute;
use egui::{Response, Ui};

/// Renders the home page for authenticated users.
///
/// Shows the signed-in header with username and the image preview.
pub fn home_page(state: &mut State, ui: &mut Ui) -> Response {
    // Get username for display
    let username = state
        .ctx
        .cached::<AuthCompute>()
        .and_then(|c| c.username().map(String::from))
        .unwrap_or_default();

    ui.vertical(|ui| {
        // Show signed-in header (reusing the shared widget)
        widgets::show_signed_in_header(ui, &username);

        ui.add_space(16.0);

        // Image preview section
        ui.heading("Image Preview");
        ui.label("Paste an image (Ctrl+V) to display it here. Click to maximize.");
        ui.add_space(8.0);

        // Get the image preview state and render
        let image_state = state.ctx.state_mut::<widgets::ImagePreviewState>();
        widgets::image_preview(image_state, ui);

        ui.add_space(16.0);
        widgets::powered_by_egui_and_eframe(ui);
    })
    .response
}

#[cfg(test)]
mod home_page_test {
    use collects_business::{AuthCompute, AuthStatus};
    use collects_states::StateCtx;
    use egui_kittest::Harness;
    use kittest::Queryable;

    /// Helper to create a StateCtx with authenticated status.
    fn create_authenticated_state() -> StateCtx {
        let mut ctx = StateCtx::new();
        ctx.record_compute(AuthCompute {
            status: AuthStatus::Authenticated {
                username: "TestUser".to_string(),
                token: None,
            },
        });
        ctx
    }

    #[test]
    fn test_home_page_shows_signed_in_header() {
        let ctx = create_authenticated_state();

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                // Get username for display
                let username = state_ctx
                    .cached::<AuthCompute>()
                    .and_then(|c| c.username().map(String::from))
                    .unwrap_or_default();

                // Show signed-in header (simulating the home_page behavior)
                crate::widgets::show_signed_in_header(ui, &username);
            },
            ctx,
        );

        // Should show "Signed" status
        assert!(
            harness.query_by_label_contains("Signed").is_some(),
            "Home page should show 'Signed' status"
        );

        // Should show welcome message with username
        assert!(
            harness.query_by_label_contains("Welcome").is_some(),
            "Home page should show Welcome message"
        );
        assert!(
            harness.query_by_label_contains("TestUser").is_some(),
            "Home page should show the username"
        );
    }
}
