//! Modal dialogs for user management actions.

use collects_business::internal_users::api as internal_users_api;
use collects_business::{CreateUserCompute, CreateUserResult, InternalUsersState};
use collects_states::StateCtx;
use egui::{Color32, RichText, Ui, Window};
use ustr::Ustr;

use super::qr::generate_qr_image;

/// Shows the edit username modal.
pub fn show_edit_username_modal(
    state_ctx: &mut StateCtx,
    api_base_url: &str,
    username: Ustr,
    ui: &mut Ui,
) {
    let mut open = true;
    let state = state_ctx.state_mut::<InternalUsersState>();

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
                let can_update = !state.edit_username_input.is_empty()
                    && state.edit_username_input != username.as_str();

                if ui
                    .add_enabled(can_update, egui::Button::new("Update"))
                    .clicked()
                {
                    state.set_action_in_progress();

                    let ctx = ui.ctx().clone();
                    let api_base_url = api_base_url.to_string();
                    let old_username = username.to_string();
                    let new_username = state.edit_username_input.clone();

                    let Some(cf_token) = state_ctx.cached::<collects_business::CFTokenCompute>()
                    else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Missing CF token compute".to_string(),
                            );
                        });
                        return;
                    };

                    internal_users_api::update_username(
                        &api_base_url,
                        cf_token,
                        &old_username,
                        &new_username,
                        move |result: collects_business::internal_users::api::ApiResult<
                            collects_business::UpdateUsernameResponse,
                        >| {
                            ctx.request_repaint();
                            match result {
                                Ok(_) => {
                                    ctx.memory_mut(|mem| {
                                        mem.data.insert_temp(
                                            egui::Id::new("action_success"),
                                            "username_updated".to_string(),
                                        );
                                    });
                                }
                                Err(err) => {
                                    ctx.memory_mut(|mem| {
                                        mem.data.insert_temp(
                                            egui::Id::new("action_error"),
                                            err.to_string(),
                                        );
                                    });
                                }
                            }
                        },
                    );
                }

                if ui.button("Cancel").clicked() {
                    state.close_action();
                }
            });
        });

    if !open {
        state_ctx.state_mut::<InternalUsersState>().close_action();
    }
}

/// Shows the edit profile modal (nickname and avatar URL).
pub fn show_edit_profile_modal(
    state_ctx: &mut StateCtx,
    api_base_url: &str,
    username: Ustr,
    ui: &mut Ui,
) {
    let mut open = true;
    let state = state_ctx.state_mut::<InternalUsersState>();

    Window::new(format!("Edit Profile - {}", username))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            if let Some(error) = &state.action_error {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if state.action_in_progress {
                ui.label("Updating profile...");
                ui.spinner();
                return;
            }

            ui.label("Edit the user's profile information:");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Nickname:");
                ui.text_edit_singleline(&mut state.edit_nickname_input);
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Avatar URL:");
                ui.text_edit_singleline(&mut state.edit_avatar_url_input);
            });

            ui.add_space(4.0);
            ui.label(
                RichText::new("Leave fields empty to clear them.")
                    .weak()
                    .small(),
            );

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                if ui.button("Update").clicked() {
                    state.set_action_in_progress();
                    let nickname = if state.edit_nickname_input.is_empty() {
                        None
                    } else {
                        Some(state.edit_nickname_input.clone())
                    };
                    let avatar_url = if state.edit_avatar_url_input.is_empty() {
                        None
                    } else {
                        Some(state.edit_avatar_url_input.clone())
                    };
                    let ctx = ui.ctx().clone();
                    let api_base_url = api_base_url.to_string();
                    let username = username.to_string();

                    let Some(cf_token) = state_ctx.cached::<collects_business::CFTokenCompute>()
                    else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Missing CF token compute".to_string(),
                            );
                        });
                        return;
                    };

                    internal_users_api::update_profile(
                        &api_base_url,
                        cf_token,
                        &username,
                        nickname,
                        avatar_url,
                        move |result: collects_business::internal_users::api::ApiResult<
                            collects_business::UpdateProfileResponse,
                        >| {
                            ctx.request_repaint();
                            match result {
                                Ok(_) => {
                                    ctx.memory_mut(|mem| {
                                        mem.data.insert_temp(
                                            egui::Id::new("action_success"),
                                            "profile_updated".to_string(),
                                        );
                                    });
                                }
                                Err(err) => {
                                    ctx.memory_mut(|mem| {
                                        mem.data.insert_temp(
                                            egui::Id::new("action_error"),
                                            err.to_string(),
                                        );
                                    });
                                }
                            }
                        },
                    );
                }

                if ui.button("Cancel").clicked() {
                    state.close_action();
                }
            });
        });

    if !open {
        state_ctx.state_mut::<InternalUsersState>().close_action();
    }
}

/// Shows the delete user confirmation modal.
pub fn show_delete_user_modal(
    state_ctx: &mut StateCtx,
    api_base_url: &str,
    username: Ustr,
    ui: &mut Ui,
) {
    let mut open = true;
    let state = state_ctx.state_mut::<InternalUsersState>();

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

                    let ctx = ui.ctx().clone();
                    let api_base_url = api_base_url.to_string();
                    let username = username.to_string();

                    let Some(cf_token) = state_ctx.cached::<collects_business::CFTokenCompute>()
                    else {
                        ctx.memory_mut(|mem| {
                            mem.data.insert_temp(
                                egui::Id::new("action_error"),
                                "Missing CF token compute".to_string(),
                            );
                        });
                        return;
                    };

                    internal_users_api::delete_user(
                        &api_base_url,
                        cf_token,
                        &username,
                        move |result: collects_business::internal_users::api::ApiResult<
                            collects_business::DeleteUserResponse,
                        >| {
                            ctx.request_repaint();
                            match result {
                                Ok(delete_response) => {
                                    if delete_response.deleted {
                                        ctx.memory_mut(|mem| {
                                            mem.data.insert_temp(
                                                egui::Id::new("action_success"),
                                                "user_deleted".to_string(),
                                            );
                                        });
                                    } else {
                                        ctx.memory_mut(|mem| {
                                            mem.data.insert_temp(
                                                egui::Id::new("action_error"),
                                                "User not found".to_string(),
                                            );
                                        });
                                    }
                                }
                                Err(err) => {
                                    ctx.memory_mut(|mem| {
                                        mem.data.insert_temp(
                                            egui::Id::new("action_error"),
                                            err.to_string(),
                                        );
                                    });
                                }
                            }
                        },
                    );
                }

                if ui.button("Cancel").clicked() {
                    state.close_action();
                }
            });
        });

    if !open {
        state_ctx.state_mut::<InternalUsersState>().close_action();
    }
}

/// Shows the revoke OTP modal.
pub fn show_revoke_otp_modal(
    state_ctx: &mut StateCtx,
    api_base_url: &str,
    username: Ustr,
    ui: &mut Ui,
) {
    let mut open = true;
    let state = state_ctx.state_mut::<InternalUsersState>();

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
                ui.colored_label(
                    Color32::from_rgb(34, 139, 34),
                    "✓ OTP revoked successfully!",
                );
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

                        let ctx = ui.ctx().clone();
                        let api_base_url = api_base_url.to_string();
                        let username = username.to_string();

                        let Some(cf_token) =
                            state_ctx.cached::<collects_business::CFTokenCompute>()
                        else {
                            ctx.memory_mut(|mem| {
                                mem.data.insert_temp(
                                    egui::Id::new("action_error"),
                                    "Missing CF token compute".to_string(),
                                );
                            });
                            return;
                        };

                        internal_users_api::revoke_otp(
                            &api_base_url,
                            cf_token,
                            &username,
                            move |result: collects_business::internal_users::api::ApiResult<
                                collects_business::RevokeOtpResponse,
                            >| {
                                ctx.request_repaint();
                                match result {
                                    Ok(revoke_response) => {
                                        ctx.memory_mut(|mem| {
                                            mem.data.insert_temp(
                                                egui::Id::new("revoke_otp_response"),
                                                revoke_response.otpauth_url,
                                            );
                                        });
                                    }
                                    Err(err) => {
                                        ctx.memory_mut(|mem| {
                                            mem.data.insert_temp(
                                                egui::Id::new("action_error"),
                                                err.to_string(),
                                            );
                                        });
                                    }
                                }
                            },
                        );
                    }

                    if ui.button("Cancel").clicked() {
                        state.close_action();
                    }
                });
            }
        });

    if !open {
        state_ctx.state_mut::<InternalUsersState>().close_action();
    }
}

/// Shows the create user modal window.
pub fn show_create_user_modal(state_ctx: &mut StateCtx, ui: &mut Ui) {
    let state = state_ctx.state_mut::<InternalUsersState>();
    let mut open = state.create_modal_open;

    // Get the compute result
    let compute_result = state_ctx
        .cached::<CreateUserCompute>()
        .map(|c| c.result.clone())
        .unwrap_or(CreateUserResult::Idle);

    let state = state_ctx.state_mut::<InternalUsersState>();

    Window::new("Create User")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            match &compute_result {
                CreateUserResult::Success(created) => {
                    // Show success with QR code info
                    ui.colored_label(
                        Color32::from_rgb(34, 139, 34),
                        "✓ User created successfully!",
                    );
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

                    // Store create_modal_should_close flag to close modal after UI
                }
                CreateUserResult::Error(err) => {
                    ui.colored_label(Color32::RED, format!("Error: {err}"));
                    ui.add_space(8.0);
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

                    let can_create = !state.new_username.trim().is_empty();

                    ui.horizontal(|ui| {
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

    // Handle close actions after Window rendering
    let should_close_and_reset = !open
        || matches!(
            compute_result,
            CreateUserResult::Success(_) | CreateUserResult::Error(_)
        );
    if should_close_and_reset && ui.input(|i| i.pointer.any_click()) {
        // Check if Close button was clicked by re-checking state
    }

    if !open {
        super::reset_create_user_compute(state_ctx);
        state_ctx
            .state_mut::<InternalUsersState>()
            .close_create_modal();
    }
}
