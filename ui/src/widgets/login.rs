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

/// Fixed width for labels in the login form
const LABEL_WIDTH: f32 = 70.0;

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
    // Get read-only reference to login input for initial values
    let login_input = state_ctx.state::<LoginInput>();

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
            let row_width = LABEL_WIDTH + INPUT_FIELD_WIDTH + ui.spacing().item_spacing.x;
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

    // Update state if values changed using update() for proper dirty propagation
    let current_input = state_ctx.state::<LoginInput>();
    if current_input.username != username || current_input.otp != otp {
        let new_username = username;
        let new_otp = otp;
        state_ctx.update::<LoginInput>(|input| {
            input.username = new_username;
            input.otp = new_otp;
        });
    }

    // Trigger login if requested
    if should_login {
        state_ctx.dispatch::<LoginCommand>();
    }

    response
}

/// Tests for login form positioning (only for non-internal builds).
/// Internal builds use Zero Trust authentication and skip the login form.
#[cfg(test)]
#[cfg(not(any(feature = "env_internal", feature = "env_test_internal")))]
mod login_widget_test {
    use kittest::Queryable;

    use crate::test_utils::TestCtx;

    /// Default screen center X coordinate in egui_kittest (800.0 width / 2)
    const TEST_SCREEN_CENTER_X: f32 = 400.0;

    /// Tolerance for checking if input rows are centered
    /// (larger because the label + input form a row)
    const INPUT_CENTERING_TOLERANCE: f32 = 200.0;

    /// Tolerance for checking if buttons are centered
    /// (smaller because buttons should be exactly centered)
    const BUTTON_CENTERING_TOLERANCE: f32 = 50.0;

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

        // Check that the username label's center is close to the screen center
        // We allow a tolerance since the label and input together form a row
        let label_center_x = username_rect.center().x;

        // The label should be somewhat centered (within tolerance)
        let distance_from_center = (label_center_x - TEST_SCREEN_CENTER_X).abs();
        assert!(
            distance_from_center < INPUT_CENTERING_TOLERANCE,
            "Username label should be near center. Label center: {label_center_x}, screen center: {TEST_SCREEN_CENTER_X}, distance: {distance_from_center}"
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

        let label_center_x = otp_rect.center().x;

        let distance_from_center = (label_center_x - TEST_SCREEN_CENTER_X).abs();
        assert!(
            distance_from_center < INPUT_CENTERING_TOLERANCE,
            "OTP label should be near center. Label center: {label_center_x}, screen center: {TEST_SCREEN_CENTER_X}, distance: {distance_from_center}"
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

        let button_center_x = button_rect.center().x;

        // The button should be very close to center
        let distance_from_center = (button_center_x - TEST_SCREEN_CENTER_X).abs();
        assert!(
            distance_from_center < BUTTON_CENTERING_TOLERANCE,
            "Login button should be centered. Button center: {button_center_x}, screen center: {TEST_SCREEN_CENTER_X}, distance: {distance_from_center}"
        );
    }
}

/// Tests for Zero Trust authentication behavior.
#[cfg(test)]
mod login_widget_zero_trust_tests {
    use super::*;
    use collects_business::AuthStatus;
    use collects_states::StateCtx;
    use egui_kittest::Harness;
    use kittest::Queryable;

    /// Helper to create a StateCtx with authentication status.
    fn create_state_ctx_with_auth(status: AuthStatus) -> StateCtx {
        let mut ctx = StateCtx::new();
        ctx.add_state(LoginInput::default());
        ctx.record_compute(AuthCompute { status });
        ctx
    }

    #[test]
    fn test_login_widget_shows_form_when_not_authenticated() {
        let ctx = create_state_ctx_with_auth(AuthStatus::NotAuthenticated);

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                login_widget(state_ctx, ui);
            },
            ctx,
        );

        // The login form should show username and OTP fields
        assert!(
            harness.query_by_label_contains("Username").is_some(),
            "Username field should be visible when not authenticated"
        );
        assert!(
            harness.query_by_label_contains("OTP Code").is_some(),
            "OTP Code field should be visible when not authenticated"
        );
        assert!(
            harness.query_by_label_contains("Login").is_some(),
            "Login button should be visible when not authenticated"
        );
    }

    #[test]
    fn test_login_widget_shows_signed_in_when_authenticated() {
        let ctx = create_state_ctx_with_auth(AuthStatus::Authenticated {
            username: "Test User".to_string(),
            token: None,
        });

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                login_widget(state_ctx, ui);
            },
            ctx,
        );

        // The signed-in status should show the username
        assert!(
            harness.query_by_label_contains("Welcome").is_some(),
            "Welcome message should be visible when authenticated"
        );
        assert!(
            harness.query_by_label_contains("Signed").is_some(),
            "Signed status should be visible when authenticated"
        );
        // Login form elements should NOT be visible
        assert!(
            harness.query_by_label_contains("OTP Code").is_none(),
            "OTP Code field should NOT be visible when authenticated"
        );
    }

    #[test]
    fn test_login_widget_zero_trust_authenticated_skips_login_form() {
        // Simulates the Zero Trust authentication scenario where users
        // are already authenticated via Cloudflare Access
        let auth_compute = AuthCompute::zero_trust_authenticated();
        let mut ctx = StateCtx::new();
        ctx.add_state(LoginInput::default());
        ctx.record_compute(auth_compute);

        let harness = Harness::new_ui_state(
            |ui, state_ctx| {
                login_widget(state_ctx, ui);
            },
            ctx,
        );

        // Should show signed-in status for Zero Trust user
        assert!(
            harness.query_by_label_contains("Welcome").is_some(),
            "Welcome message should be visible for Zero Trust authenticated user"
        );
        assert!(
            harness.query_by_label_contains("Zero Trust User").is_some(),
            "Zero Trust User name should be displayed"
        );
        // Login form elements should NOT be visible
        assert!(
            harness.query_by_label_contains("OTP Code").is_none(),
            "Login form should NOT be visible for Zero Trust authenticated user"
        );
    }
}
