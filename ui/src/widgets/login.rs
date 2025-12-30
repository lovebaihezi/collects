//! Login widget for user authentication.
//!
//! Displays a centered login form with username and OTP input fields,
//! and shows "Signed" text after successful authentication.

use collects_business::{AuthCompute, AuthStatus, LoginCommand, LoginInput};
use collects_states::StateCtx;
use egui::{Align, Color32, Layout, Response, RichText, Ui};

/// Green color for success status
const COLOR_GREEN: Color32 = Color32::from_rgb(34, 139, 34);
/// Red color for error status
const COLOR_RED: Color32 = Color32::from_rgb(220, 53, 69);

/// Displays the login form or signed-in status based on authentication state.
///
/// Returns `true` if the user is authenticated, `false` otherwise.
pub fn login_widget(state_ctx: &mut StateCtx, ui: &mut Ui) -> Response {
    // Get current auth status
    let auth_status = state_ctx
        .cached::<AuthCompute>()
        .map(|c| c.status.clone())
        .unwrap_or_default();

    match auth_status {
        AuthStatus::Authenticated { username, .. } => {
            // Show signed-in status
            show_signed_in(ui, &username)
        }
        AuthStatus::Authenticating => {
            // Show loading state
            show_loading(ui)
        }
        AuthStatus::Failed(error) => {
            // Show login form with error
            show_login_form(state_ctx, ui, Some(&error))
        }
        AuthStatus::NotAuthenticated => {
            // Show login form
            show_login_form(state_ctx, ui, None)
        }
    }
}

/// Shows the signed-in status with the username.
fn show_signed_in(ui: &mut Ui, username: &str) -> Response {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(20.0);
        ui.heading("Collects App");
        ui.add_space(40.0);

        ui.label(RichText::new("Signed").size(24.0).color(COLOR_GREEN));
        ui.add_space(8.0);
        ui.label(format!("Welcome, {username}"));
    })
    .response
}

/// Shows the loading state during authentication.
fn show_loading(ui: &mut Ui) -> Response {
    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        ui.add_space(20.0);
        ui.heading("Collects App");
        ui.add_space(40.0);

        ui.spinner();
        ui.label("Authenticating...");
    })
    .response
}

/// Shows the login form with optional error message.
fn show_login_form(state_ctx: &mut StateCtx, ui: &mut Ui, error: Option<&str>) -> Response {
    // Get mutable reference to login input
    let login_input = state_ctx.state_mut::<LoginInput>();

    let mut username = login_input.username.clone();
    let mut otp = login_input.otp.clone();
    let mut should_login = false;

    let response = ui
        .with_layout(Layout::top_down(Align::Center), |ui| {
            ui.add_space(20.0);
            ui.heading("Collects App");
            ui.add_space(40.0);

            // Show error message if present
            if let Some(err) = error {
                ui.colored_label(COLOR_RED, err);
                ui.add_space(8.0);
            }

            // Username input
            ui.horizontal(|ui| {
                ui.label("Username:");
                ui.text_edit_singleline(&mut username);
            });

            ui.add_space(8.0);

            // OTP input
            ui.horizontal(|ui| {
                ui.label("OTP Code:");
                let otp_response = ui.text_edit_singleline(&mut otp);

                // Check for Enter key press
                if otp_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    should_login = true;
                }
            });

            ui.add_space(16.0);

            // Login button
            let can_login = !username.trim().is_empty() && !otp.trim().is_empty();
            if ui
                .add_enabled(can_login, egui::Button::new("Login"))
                .clicked()
            {
                should_login = true;
            }
        })
        .response;

    // Update state if values changed
    let login_input = state_ctx.state_mut::<LoginInput>();
    if login_input.username != username {
        login_input.username = username;
    }
    if login_input.otp != otp {
        login_input.otp = otp;
    }

    // Trigger login if requested
    if should_login {
        state_ctx.dispatch::<LoginCommand>();
    }

    response
}

#[cfg(test)]
mod login_widget_tests {
    use kittest::Queryable;

    use crate::test_utils::TestCtx;

    #[tokio::test]
    async fn test_login_form_displayed() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::login_widget(&mut state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // Check that the heading is displayed
        assert!(
            harness.query_by_label_contains("Collects App").is_some(),
            "Collects App heading should be displayed"
        );

        // Check that username label is displayed
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Username label should be displayed"
        );

        // Check that OTP label is displayed
        assert!(
            harness.query_by_label_contains("OTP Code").is_some(),
            "OTP Code label should be displayed"
        );

        // Check that Login button is displayed
        assert!(
            harness.query_by_label_contains("Login").is_some(),
            "Login button should be displayed"
        );
    }
}
