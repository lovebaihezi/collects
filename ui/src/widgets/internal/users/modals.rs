//! Modal dialogs for user management actions.

use collects_business::{
    CreateUserCompute, CreateUserResult, DeleteUserCommand, InternalUsersActionCompute,
    InternalUsersActionInput, InternalUsersActionKind, InternalUsersActionState,
    InternalUsersState, RevokeOtpCommand, UpdateProfileCommand, UpdateUsernameCommand,
};
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
            // Local draft (UI-only). Seed once from the selected username.
            let draft_id = egui::Id::new(("internal_users_edit_username_draft", username));
            let mut draft: String = ui
                .ctx()
                .data_mut(|d| d.get_temp::<String>(draft_id))
                .unwrap_or_else(|| username.as_str().to_string());

            // Typed action state from business compute.
            let action_state = state_ctx
                .cached::<InternalUsersActionCompute>()
                .map(|c| c.state.clone())
                .unwrap_or(InternalUsersActionState::Idle);

            let (in_flight, error_msg) = match &action_state {
                InternalUsersActionState::InFlight { kind, user } => (
                    *kind == InternalUsersActionKind::UpdateUsername && *user == username,
                    None,
                ),
                InternalUsersActionState::Error {
                    kind,
                    user,
                    message,
                } => (
                    *kind == InternalUsersActionKind::UpdateUsername && *user == username,
                    Some(message.as_str()),
                ),
                _ => (false, None),
            };

            if let Some(error) = error_msg {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if in_flight {
                ui.label("Updating username...");
                ui.spinner();
                return;
            }

            ui.label("Enter the new username:");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("New Username:");
                ui.text_edit_singleline(&mut draft);
            });

            // Persist draft back into UI temp data (no business mutation per keystroke).
            ui.ctx().data_mut(|d| {
                d.insert_temp(draft_id, draft.clone());
            });

            ui.add_space(16.0);

            ui.horizontal(|ui| {
                let can_update = !draft.is_empty() && draft != username.as_str();

                if ui
                    .add_enabled(can_update, egui::Button::new("Update"))
                    .clicked()
                {
                    // Configure inputs for the business command.
                    state_ctx.update::<InternalUsersActionInput>(|input| {
                        input.api_base_url = Some(Ustr::from(api_base_url));
                        input.username = Some(username);
                        input.new_username = Some(Ustr::from(draft.as_str()));
                        input.nickname = None;
                        input.avatar_url = None;
                    });

                    state_ctx.dispatch::<UpdateUsernameCommand>();
                }

                if ui.button("Cancel").clicked() {
                    // Keep existing workflow state mutation for now (TODO #2).
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
            // Local drafts (UI-only). Seed once from the selected business state snapshot.
            let seed_nickname = state
                .users
                .iter()
                .find(|u| u.username == username.as_str())
                .and_then(|u| u.nickname.clone())
                .unwrap_or_default();
            let seed_avatar_url = state
                .users
                .iter()
                .find(|u| u.username == username.as_str())
                .and_then(|u| u.avatar_url.clone())
                .unwrap_or_default();

            let nickname_id =
                egui::Id::new(("internal_users_edit_profile_nickname_draft", username));
            let avatar_id = egui::Id::new(("internal_users_edit_profile_avatar_draft", username));

            let mut nickname_draft: String = ui
                .ctx()
                .data_mut(|d| d.get_temp::<String>(nickname_id))
                .unwrap_or(seed_nickname);
            let mut avatar_draft: String = ui
                .ctx()
                .data_mut(|d| d.get_temp::<String>(avatar_id))
                .unwrap_or(seed_avatar_url);

            // Typed action state from business compute.
            let action_state = state_ctx
                .cached::<InternalUsersActionCompute>()
                .map(|c| c.state.clone())
                .unwrap_or(InternalUsersActionState::Idle);

            let (in_flight, error_msg) = match &action_state {
                InternalUsersActionState::InFlight { kind, user } => (
                    *kind == InternalUsersActionKind::UpdateProfile && *user == username,
                    None,
                ),
                InternalUsersActionState::Error {
                    kind,
                    user,
                    message,
                } => (
                    *kind == InternalUsersActionKind::UpdateProfile && *user == username,
                    Some(message.as_str()),
                ),
                _ => (false, None),
            };

            if let Some(error) = error_msg {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if in_flight {
                ui.label("Updating profile...");
                ui.spinner();
                return;
            }

            ui.label("Edit the user's profile information:");
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Nickname:");
                ui.text_edit_singleline(&mut nickname_draft);
            });

            ui.add_space(8.0);

            ui.horizontal(|ui| {
                ui.label("Avatar URL:");
                ui.text_edit_singleline(&mut avatar_draft);
            });

            // Persist drafts back into UI temp data (no business mutation per keystroke).
            ui.ctx().data_mut(|d| {
                d.insert_temp(nickname_id, nickname_draft.clone());
                d.insert_temp(avatar_id, avatar_draft.clone());
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
                    let nickname = if nickname_draft.is_empty() {
                        None
                    } else {
                        Some(nickname_draft.clone())
                    };
                    let avatar_url = if avatar_draft.is_empty() {
                        None
                    } else {
                        Some(avatar_draft.clone())
                    };

                    state_ctx.update::<InternalUsersActionInput>(|input| {
                        input.api_base_url = Some(Ustr::from(api_base_url));
                        input.username = Some(username);
                        input.new_username = None;
                        input.nickname = nickname;
                        input.avatar_url = avatar_url;
                    });

                    state_ctx.dispatch::<UpdateProfileCommand>();
                }

                if ui.button("Cancel").clicked() {
                    // Keep existing workflow state mutation for now (TODO #2).
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
            // Typed action state from business compute.
            let action_state = state_ctx
                .cached::<InternalUsersActionCompute>()
                .map(|c| c.state.clone())
                .unwrap_or(InternalUsersActionState::Idle);

            let (in_flight, error_msg) = match &action_state {
                InternalUsersActionState::InFlight { kind, user } => (
                    *kind == InternalUsersActionKind::DeleteUser && *user == username,
                    None,
                ),
                InternalUsersActionState::Error {
                    kind,
                    user,
                    message,
                } => (
                    *kind == InternalUsersActionKind::DeleteUser && *user == username,
                    Some(message.as_str()),
                ),
                _ => (false, None),
            };

            if let Some(error) = error_msg {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if in_flight {
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
                    state_ctx.update::<InternalUsersActionInput>(|input| {
                        input.api_base_url = Some(Ustr::from(api_base_url));
                        input.username = Some(username);
                        input.new_username = None;
                        input.nickname = None;
                        input.avatar_url = None;
                    });

                    state_ctx.dispatch::<DeleteUserCommand>();
                }

                if ui.button("Cancel").clicked() {
                    // Keep existing workflow state mutation for now (TODO #2).
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
            // Typed action state from business compute.
            let action_state = state_ctx
                .cached::<InternalUsersActionCompute>()
                .map(|c| c.state.clone())
                .unwrap_or(InternalUsersActionState::Idle);

            let (in_flight, error_msg, qr_data) = match &action_state {
                InternalUsersActionState::InFlight { kind, user } => (
                    *kind == InternalUsersActionKind::RevokeOtp && *user == username,
                    None,
                    None,
                ),
                InternalUsersActionState::Error {
                    kind,
                    user,
                    message,
                } => (
                    *kind == InternalUsersActionKind::RevokeOtp && *user == username,
                    Some(message.as_str()),
                    None,
                ),
                InternalUsersActionState::Success { kind, user, data } => (
                    false,
                    None,
                    if *kind == InternalUsersActionKind::RevokeOtp && *user == username {
                        data.as_deref()
                    } else {
                        None
                    },
                ),
                _ => (false, None, None),
            };

            if let Some(error) = error_msg {
                ui.colored_label(Color32::RED, format!("Error: {error}"));
                ui.add_space(8.0);
            }

            if in_flight {
                ui.label("Revoking OTP...");
                ui.spinner();
                return;
            }

            // Success state: show QR code returned by action command.
            if let Some(otpauth_url) = qr_data {
                ui.colored_label(
                    Color32::from_rgb(34, 139, 34),
                    "✓ OTP revoked successfully!",
                );
                ui.add_space(8.0);
                ui.label("The user must scan this new QR code:");
                ui.add_space(4.0);

                if state.qr_texture.is_none()
                    && let Some(qr_image) = generate_qr_image(otpauth_url, 200)
                {
                    state.qr_texture = Some(ui.ctx().load_texture(
                        "qr_code_revoke",
                        qr_image,
                        egui::TextureOptions::NEAREST,
                    ));
                }

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
                    // Keep existing workflow state mutation for now (TODO #2).
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
                        state_ctx.update::<InternalUsersActionInput>(|input| {
                            input.api_base_url = Some(Ustr::from(api_base_url));
                            input.username = Some(username);
                            input.new_username = None;
                            input.nickname = None;
                            input.avatar_url = None;
                        });

                        state_ctx.dispatch::<RevokeOtpCommand>();
                    }

                    if ui.button("Cancel").clicked() {
                        // Keep existing workflow state mutation for now (TODO #2).
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
