//! Login widget for user authentication.
//!
//! Displays a centered login form with username and OTP input fields,
//! and shows "Signed" text after successful authentication.

use crate::utils::colors::{COLOR_GREEN, COLOR_RED};
use collects_business::{AuthCompute, AuthStatus, LoginCommand, LoginInput};
use collects_states::StateCtx;
use egui::{Align, Layout, Response, RichText, Ui};

/// Estimated height of the login form (heading + fields + button + spacing)
const LOGIN_FORM_HEIGHT: f32 = 250.0;

/// Estimated height of status screens (signed-in header, loading state)
const STATUS_SCREEN_HEIGHT: f32 = 150.0;

/// Fixed width for input fields to ensure proper centering
const INPUT_FIELD_WIDTH: f32 = 200.0;

/// Calculate vertical spacing to center content on screen.
///
/// Returns the amount of space to add at the top to vertically center
/// content with the given estimated height.
fn calculate_vertical_centering(ui: &Ui, estimated_content_height: f32) -> f32 {
    let available_height = ui.available_height();
    (available_height - estimated_content_height).max(0.0) / 2.0
}

/// Displays the login form or signed-in status based on authentication state.
pub fn login_widget(state_ctx: &mut StateCtx, ui: &mut Ui) -> Response {
    // Get current auth status
    let auth_status = state_ctx
        .cached::<AuthCompute>()
        .map(|c| c.status.clone())
        .unwrap_or_default();

    match auth_status {
        AuthStatus::Authenticated { username, .. } => {
            // Show signed-in status
            show_signed_in_header(ui, &username)
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
///
/// This can be used both by the login widget and by other parts of the app
/// that need to display the signed-in header.
pub fn show_signed_in_header(ui: &mut Ui, username: &str) -> Response {
    let top_spacing = calculate_vertical_centering(ui, STATUS_SCREEN_HEIGHT);

    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        // Add vertical spacing to center the content
        ui.add_space(top_spacing);

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
    let top_spacing = calculate_vertical_centering(ui, STATUS_SCREEN_HEIGHT);

    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        // Add vertical spacing to center the content
        ui.add_space(top_spacing);

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

    let top_spacing = calculate_vertical_centering(ui, LOGIN_FORM_HEIGHT);

    let response = ui
        .with_layout(Layout::top_down(Align::Center), |ui| {
            // Add vertical spacing to center the form
            ui.add_space(top_spacing);

            ui.heading("Collects App");
            ui.add_space(40.0);

            // Show error message if present
            if let Some(err) = error {
                ui.colored_label(COLOR_RED, err);
                ui.add_space(8.0);
            }

            // Username input - use a sized horizontal group to ensure centering
            let row_width = 70.0 + INPUT_FIELD_WIDTH + ui.spacing().item_spacing.x; // label width + input + spacing
            ui.allocate_ui_with_layout(
                egui::vec2(row_width, ui.spacing().interact_size.y),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label("Username:");
                    ui.add(
                        egui::TextEdit::singleline(&mut username).desired_width(INPUT_FIELD_WIDTH),
                    );
                },
            );

            ui.add_space(8.0);

            // OTP input - use a sized horizontal group to ensure centering
            let otp_response = ui
                .allocate_ui_with_layout(
                    egui::vec2(row_width, ui.spacing().interact_size.y),
                    Layout::left_to_right(Align::Center),
                    |ui| {
                        ui.label("OTP Code:");
                        ui.add(
                            egui::TextEdit::singleline(&mut otp).desired_width(INPUT_FIELD_WIDTH),
                        )
                    },
                )
                .inner;

            // Check for Enter key press
            if otp_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                should_login = true;
            }

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
mod login_widget_test {
    use kittest::Queryable;

    use crate::test_utils::TestCtx;

    /// Tests that the username input field is horizontally centered.
    ///
    /// The input field should be centered relative to the screen width.
    /// This test verifies that the center of the username row is close to
    /// the horizontal center of the available area.
    #[tokio::test]
    async fn test_username_input_is_centered() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::login_widget(&mut state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // Find the username label
        let username_label = harness.query_by_label_contains("Username");
        assert!(
            username_label.is_some(),
            "Username label should be displayed"
        );

        // Get the rect of the username label
        let username_rect = username_label.as_ref().map(|n| n.rect());
        assert!(
            username_rect.is_some(),
            "Username label should have a bounding rect"
        );

        let username_rect = username_rect.unwrap();

        // The screen width is 800.0 by default in egui_kittest
        let screen_center_x = 400.0;

        // Check that the username label's center is close to the screen center
        // We allow a tolerance since the label and input together form a row
        let label_center_x = username_rect.center().x;

        // The label should be somewhat centered (within 200 pixels of center)
        // This is a reasonable tolerance since the label + input row together should be centered
        let distance_from_center = (label_center_x - screen_center_x).abs();
        assert!(
            distance_from_center < 200.0,
            "Username label should be near center. Label center: {label_center_x}, screen center: {screen_center_x}, distance: {distance_from_center}"
        );
    }

    /// Tests that the OTP input field is horizontally centered.
    #[tokio::test]
    async fn test_otp_input_is_centered() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::login_widget(&mut state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // Find the OTP label
        let otp_label = harness.query_by_label_contains("OTP Code");
        assert!(otp_label.is_some(), "OTP Code label should be displayed");

        let otp_rect = otp_label.as_ref().map(|n| n.rect()).unwrap();

        // The screen width is 800.0 by default in egui_kittest
        let screen_center_x = 400.0;
        let label_center_x = otp_rect.center().x;

        let distance_from_center = (label_center_x - screen_center_x).abs();
        assert!(
            distance_from_center < 200.0,
            "OTP label should be near center. Label center: {label_center_x}, screen center: {screen_center_x}, distance: {distance_from_center}"
        );
    }

    /// Tests that the Login button is horizontally centered.
    #[tokio::test]
    async fn test_login_button_is_centered() {
        let mut ctx = TestCtx::new(|ui, state| {
            super::login_widget(&mut state.ctx, ui);
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        // Find the Login button
        let login_button = harness.query_by_label_contains("Login");
        assert!(login_button.is_some(), "Login button should be displayed");

        let button_rect = login_button.as_ref().map(|n| n.rect()).unwrap();

        // The screen width is 800.0 by default in egui_kittest
        let screen_center_x = 400.0;
        let button_center_x = button_rect.center().x;

        // The button should be very close to center (within 50 pixels)
        let distance_from_center = (button_center_x - screen_center_x).abs();
        assert!(
            distance_from_center < 50.0,
            "Login button should be centered. Button center: {button_center_x}, screen center: {screen_center_x}, distance: {distance_from_center}"
        );
    }
}
