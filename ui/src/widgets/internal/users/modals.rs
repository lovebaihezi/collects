//! Modal dialogs for user management actions.

use collects_business::{CreateUserCompute, CreateUserResult};
use collects_states::StateCtx;
use egui::{Color32, RichText, Ui, Window};

use super::api::{delete_user, fetch_user_qr_code, revoke_otp, update_username};
use super::qr::generate_qr_image;
use super::state::InternalUsersState;

/// Shows the QR code modal for an existing user.
pub fn show_qr_code_modal(
    state: &mut InternalUsersState,
    api_base_url: &str,
    username: String,
    ui: &mut Ui,
) {
    let mut open = true;

    Window::new(format!("QR Code - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    state.close_action();
                }
                return;
            }

            if state.action_in_progress {
                ui.label("Loading QR code...");
                ui.spinner();
                return;
            }

            if let Some(otpauth_url) = &state.qr_code_data {
                ui.label("Scan this QR code with Google Authenticator:");
                ui.add_space(4.0);

                // Generate QR code texture if not cached
                if state.qr_texture.is_none()
                    && let Some(qr_image) = generate_qr_image(otpauth_url, 200)
                {
                    state.qr_texture = Some(ui.ctx().load_texture(
                        "qr_code_display",
                        qr_image,
                        egui::TextureOptions::NEAREST,
                    ));
                }

                // Display QR code as an image
                egui::Frame::NONE
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        if let Some(texture) = &state.qr_texture {
                            ui.image(texture);
                        } else {
                            ui.label(RichText::new(otpauth_url).monospace().small());
                        }
                    });

                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    state.close_action();
                }
            } else {
                // Fetch user data to get QR code
                state.set_action_in_progress();
                fetch_user_qr_code(api_base_url, &username, ui.ctx().clone());
            }
        });

    if !open {
        state.close_action();
    }
}

/// Shows the edit username modal.
pub fn show_edit_username_modal(
    state: &mut InternalUsersState,
    api_base_url: &str,
    username: String,
    ui: &mut Ui,
) {
    let mut open = true;

    Window::new(format!("Edit Username - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if state.action_in_progress {
                ui.label("Updating username...");
                ui.spinner();
                return;
            }

            ui.label("Enter the new username:");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("New Username:");
                ui.text_edit_singleline(&mut state.edit_username_input);
            });

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                let can_update =
                    !state.edit_username_input.is_empty() && state.edit_username_input != username;

                if ui
                    .add_enabled(can_update, egui::Button::new("Update"))
                    .clicked()
                {
                    state.set_action_in_progress();
                    update_username(
                        api_base_url,
                        &username,
                        &state.edit_username_input,
                        ui.ctx().clone(),
                    );
                }

                if ui.button("Cancel").clicked() {
                    state.close_action();
                }
            });
        });

    if !open {
        state.close_action();
    }
}

/// Shows the delete user confirmation modal.
pub fn show_delete_user_modal(
    state: &mut InternalUsersState,
    api_base_url: &str,
    username: String,
    ui: &mut Ui,
) {
    let mut open = true;

    Window::new(format!("Delete User - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if state.action_in_progress {
                ui.label("Deleting user...");
                ui.spinner();
                return;
            }

            ui.colored_label(Color32::from_rgb(255, 165, 0), "⚠️ Warning");
            ui.add_space(4.0);
            ui.label(format!(
                "Are you sure you want to delete user '{}'?",
                username
            ));
            ui.label("This action cannot be undone.");

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                if ui
                    .button(RichText::new("Delete").color(Color32::RED))
                    .clicked()
                {
                    state.set_action_in_progress();
                    delete_user(api_base_url, &username, ui.ctx().clone());
                }

                if ui.button("Cancel").clicked() {
                    state.close_action();
                }
            });
        });

    if !open {
        state.close_action();
    }
}

/// Shows the revoke OTP modal.
pub fn show_revoke_otp_modal(
    state: &mut InternalUsersState,
    api_base_url: &str,
    username: String,
    ui: &mut Ui,
) {
    let mut open = true;

    Window::new(format!("Revoke OTP - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if state.action_in_progress {
                ui.label("Revoking OTP...");
                ui.spinner();
                return;
            }

            // Check if we have new QR code data (after revoke)
            if let Some(otpauth_url) = &state.qr_code_data {
                ui.colored_label(Color32::from_rgb(34, 139, 34), "✓ OTP revoked successfully!");
                ui.add_space(8.0);
                ui.label("The user must scan this new QR code:");
                ui.add_space(4.0);

                // Generate QR code texture if not cached
                if state.qr_texture.is_none()
                    && let Some(qr_image) = generate_qr_image(otpauth_url, 200)
                {
                    state.qr_texture = Some(ui.ctx().load_texture(
                        "qr_code_revoke",
                        qr_image,
                        egui::TextureOptions::NEAREST,
                    ));
                }

                // Display QR code as an image
                egui::Frame::NONE
                    .fill(Color32::WHITE)
                    .inner_margin(egui::Margin::same(8))
                    .corner_radius(4.0)
                    .show(ui, |ui| {
                        if let Some(texture) = &state.qr_texture {
                            ui.image(texture);
                        } else {
                            ui.label(RichText::new(otpauth_url).monospace().small());
                        }
                    });

                ui.add_space(8.0);
                if ui.button("Close").clicked() {
                    state.close_action();
                }
            } else {
                ui.colored_label(Color32::from_rgb(255, 165, 0), "⚠️ Warning");
                ui.add_space(4.0);
                ui.label(format!(
                    "Are you sure you want to revoke OTP for user '{}'?",
                    username
                ));
                ui.label("The user will need to re-scan a new QR code.");

                ui.add_space(16.0);

                ui.horizontal(|ui| {
                    if ui
                        .button(RichText::new("Revoke").color(Color32::from_rgb(255, 165, 0)))
                        .clicked()
                    {
                        state.set_action_in_progress();
                        revoke_otp(api_base_url, &username, ui.ctx().clone());
                    }

                    if ui.button("Cancel").clicked() {
                        state.close_action();
                    }
                });
            }
        });

    if !open {
        state.close_action();
    }
}

/// Shows the create user modal window.
pub fn show_create_user_modal(
    state: &mut InternalUsersState,
    state_ctx: &mut StateCtx,
    ui: &mut Ui,
) {
    let mut open = state.create_modal_open;

    // Get the compute result
    let compute_result = state_ctx
        .cached::<CreateUserCompute>()
        .map(|c| c.result.clone())
        .unwrap_or(CreateUserResult::Idle);

    Window::new("Create User")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            match &compute_result {
                CreateUserResult::Success(created) => {
                    // Show success with QR code info
                    ui.colored_label(Color32::from_rgb(34, 139, 34), "✓ User created successfully!");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.strong("Username:");
                        ui.label(&created.username);
                    });

                    ui.add_space(8.0);
                    ui.label("Scan this QR code with Google Authenticator:");
                    ui.add_space(4.0);

                    // Generate QR code texture if not cached
                    if state.qr_texture.is_none()
                        && let Some(qr_image) = generate_qr_image(&created.otpauth_url, 200)
                    {
                        state.qr_texture = Some(ui.ctx().load_texture(
                            "qr_code_create",
                            qr_image,
                            egui::TextureOptions::NEAREST,
                        ));
                    }

                    // Display QR code as an image
                    egui::Frame::NONE
                        .fill(Color32::WHITE)
                        .inner_margin(egui::Margin::same(8))
                        .corner_radius(4.0)
                        .show(ui, |ui| {
                            if let Some(texture) = &state.qr_texture {
                                ui.image(texture);
                            } else {
                                ui.label(RichText::new(&created.otpauth_url).monospace().small());
                            }
                        });

                    ui.add_space(8.0);

                    if ui.button("Close").clicked() {
                        state.close_create_modal();
                    }
                }
                CreateUserResult::Error(err) => {
                    ui.colored_label(Color32::RED, format!("Error: {err}"));
                    ui.add_space(8.0);

                    if ui.button("Close").clicked() {
                        state.close_create_modal();
                    }
                }
                CreateUserResult::Pending => {
                    ui.label("Creating user...");
                    ui.spinner();
                }
                CreateUserResult::Idle => {
                    ui.label("Enter the username for the new user:");
                    ui.add_space(8.0);

                    ui.horizontal(|ui| {
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut state.new_username);
                    });

                    ui.add_space(16.0);

                    ui.horizontal(|ui| {
                        let can_create = !state.new_username.trim().is_empty();

                        if ui
                            .add_enabled(can_create, egui::Button::new("Create"))
                            .clicked()
                        {
                            super::trigger_create_user(state_ctx, &state.new_username);
                        }

                        if ui.button("Cancel").clicked() {
                            state.close_create_modal();
                        }
                    });
                }
            }
        });

    if !open {
        super::reset_create_user_compute(state_ctx);
        state.close_create_modal();
    }
}
