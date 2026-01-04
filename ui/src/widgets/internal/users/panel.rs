//! Main panel for internal users management.

use crate::utils::colors::{TABLE_BORDER, TABLE_HEADER_BG, TABLE_ROW_STRIPE};
use collects_business::{CreateUserCommand, CreateUserCompute, CreateUserInput, InternalUserItem};
use collects_states::{StateCtx, Time};
use egui::{Color32, Frame, Margin, Response, RichText, ScrollArea, Stroke, Ui};
use std::any::TypeId;
use ustr::Ustr;

use super::api::fetch_users;
use super::modals::{
    show_create_user_modal, show_delete_user_modal, show_edit_username_modal, show_qr_code_modal,
    show_revoke_otp_modal,
};
use super::state::{InternalUsersState, UserAction};

/// Displays the internal users panel with a table and create button.
pub fn internal_users_panel(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) -> Response {
    let response = ui.vertical(|ui| {
        // Controls row: Refresh and Create buttons
        ui.horizontal(|ui| {
            let state = state_ctx.state_mut::<InternalUsersState>();
            if ui.button("🔄 Refresh").clicked() && !state.is_fetching {
                state.set_fetching();
                fetch_users(api_base_url, ui.ctx().clone());
            }

            let should_open_create = ui.button("➕ Create User").clicked();
            if state.is_fetching {
                ui.spinner();
                ui.label("Loading...");
            }
            should_open_create
        });

        let should_open_create = ui.horizontal(|_ui| false).inner;

        // Error display
        let state = state_ctx.state_mut::<InternalUsersState>();
        if let Some(error) = &state.error {
            ui.colored_label(Color32::RED, format!("Error: {error}"));
        }

        ui.add_space(8.0);

        // Collect actions (avoiding borrow issues)
        let mut username_to_toggle: Option<Ustr> = None;
        let mut action_to_start: Option<UserAction> = None;

        // Get current time for calculating real-time OTP time remaining
        let now = *state_ctx.state_mut::<Time>().as_ref();

        // Users table with Typora-like styling
        let state = state_ctx.state_mut::<InternalUsersState>();

        // Table frame with border
        Frame::NONE
            .stroke(Stroke::new(1.0, TABLE_BORDER))
            .show(ui, |ui| {
                ScrollArea::vertical().show(ui, |ui| {
                    egui::Grid::new("users_table")
                        .num_columns(5)
                        .striped(false) // We handle striping manually for Typora style
                        .spacing([0.0, 0.0]) // No extra spacing, padding handled by frames
                        .show(ui, |ui| {
                            // Header row with background
                            show_table_header_cell(ui, "Username");
                            show_table_header_cell(ui, "OTP Code");
                            show_table_header_cell(ui, "Time Left");
                            show_table_header_cell(ui, "OTP");
                            show_table_header_cell(ui, "Actions");
                            ui.end_row();

                            // User rows with alternating stripe colors
                            for (row_idx, user) in state.users.iter().enumerate() {
                                let is_even = row_idx % 2 == 0;
                                let row_bg = if is_even {
                                    Color32::TRANSPARENT
                                } else {
                                    TABLE_ROW_STRIPE
                                };

                                // Username cell
                                show_table_cell(ui, row_bg, |ui| {
                                    ui.label(&user.username);
                                });

                                // OTP code with reveal/hide
                                show_table_cell(ui, row_bg, |ui| {
                                    if state.is_otp_revealed(&user.username) {
                                        ui.label(RichText::new(&user.current_otp).monospace());
                                    } else {
                                        ui.label(RichText::new("••••••").monospace());
                                    }
                                });

                                // Calculate real-time time remaining based on elapsed time since fetch
                                let time_remaining =
                                    state.calculate_time_remaining(user.time_remaining, now);

                                // Time remaining indicator with color coding
                                let time_color = if time_remaining <= 5 {
                                    Color32::RED // Critical: 5 seconds or less
                                } else if time_remaining <= 10 {
                                    Color32::from_rgb(255, 165, 0) // Warning: 10 seconds or less
                                } else {
                                    Color32::from_rgb(34, 139, 34) // Safe: more than 10 seconds
                                };

                                show_table_cell(ui, row_bg, |ui| {
                                    ui.label(
                                        RichText::new(format!("{}s", time_remaining))
                                            .monospace()
                                            .color(time_color),
                                    );
                                });

                                // Reveal/hide button
                                let username_ustr = Ustr::from(&user.username);
                                show_table_cell(ui, row_bg, |ui| {
                                    let button_text = if state.is_otp_revealed(&user.username) {
                                        "Hide"
                                    } else {
                                        "Reveal"
                                    };
                                    if ui.button(button_text).clicked() {
                                        username_to_toggle = Some(username_ustr);
                                    }
                                });

                                // Action buttons
                                show_table_cell(ui, row_bg, |ui| {
                                    ui.horizontal(|ui| {
                                        if ui
                                            .button("📱 QR")
                                            .on_hover_text("Show QR Code")
                                            .clicked()
                                        {
                                            action_to_start =
                                                Some(UserAction::ShowQrCode(username_ustr));
                                        }
                                        if ui.button("✏️").on_hover_text("Edit Username").clicked()
                                        {
                                            action_to_start =
                                                Some(UserAction::EditUsername(username_ustr));
                                        }
                                        if ui.button("🔄").on_hover_text("Revoke OTP").clicked() {
                                            action_to_start =
                                                Some(UserAction::RevokeOtp(username_ustr));
                                        }
                                        if ui.button("🗑️").on_hover_text("Delete User").clicked()
                                        {
                                            action_to_start =
                                                Some(UserAction::DeleteUser(username_ustr));
                                        }
                                    });
                                });

                                ui.end_row();
                            }
                        });
                });
            });

        // Apply toggle action after table iteration
        if let Some(username) = username_to_toggle {
            state.toggle_otp_visibility(username);
        }

        // Start action if requested
        if let Some(action) = action_to_start {
            state.start_action(action);
        }

        // Handle create modal open (after borrowing issues resolved)
        if should_open_create {
            // Reset the compute state when opening modal
            reset_create_user_compute(state_ctx);
            state_ctx
                .state_mut::<InternalUsersState>()
                .open_create_modal();
        }
    });

    // Create user modal
    let state = state_ctx.state_mut::<InternalUsersState>();
    if state.create_modal_open {
        show_create_user_modal(state_ctx, ui);
    }

    // Action modals
    let state = state_ctx.state_mut::<InternalUsersState>();
    match &state.current_action.clone() {
        UserAction::ShowQrCode(username) => {
            show_qr_code_modal(state_ctx, api_base_url, *username, ui);
        }
        UserAction::EditUsername(username) => {
            show_edit_username_modal(state_ctx, api_base_url, *username, ui);
        }
        UserAction::DeleteUser(username) => {
            show_delete_user_modal(state_ctx, api_base_url, *username, ui);
        }
        UserAction::RevokeOtp(username) => {
            show_revoke_otp_modal(state_ctx, api_base_url, *username, ui);
        }
        UserAction::None => {}
    }

    response.response
}

/// Renders a styled header cell for the Typora-like table.
fn show_table_header_cell(ui: &mut Ui, text: &str) {
    Frame::NONE
        .fill(TABLE_HEADER_BG)
        .stroke(Stroke::new(1.0, TABLE_BORDER))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.strong(text);
        });
}

/// Renders a styled data cell for the Typora-like table.
fn show_table_cell<R>(
    ui: &mut Ui,
    bg_color: Color32,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    Frame::NONE
        .fill(bg_color)
        .stroke(Stroke::new(1.0, TABLE_BORDER))
        .inner_margin(Margin::symmetric(12, 8))
        .show(ui, add_contents)
        .inner
}

/// Poll for async responses and update state.
/// Call this in the update loop.
pub fn poll_internal_users_responses(state_ctx: &mut StateCtx, ctx: &egui::Context) {
    // Check for users list response
    if let Some(users) = ctx.memory(|mem| {
        mem.data
            .get_temp::<Vec<InternalUserItem>>(egui::Id::new("internal_users_response"))
    }) {
        // Get current time from Time state for mockability
        let now = *state_ctx.state_mut::<Time>().as_ref();
        state_ctx
            .state_mut::<InternalUsersState>()
            .update_users(users, now);
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
        state_ctx.state_mut::<InternalUsersState>().set_error(error);
        ctx.memory_mut(|mem| {
            mem.data
                .remove::<String>(egui::Id::new("internal_users_error"));
        });
    }

    // Check for action error
    if let Some(error) =
        ctx.memory(|mem| mem.data.get_temp::<String>(egui::Id::new("action_error")))
    {
        state_ctx
            .state_mut::<InternalUsersState>()
            .set_action_error(error);
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
        let state = state_ctx.state_mut::<InternalUsersState>();
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
        state_ctx
            .state_mut::<InternalUsersState>()
            .set_qr_code_data(otpauth_url);
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
        state_ctx
            .state_mut::<InternalUsersState>()
            .set_qr_code_data(otpauth_url);
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
