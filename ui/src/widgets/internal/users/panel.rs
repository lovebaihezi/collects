//! Main panel for internal users management.

use collects_business::{CreateUserCommand, CreateUserCompute, CreateUserInput, InternalUserItem};
use collects_states::{StateCtx, Time};
use egui::{Color32, Response, RichText, ScrollArea, Ui};
use std::any::TypeId;

use super::api::fetch_users;
use super::modals::{
    show_create_user_modal, show_delete_user_modal, show_edit_username_modal, show_qr_code_modal,
    show_revoke_otp_modal,
};
use super::state::{InternalUsersState, UserAction};

/// Displays the internal users panel with a table and create button.
pub fn internal_users_panel(
    state: &mut InternalUsersState,
    state_ctx: &mut StateCtx,
    api_base_url: &str,
    ui: &mut Ui,
) -> Response {
    let response = ui.vertical(|ui| {
        ui.heading("Internal Users");
        ui.separator();

        // Controls row: Refresh and Create buttons
        ui.horizontal(|ui| {
            if ui.button("🔄 Refresh").clicked() && !state.is_fetching {
                state.set_fetching();
                fetch_users(api_base_url, ui.ctx().clone());
            }

            if ui.button("➕ Create User").clicked() {
                // Reset the compute state when opening modal
                super::reset_create_user_compute(state_ctx);
                state.open_create_modal();
            }

            if state.is_fetching {
                ui.spinner();
                ui.label("Loading...");
            }
        });

        // Error display
        if let Some(error) = &state.error {
            ui.colored_label(Color32::RED, format!("Error: {error}"));
        }

        ui.add_space(8.0);

        // Collect actions (avoiding borrow issues)
        let mut username_to_toggle: Option<String> = None;
        let mut action_to_start: Option<UserAction> = None;

        // Users table
        ScrollArea::vertical().show(ui, |ui| {
            egui::Grid::new("users_table")
                .num_columns(5)
                .striped(true)
                .spacing([20.0, 8.0])
                .show(ui, |ui| {
                    // Header row
                    ui.strong("Username");
                    ui.strong("OTP Code");
                    ui.strong("Time Left");
                    ui.strong("OTP");
                    ui.strong("Actions");
                    ui.end_row();

                    // User rows
                    for user in &state.users {
                        ui.label(&user.username);

                        // OTP code with reveal/hide
                        if state.is_otp_revealed(&user.username) {
                            ui.label(RichText::new(&user.current_otp).monospace());
                        } else {
                            ui.label(RichText::new("••••••").monospace());
                        }

                        // Time remaining indicator with color coding
                        let time_color = if user.time_remaining <= 5 {
                            Color32::RED // Critical: 5 seconds or less
                        } else if user.time_remaining <= 10 {
                            Color32::from_rgb(255, 165, 0) // Warning: 10 seconds or less
                        } else {
                            Color32::from_rgb(34, 139, 34) // Safe: more than 10 seconds
                        };
                        ui.label(
                            RichText::new(format!("{}s", user.time_remaining))
                                .monospace()
                                .color(time_color),
                        );

                        // Reveal/hide button
                        let button_text = if state.is_otp_revealed(&user.username) {
                            "Hide"
                        } else {
                            "Reveal"
                        };
                        if ui.button(button_text).clicked() {
                            username_to_toggle = Some(user.username.clone());
                        }

                        // Action buttons
                        ui.horizontal(|ui| {
                            if ui.button("📱 QR").on_hover_text("Show QR Code").clicked() {
                                action_to_start =
                                    Some(UserAction::ShowQrCode(user.username.clone()));
                            }
                            if ui.button("✏️").on_hover_text("Edit Username").clicked() {
                                action_to_start =
                                    Some(UserAction::EditUsername(user.username.clone()));
                            }
                            if ui.button("🔄").on_hover_text("Revoke OTP").clicked() {
                                action_to_start =
                                    Some(UserAction::RevokeOtp(user.username.clone()));
                            }
                            if ui.button("🗑️").on_hover_text("Delete User").clicked() {
                                action_to_start =
                                    Some(UserAction::DeleteUser(user.username.clone()));
                            }
                        });

                        ui.end_row();
                    }
                });
        });

        // Apply toggle action after table iteration
        if let Some(username) = username_to_toggle {
            state.toggle_otp_visibility(&username);
        }

        // Start action if requested
        if let Some(action) = action_to_start {
            state.start_action(action);
        }
    });

    // Create user modal
    if state.create_modal_open {
        show_create_user_modal(state, state_ctx, ui);
    }

    // Action modals
    match &state.current_action {
        UserAction::ShowQrCode(username) => {
            show_qr_code_modal(state, api_base_url, username.clone(), ui);
        }
        UserAction::EditUsername(username) => {
            show_edit_username_modal(state, api_base_url, username.clone(), ui);
        }
        UserAction::DeleteUser(username) => {
            show_delete_user_modal(state, api_base_url, username.clone(), ui);
        }
        UserAction::RevokeOtp(username) => {
            show_revoke_otp_modal(state, api_base_url, username.clone(), ui);
        }
        UserAction::None => {}
    }

    response.response
}

/// Poll for async responses and update state.
/// Call this in the update loop.
pub fn poll_internal_users_responses(
    state: &mut InternalUsersState,
    state_ctx: &StateCtx,
    ctx: &egui::Context,
) {
    // Check for users list response
    if let Some(users) = ctx.memory(|mem| {
        mem.data
            .get_temp::<Vec<InternalUserItem>>(egui::Id::new("internal_users_response"))
    }) {
        // Get current time from Time state for mockability
        let now = *state_ctx.state_mut::<Time>().as_ref();
        state.update_users(users, now);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<Vec<InternalUserItem>>(egui::Id::new("internal_users_response"));
        });
    }

    // Check for users list error
    if let Some(error) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("internal_users_error"))
    }) {
        state.set_error(error);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("internal_users_error"));
        });
    }

    // Check for action error
    if let Some(error) =
        ctx.memory(|mem| mem.data.get_temp::<String>(egui::Id::new("action_error")))
    {
        state.set_action_error(error);
        ctx.memory_mut(|mem| {
            mem.data.remove::<String>(egui::Id::new("action_error"));
        });
    }

    // Check for action success (triggers refresh)
    if let Some(action) =
        ctx.memory(|mem| mem.data.get_temp::<String>(egui::Id::new("action_success")))
    {
        ctx.memory_mut(|mem| {
            mem.data.remove::<String>(egui::Id::new("action_success"));
        });
        // Close action modal and mark for refresh
        state.close_action();
        if action == "user_deleted" || action == "username_updated" {
            // Mark as needing fetch - the actual fetch will happen on next panel render
            // when internal_users_panel() is called with api_base_url
            state.set_fetching();
        }
    }

    // Check for QR code response
    if let Some(otpauth_url) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("user_qr_code_response"))
    }) {
        state.set_qr_code_data(otpauth_url);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("user_qr_code_response"));
        });
    }

    // Check for revoke OTP response
    if let Some(otpauth_url) = ctx.memory(|mem| {
        mem.data
            .get_temp::<String>(egui::Id::new("revoke_otp_response"))
    }) {
        state.set_qr_code_data(otpauth_url);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("revoke_otp_response"));
        });
    }
}

/// Reset the CreateUserCompute to idle state.
pub(crate) fn reset_create_user_compute(state_ctx: &mut StateCtx) {
    // Clear the input
    let input = state_ctx.state_mut::<CreateUserInput>();
    input.username = None;
    // Mark compute as clean so it doesn't auto-run
    state_ctx.mark_clean(&TypeId::of::<CreateUserCompute>());
}

/// Trigger the create-user side effect by setting input and dispatching the command.
///
/// The command will update `CreateUserCompute` via `Updater`, and the normal
/// `StateCtx::sync_computes()` path will apply the result.
pub(crate) fn trigger_create_user(state_ctx: &mut StateCtx, username: &str) {
    // Update command input state
    state_ctx.update::<CreateUserInput>(|input| {
        input.username = Some(username.to_string());
    });

    // Explicitly dispatch the command (manual-only; never runs implicitly)
    state_ctx.dispatch::<CreateUserCommand>();
}
