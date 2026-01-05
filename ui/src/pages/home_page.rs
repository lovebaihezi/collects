//! Home page for authenticated users (non-internal builds).
//!
//! Displays the signed-in welcome header.
//! When an image is pasted, it displays full screen.

use crate::{state::State, widgets};
use collects_business::AuthCompute;
use egui::{Response, Ui};

/// Renders the home page for authenticated users.
///
/// When no image is pasted: Shows the signed-in welcome header.
/// When an image is pasted: Shows the image full screen without the header.
pub fn home_page(state: &mut State, ui: &mut Ui) -> Response {
    // Get username for display
    let username = state
        .ctx
        .cached::<AuthCompute>()
        .and_then(|c| c.username().map(String::from))
        .unwrap_or_default();

    // Check if we have an image to display full screen
    let has_image = {
        let image_state = state.ctx.state_mut::<widgets::ImagePreviewState>();
        image_state.has_image()
    };

    if has_image {
        // Full screen image display mode - no header
        ui.vertical_centered(|ui| {
            let image_state = state.ctx.state_mut::<widgets::ImagePreviewState>();
            widgets::image_preview_fullscreen(image_state, ui);
        })
        .response
    } else {
        // Normal mode - show signed-in header only (no image preview tips)
        widgets::show_signed_in_header(ui, &username)
    }
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
