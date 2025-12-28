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

/// Validates the login form data.
///
/// Returns true if:
/// - Username is not empty
/// - OTP code is exactly 6 digits
fn is_form_valid(form_data: &LoginFormData) -> bool {
    !form_data.username.is_empty()
        && form_data.otp_code.len() == 6
        && form_data.otp_code.chars().all(|c| c.is_ascii_digit())
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

    Frame::NONE
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
        .inner
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
                    let is_valid = is_form_valid(&dialog_state.form_data);
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
                if ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && is_form_valid(&dialog_state.form_data)
                    && !auth_state.is_logging_in()
                {
                    auth_state.start_login();
                    result = Some(dialog_state.form_data.clone());
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

/// Result of a login attempt.
#[derive(Debug, Clone)]
pub enum LoginResult {
    /// Login was successful.
    Success(String),
    /// Login failed with an error message.
    Failed(String),
}

/// Channel for receiving login results from async operations.
pub type LoginResultReceiver = flume::Receiver<LoginResult>;
/// Channel for sending login results from async operations.
pub type LoginResultSender = flume::Sender<LoginResult>;

/// Creates a new login result channel.
pub fn create_login_channel() -> (LoginResultSender, LoginResultReceiver) {
    flume::unbounded()
}

/// Performs the login request to the backend.
///
/// This function sends the login request asynchronously. The result is sent
/// through the provided channel sender, which should be polled by the UI
/// to update the auth state.
///
/// # Arguments
///
/// * `state_ctx` - The state context for accessing configuration
/// * `form_data` - The login form data containing username and OTP code
/// * `result_sender` - Channel sender to communicate the login result
pub fn perform_login(
    state_ctx: &StateCtx,
    form_data: &LoginFormData,
    result_sender: LoginResultSender,
) {
    let config = state_ctx.state_mut::<BusinessConfig>();
    let api_url = config.api_url();
    let url = format!("{}/auth/verify-otp", api_url.as_str());

    let username = form_data.username.clone();
    let code = form_data.otp_code.clone();

    // Create the request body - use expect since serialization of simple JSON should never fail
    let body = serde_json::json!({
        "username": username,
        "code": code
    });

    let body_bytes = serde_json::to_vec(&body).expect("Failed to serialize login request body");

    let request = ehttp::Request::post(url, body_bytes);
    let request = ehttp::Request {
        headers: ehttp::Headers::new(&[("Content-Type", "application/json")]),
        ..request
    };

    // Clone values for the closure
    let username_for_success = username.clone();

    // Send the request and communicate result via channel
    ehttp::fetch(request, move |response| {
        let result = match response {
            Ok(resp) => {
                if resp.status == 200 {
                    // Parse response to check if valid
                    match resp.json::<serde_json::Value>() {
                        Ok(json) => {
                            if let Some(valid) =
                                json.get("valid").and_then(serde_json::Value::as_bool)
                            {
                                if valid {
                                    log::info!(
                                        "Login successful for user: {}",
                                        username_for_success
                                    );
                                    LoginResult::Success(username_for_success)
                                } else {
                                    log::warn!("Login failed: Invalid OTP code");
                                    LoginResult::Failed("Invalid username or OTP code".to_string())
                                }
                            } else {
                                LoginResult::Failed("Invalid server response".to_string())
                            }
                        }
                        Err(e) => {
                            log::error!("Failed to parse login response: {}", e);
                            LoginResult::Failed("Failed to parse server response".to_string())
                        }
                    }
                } else {
                    log::warn!("Login request failed with status: {}", resp.status);
                    LoginResult::Failed(format!("Login failed (status: {})", resp.status))
                }
            }
            Err(err) => {
                log::error!("Login request error: {}", err);
                LoginResult::Failed(format!("Network error: {err}"))
            }
        };

        // Send result through channel - ignore errors if receiver dropped
        let _ = result_sender.send(result);
    });
}

/// Polls for login results and updates the auth state accordingly.
///
/// Call this function in your UI update loop to check for completed login attempts.
///
/// # Returns
///
/// Returns `true` if the login was successful and the dialog should be closed.
pub fn poll_login_result(
    receiver: &LoginResultReceiver,
    auth_state: &mut AuthState,
    dialog_state: &mut LoginDialogState,
) -> bool {
    match receiver.try_recv() {
        Ok(LoginResult::Success(username)) => {
            auth_state.login_success(username);
            dialog_state.close();
            true
        }
        Ok(LoginResult::Failed(error)) => {
            auth_state.login_failed(error);
            false
        }
        Err(flume::TryRecvError::Empty) => false,
        Err(flume::TryRecvError::Disconnected) => {
            // Channel closed unexpectedly - treat as error
            auth_state.login_failed("Login request was interrupted".to_string());
            false
        }
    }
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

    #[test]
    fn test_login_form_data_validation() {
        // Valid form
        let valid_form = LoginFormData {
            username: "testuser".to_string(),
            otp_code: "123456".to_string(),
        };
        assert!(is_form_valid(&valid_form));

        // Empty username
        let empty_username = LoginFormData {
            username: String::new(),
            otp_code: "123456".to_string(),
        };
        assert!(!is_form_valid(&empty_username));

        // Empty OTP
        let empty_otp = LoginFormData {
            username: "testuser".to_string(),
            otp_code: String::new(),
        };
        assert!(!is_form_valid(&empty_otp));

        // Short OTP (5 digits)
        let short_otp = LoginFormData {
            username: "testuser".to_string(),
            otp_code: "12345".to_string(),
        };
        assert!(!is_form_valid(&short_otp));

        // Long OTP (7 digits)
        let long_otp = LoginFormData {
            username: "testuser".to_string(),
            otp_code: "1234567".to_string(),
        };
        assert!(!is_form_valid(&long_otp));

        // Non-numeric OTP
        let alpha_otp = LoginFormData {
            username: "testuser".to_string(),
            otp_code: "abcdef".to_string(),
        };
        assert!(!is_form_valid(&alpha_otp));

        // Mixed OTP
        let mixed_otp = LoginFormData {
            username: "testuser".to_string(),
            otp_code: "123abc".to_string(),
        };
        assert!(!is_form_valid(&mixed_otp));
    }
}

#[cfg(test)]
mod signin_widget_test {
    use kittest::Queryable;

    use crate::test_utils::TestCtx;

    use super::signin_button;

    #[tokio::test]
    async fn test_signin_button_shows_sign_in_when_logged_out() {
        let mut ctx = TestCtx::new(|ui, state| {
            signin_button(
                &state.ctx,
                &state.auth_state,
                &mut state.login_dialog_state,
                ui,
            );
        })
        .await;

        let harness = ctx.harness_mut();
        harness.step();

        assert!(
            harness.query_by_label("Sign In").is_some(),
            "'Sign In' button should exist when logged out"
        );
    }

    #[tokio::test]
    async fn test_signin_button_shows_username_when_logged_in() {
        let mut ctx = TestCtx::new(|ui, state| {
            signin_button(
                &state.ctx,
                &state.auth_state,
                &mut state.login_dialog_state,
                ui,
            );
        })
        .await;

        let harness = ctx.harness_mut();

        // Simulate login
        harness.state_mut().auth_state.login_success("testuser".to_string());
        harness.step();

        assert!(
            harness.query_by_label_contains("testuser").is_some(),
            "Username should be displayed when logged in"
        );
    }

    #[tokio::test]
    async fn test_signin_button_changes_text_when_dialog_open() {
        let mut ctx = TestCtx::new(|ui, state| {
            signin_button(
                &state.ctx,
                &state.auth_state,
                &mut state.login_dialog_state,
                ui,
            );
        })
        .await;

        let harness = ctx.harness_mut();

        // Open the dialog
        harness.state_mut().login_dialog_state.open();
        harness.step();

        assert!(
            harness.query_by_label("Signing In...").is_some(),
            "'Signing In...' should be displayed when dialog is open"
        );
    }
}

#[cfg(test)]
mod signin_integration_test {
    use std::time::Duration;

    use collects_business::{AuthState, LoginFormData};

    use crate::test_utils::TestCtx;
    use crate::widgets::{
        LoginDialogState, LoginResult, create_login_channel, perform_login, poll_login_result,
    };

    #[tokio::test]
    async fn test_login_success_with_valid_credentials() {
        let valid_username = "testuser";
        let valid_otp = "123456";

        let mut ctx = TestCtx::new_with_auth(
            |_ui, _state| {
                // Empty UI - we're testing the login flow directly
            },
            valid_username,
            valid_otp,
        )
        .await;

        let harness = ctx.harness_mut();
        let state = harness.state_mut();

        // Create login channel
        let (sender, receiver) = create_login_channel();

        // Perform login with valid credentials
        let form_data = LoginFormData {
            username: valid_username.to_string(),
            otp_code: valid_otp.to_string(),
        };

        perform_login(&state.ctx, &form_data, sender);

        // Wait for the async request to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Poll for result
        let mut auth_state = AuthState::default();
        auth_state.start_login();
        let mut dialog_state = LoginDialogState::default();
        dialog_state.open();

        let success = poll_login_result(&receiver, &mut auth_state, &mut dialog_state);

        assert!(success, "Login should succeed with valid credentials");
        assert!(auth_state.is_logged_in(), "Auth state should be logged in");
        assert_eq!(
            auth_state.username,
            Some(valid_username.to_string()),
            "Username should be set after login"
        );
        assert!(!dialog_state.is_open, "Dialog should be closed after successful login");
    }

    #[tokio::test]
    async fn test_login_failure_with_invalid_credentials() {
        let valid_username = "testuser";
        let valid_otp = "123456";

        let mut ctx = TestCtx::new_with_auth(
            |_ui, _state| {
                // Empty UI - we're testing the login flow directly
            },
            valid_username,
            valid_otp,
        )
        .await;

        let harness = ctx.harness_mut();
        let state = harness.state_mut();

        // Create login channel
        let (sender, receiver) = create_login_channel();

        // Perform login with invalid credentials
        let form_data = LoginFormData {
            username: "wronguser".to_string(),
            otp_code: "000000".to_string(),
        };

        perform_login(&state.ctx, &form_data, sender);

        // Wait for the async request to complete
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Poll for result
        let mut auth_state = AuthState::default();
        auth_state.start_login();
        let mut dialog_state = LoginDialogState::default();
        dialog_state.open();

        let success = poll_login_result(&receiver, &mut auth_state, &mut dialog_state);

        assert!(!success, "Login should fail with invalid credentials");
        assert!(!auth_state.is_logged_in(), "Auth state should not be logged in");
        assert!(
            auth_state.error.is_some(),
            "Error message should be set after failed login"
        );
        assert!(dialog_state.is_open, "Dialog should remain open after failed login");
    }

    #[tokio::test]
    async fn test_login_result_channel_communication() {
        let (sender, receiver) = create_login_channel();

        // Send a success result
        sender
            .send(LoginResult::Success("channeluser".to_string()))
            .expect("Should send result");

        let mut auth_state = AuthState::default();
        auth_state.start_login();
        let mut dialog_state = LoginDialogState::default();
        dialog_state.open();

        let success = poll_login_result(&receiver, &mut auth_state, &mut dialog_state);

        assert!(success, "Poll should return success");
        assert!(auth_state.is_logged_in(), "Auth state should be logged in");
        assert_eq!(auth_state.username, Some("channeluser".to_string()));
    }

    #[tokio::test]
    async fn test_login_result_channel_failure() {
        let (sender, receiver) = create_login_channel();

        // Send a failure result
        sender
            .send(LoginResult::Failed("Test error message".to_string()))
            .expect("Should send result");

        let mut auth_state = AuthState::default();
        auth_state.start_login();
        let mut dialog_state = LoginDialogState::default();
        dialog_state.open();

        let success = poll_login_result(&receiver, &mut auth_state, &mut dialog_state);

        assert!(!success, "Poll should return failure");
        assert!(!auth_state.is_logged_in(), "Auth state should not be logged in");
        assert_eq!(auth_state.error, Some("Test error message".to_string()));
    }

    #[tokio::test]
    async fn test_poll_login_result_empty_channel() {
        let (_sender, receiver) = create_login_channel();

        let mut auth_state = AuthState::default();
        let mut dialog_state = LoginDialogState::default();

        // Poll empty channel should return false and not modify state
        let success = poll_login_result(&receiver, &mut auth_state, &mut dialog_state);

        assert!(!success, "Poll should return false for empty channel");
        assert!(!auth_state.is_logged_in(), "Auth state should remain unchanged");
    }
}
