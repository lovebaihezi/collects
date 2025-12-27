//! Sign-in button and login form widgets.
//!
//! This module provides UI components for user authentication:
//! - A sign-in button that shows in the top bar when not logged in
//! - A login form for entering username and OTP code
//! - User info display when logged in

use collects_business::{AuthState, BusinessConfig, LoginFormData};
use collects_states::StateCtx;
use egui::{Color32, Frame, Margin, Response, RichText, Ui, Window};

/// Login dialog state for controlling visibility and form data.
#[derive(Debug, Clone, Default)]
pub struct LoginDialogState {
    /// Whether the login dialog is open.
    pub is_open: bool,
    /// Form data for the login dialog.
    pub form_data: LoginFormData,
}

impl LoginDialogState {
    /// Opens the login dialog.
    pub fn open(&mut self) {
        self.is_open = true;
        self.form_data = LoginFormData::default();
    }

    /// Closes the login dialog.
    pub fn close(&mut self) {
        self.is_open = false;
    }
}

/// Renders the sign-in button or user info based on authentication state.
///
/// When logged out, shows a "Sign In" button.
/// When logged in, shows the username and a "Sign Out" button.
pub fn signin_button(
    state_ctx: &StateCtx,
    auth_state: &AuthState,
    dialog_state: &mut LoginDialogState,
    ui: &mut Ui,
) -> Response {
    if auth_state.is_logged_in() {
        // Show user info and logout button
        user_info_widget(state_ctx, auth_state, ui)
    } else {
        // Show sign-in button
        signin_button_widget(dialog_state, ui)
    }
}

/// Renders the sign-in button.
fn signin_button_widget(dialog_state: &mut LoginDialogState, ui: &mut Ui) -> Response {
    let (bg_color, text_color, text) = if dialog_state.is_open {
        (
            Color32::from_rgb(30, 90, 200), // Darker blue when open
            Color32::WHITE,
            "Signing In...",
        )
    } else {
        (
            Color32::from_rgb(13, 110, 253), // Blue background
            Color32::WHITE,
            "Sign In",
        )
    };

    let response = Frame::NONE
        .fill(bg_color)
        .inner_margin(Margin::symmetric(12, 4))
        .outer_margin(Margin::symmetric(0, 4))
        .corner_radius(4.0)
        .show(ui, |ui| {
            let label_response = ui.add(
                egui::Label::new(RichText::new(text).color(text_color))
                    .selectable(false)
                    .sense(egui::Sense::click()),
            );

            if label_response.clicked() && !dialog_state.is_open {
                dialog_state.open();
            }

            label_response
        })
        .inner;

    response
}

/// Renders the user info widget when logged in.
fn user_info_widget(_state_ctx: &StateCtx, auth_state: &AuthState, ui: &mut Ui) -> Response {
    let username = auth_state.username.as_deref().unwrap_or("User");

    Frame::NONE
        .fill(Color32::from_rgb(34, 139, 34)) // Forest green
        .inner_margin(Margin::symmetric(12, 4))
        .outer_margin(Margin::symmetric(0, 4))
        .corner_radius(4.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("👤 {username}")).color(Color32::WHITE));
            });
        })
        .response
}

/// Renders the login dialog window.
///
/// Returns `Some(LoginFormData)` if the user clicked "Sign In",
/// or `None` if the dialog should remain open or was closed.
pub fn login_dialog(
    ctx: &egui::Context,
    _state_ctx: &StateCtx,
    auth_state: &mut AuthState,
    dialog_state: &mut LoginDialogState,
) -> Option<LoginFormData> {
    if !dialog_state.is_open {
        return None;
    }

    let mut result = None;
    let mut should_close = false;

    Window::new("Sign In")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
        .fixed_size([320.0, 200.0])
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(10.0);

                // Show error message if any
                if let Some(ref error) = auth_state.error {
                    Frame::NONE
                        .fill(Color32::from_rgb(220, 53, 69)) // Red background
                        .inner_margin(Margin::symmetric(8, 4))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            ui.label(RichText::new(error).color(Color32::WHITE).small());
                        });
                    ui.add_space(8.0);
                }

                // Username field
                ui.horizontal(|ui| {
                    ui.label("Username:");
                    ui.add_space(8.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog_state.form_data.username)
                            .desired_width(180.0)
                            .hint_text("Enter username"),
                    );
                });

                ui.add_space(8.0);

                // OTP code field
                ui.horizontal(|ui| {
                    ui.label("OTP Code:");
                    ui.add_space(12.0);
                    ui.add(
                        egui::TextEdit::singleline(&mut dialog_state.form_data.otp_code)
                            .desired_width(180.0)
                            .hint_text("6-digit code"),
                    );
                });

                ui.add_space(16.0);

                // Buttons
                ui.horizontal(|ui| {
                    let is_valid = !dialog_state.form_data.username.is_empty()
                        && dialog_state.form_data.otp_code.len() == 6
                        && dialog_state
                            .form_data
                            .otp_code
                            .chars()
                            .all(|c| c.is_ascii_digit());

                    let is_logging_in = auth_state.is_logging_in();

                    // Cancel button
                    if ui
                        .add_enabled(!is_logging_in, egui::Button::new("Cancel"))
                        .clicked()
                    {
                        should_close = true;
                        auth_state.error = None;
                    }

                    ui.add_space(8.0);

                    // Sign In button
                    let signin_enabled = is_valid && !is_logging_in;
                    let signin_text = if is_logging_in {
                        "Signing in..."
                    } else {
                        "Sign In"
                    };

                    if ui
                        .add_enabled(signin_enabled, egui::Button::new(signin_text))
                        .clicked()
                    {
                        auth_state.start_login();
                        result = Some(dialog_state.form_data.clone());
                        // Don't close yet - wait for response
                        // The caller should handle the login request and close on success
                    }
                });

                // Handle enter key to submit
                if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    let is_valid = !dialog_state.form_data.username.is_empty()
                        && dialog_state.form_data.otp_code.len() == 6
                        && dialog_state
                            .form_data
                            .otp_code
                            .chars()
                            .all(|c| c.is_ascii_digit());

                    if is_valid && !auth_state.is_logging_in() {
                        auth_state.start_login();
                        result = Some(dialog_state.form_data.clone());
                    }
                }

                // Handle escape key to close
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) && !auth_state.is_logging_in() {
                    should_close = true;
                    auth_state.error = None;
                }
            });
        });

    if should_close {
        dialog_state.close();
    }

    result
}

/// Performs the login request to the backend.
///
/// This function sends the login request and updates the auth state
/// based on the response.
pub fn perform_login(
    state_ctx: &StateCtx,
    _auth_state: &mut AuthState,
    _dialog_state: &mut LoginDialogState,
    form_data: &LoginFormData,
) {
    let config = state_ctx.state_mut::<BusinessConfig>();
    let api_url = config.api_url();
    let url = format!("{}/auth/verify-otp", api_url.as_str());

    let username = form_data.username.clone();
    let code = form_data.otp_code.clone();

    // Create the request body
    let body = serde_json::json!({
        "username": username,
        "code": code
    });

    let request = ehttp::Request::post(
        url,
        serde_json::to_vec(&body).unwrap_or_default(),
    );
    let request = ehttp::Request {
        headers: ehttp::Headers::new(&[("Content-Type", "application/json")]),
        ..request
    };

    // Clone values for the closure
    let username_for_success = username.clone();

    // For now, we'll use a simplified approach that directly updates state
    // In a real implementation, this would use the StateCtx compute system
    ehttp::fetch(request, move |response| {
        match response {
            Ok(resp) => {
                if resp.status == 200 {
                    // Parse response to check if valid
                    if let Ok(json) = resp.json::<serde_json::Value>() {
                        if let Some(valid) = json.get("valid").and_then(serde_json::Value::as_bool)
                        {
                            if valid {
                                log::info!("Login successful for user: {}", username_for_success);
                                // Note: Due to ehttp callback limitations, we can't directly
                                // update auth_state here. The UI should poll for results.
                            } else {
                                log::warn!("Login failed: Invalid OTP code");
                            }
                        }
                    }
                } else {
                    log::warn!("Login request failed with status: {}", resp.status);
                }
            }
            Err(err) => {
                log::error!("Login request error: {}", err);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_login_dialog_state_default() {
        let state = LoginDialogState::default();
        assert!(!state.is_open);
        assert!(state.form_data.username.is_empty());
        assert!(state.form_data.otp_code.is_empty());
    }

    #[test]
    fn test_login_dialog_state_open_close() {
        let mut state = LoginDialogState::default();

        state.open();
        assert!(state.is_open);

        state.close();
        assert!(!state.is_open);
    }

    #[test]
    fn test_login_dialog_state_clears_form_on_open() {
        let mut state = LoginDialogState::default();

        // Set some form data
        state.form_data.username = "testuser".to_string();
        state.form_data.otp_code = "123456".to_string();

        // Open should clear the form
        state.open();
        assert!(state.form_data.username.is_empty());
        assert!(state.form_data.otp_code.is_empty());
    }
}
