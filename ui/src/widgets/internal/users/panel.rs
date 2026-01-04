//! Main panel for internal users management.

use collects_business::{CreateUserCommand, CreateUserCompute, CreateUserInput, InternalUserItem};
use collects_states::{StateCtx, Time};
use egui::{Color32, Response, Ui};
use egui_extras::TableBuilder;
use std::any::TypeId;
use ustr::Ustr;

use super::api::fetch_users;
use super::modals::{
    show_create_user_modal, show_delete_user_modal, show_edit_username_modal, show_revoke_otp_modal,
};
use super::state::{InternalUsersState, UserAction};
use super::table::columns::{HEADER_HEIGHT, QR_ROW_HEIGHT, ROW_HEIGHT, table_columns};
use super::table::header::render_table_header;
use super::table::row::{prepare_user_row_data, render_qr_expansion, render_user_row};

/// Displays the internal users panel with a table and create button.
pub fn internal_users_panel(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) -> Response {
    let response = ui.vertical(|ui| {
        // Controls row: Refresh and Create buttons
        render_controls_row(state_ctx, api_base_url, ui);

        // Error display
        render_error_display(state_ctx, ui);

        ui.add_space(8.0);

        // Render the users table
        let (username_to_toggle, action_to_start) = render_users_table(state_ctx, ui);

        // Apply toggle action after table iteration
        if let Some(username) = username_to_toggle {
            state_ctx
                .state_mut::<InternalUsersState>()
                .toggle_otp_visibility(username);
        }

        // Start action if requested
        if let Some(action) = action_to_start {
            state_ctx
                .state_mut::<InternalUsersState>()
                .start_action(action);
        }

        // Render QR code expansion inline (after table) if ShowQrCode action is active
        render_inline_qr_expansion(state_ctx, api_base_url, ui);
    });

    // Create user modal
    render_modals(state_ctx, api_base_url, ui);

    response.response
}

/// Renders the controls row with Refresh and Create buttons.
#[inline]
fn render_controls_row(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) {
    let should_open_create = ui
        .horizontal(|ui| {
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
        })
        .inner;

    // Handle create modal open
    if should_open_create {
        reset_create_user_compute(state_ctx);
        state_ctx
            .state_mut::<InternalUsersState>()
            .open_create_modal();
    }
}

/// Renders error display if present.
#[inline]
fn render_error_display(state_ctx: &mut StateCtx, ui: &mut Ui) {
    let state = state_ctx.state_mut::<InternalUsersState>();
    if let Some(error) = &state.error {
        ui.colored_label(Color32::RED, format!("Error: {error}"));
    }
}

/// Renders the users table and returns any pending actions.
#[inline]
fn render_users_table(state_ctx: &mut StateCtx, ui: &mut Ui) -> (Option<Ustr>, Option<UserAction>) {
    let mut username_to_toggle: Option<Ustr> = None;
    let mut action_to_start: Option<UserAction> = None;

    // Get current time for calculating real-time OTP time remaining
    let now = *state_ctx.state_mut::<Time>().as_ref();

    // Users table using native egui_extras TableBuilder
    let state = state_ctx.state_mut::<InternalUsersState>();

    // Prepare user row data outside the table body closure
    let user_data: Vec<_> = state
        .users
        .iter()
        .enumerate()
        .map(|(i, user)| prepare_user_row_data(i, user, state, now))
        .collect();

    // Build table with columns
    let columns = table_columns();
    let mut table = TableBuilder::new(ui).striped(true);
    for col in columns {
        table = table.column(col);
    }

    table
        .header(HEADER_HEIGHT, |mut header| {
            render_table_header(&mut header);
        })
        .body(|mut body| {
            for data in &user_data {
                let username_ustr = Ustr::from(&data.user.username);

                // Determine row height - taller if QR is expanded
                let row_height = if data.is_qr_expanded {
                    ROW_HEIGHT + QR_ROW_HEIGHT
                } else {
                    ROW_HEIGHT
                };

                body.row(row_height, |mut row| {
                    let result = render_user_row(&mut row, data);

                    if result.toggle_otp {
                        username_to_toggle = Some(username_ustr);
                    }
                    if result.action.is_some() {
                        action_to_start = result.action;
                    }
                });
            }
        });

    (username_to_toggle, action_to_start)
}

/// Renders the inline QR code expansion if ShowQrCode action is active.
#[inline]
fn render_inline_qr_expansion(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) {
    let state = state_ctx.state_mut::<InternalUsersState>();

    if let UserAction::ShowQrCode(username) = &state.current_action.clone() {
        render_qr_expansion(state, api_base_url, username, ui);
    }
}

/// Renders all modal dialogs.
#[inline]
fn render_modals(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) {
    // Create user modal
    let state = state_ctx.state_mut::<InternalUsersState>();
    if state.create_modal_open {
        show_create_user_modal(state_ctx, ui);
    }

    // Action modals (except QR code which is now inline)
    let state = state_ctx.state_mut::<InternalUsersState>();
    match &state.current_action.clone() {
        UserAction::EditUsername(username) => {
            show_edit_username_modal(state_ctx, api_base_url, *username, ui);
        }
        UserAction::DeleteUser(username) => {
            show_delete_user_modal(state_ctx, api_base_url, *username, ui);
        }
        UserAction::RevokeOtp(username) => {
            show_revoke_otp_modal(state_ctx, api_base_url, *username, ui);
        }
        UserAction::ShowQrCode(_) | UserAction::None => {}
    }
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
