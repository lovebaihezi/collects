//! Main panel for internal users management.

use collects_business::{
    CreateUserCommand, CreateUserCompute, CreateUserInput, InternalUserItem,
    InternalUsersListUsersCompute, InternalUsersListUsersInput, InternalUsersListUsersResult,
    RefreshInternalUsersCommand,
};
use collects_states::{StateCtx, Time};
use egui::{Color32, Response, Ui};
use egui_extras::TableBuilder;
use std::any::TypeId;
use ustr::Ustr;

use super::modals::{
    show_create_user_modal, show_delete_user_modal, show_edit_profile_modal,
    show_edit_username_modal, show_revoke_otp_modal,
};
use super::table::columns::{HEADER_HEIGHT, ROW_HEIGHT, table_columns};
use super::table::header::render_table_header;
use super::table::row::{prepare_user_row_data, render_qr_expansion, render_user_row};
use collects_business::{InternalUsersState, UserAction};

/// Displays the internal users panel with a table and create button.
pub fn internal_users_panel(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) -> Response {
    // Check if OTP codes have become stale and auto-refresh if needed
    check_and_auto_refresh_otp(state_ctx, api_base_url);

    let response = ui.vertical(|ui| {
        // Controls row: Refresh and Create buttons
        render_controls_row(state_ctx, api_base_url, ui);

        // Error display (from compute)
        render_error_display(state_ctx, ui);

        ui.add_space(8.0);

        // Render the users table (from compute)
        let (username_to_toggle, action_to_start) = render_users_table(state_ctx, ui);

        // Apply toggle action after table iteration
        if let Some(username) = username_to_toggle {
            state_ctx.update::<InternalUsersState>(|s| s.toggle_otp_visibility(username));
        }

        // Start action if requested
        if let Some(action) = action_to_start {
            state_ctx.update::<InternalUsersState>(|s| s.start_action(action));
        }

        // Render QR code expansion inline (after table) if ShowQrCode action is active
        render_inline_qr_expansion(state_ctx, api_base_url, ui);
    });

    // Create user modal
    render_modals(state_ctx, api_base_url, ui);

    response.response
}

/// Checks if any OTP codes have become stale and triggers auto-refresh.
///
/// OTP codes change every 30 seconds. When the time remaining crosses a 30-second
/// boundary, the cached OTP codes are stale and need to be refreshed from the API.
/// This function automatically triggers a refresh when stale OTP is detected.
#[inline]
fn check_and_auto_refresh_otp(state_ctx: &mut StateCtx, api_base_url: &str) {
    // Don't auto-refresh if already loading
    let is_loading = state_ctx
        .cached::<InternalUsersListUsersCompute>()
        .map(|c| c.is_loading())
        .unwrap_or(false);

    if is_loading {
        return;
    }

    // Get current time
    let now = *state_ctx.state::<Time>().as_ref();

    // Get users from the compute
    let users: Vec<InternalUserItem> = match state_ctx.cached::<InternalUsersListUsersCompute>() {
        Some(c) => match &c.result {
            InternalUsersListUsersResult::Loaded(users) => users.clone(),
            _ => return, // No data loaded yet
        },
        None => return,
    };

    // Check if any user's OTP is stale
    let state = state_ctx.state::<InternalUsersState>();
    let any_stale = users
        .iter()
        .any(|user| state.is_otp_stale(user.time_remaining, now));

    if any_stale {
        // Trigger auto-refresh
        state_ctx.update::<InternalUsersListUsersInput>(|input| {
            input.api_base_url = Some(Ustr::from(api_base_url));
        });
        state_ctx.dispatch::<RefreshInternalUsersCommand>();
    }
}

/// Renders the controls row with Refresh and Create buttons.
#[inline]
fn render_controls_row(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) {
    let should_open_create = ui
        .horizontal(|ui| {
            // Refresh: dispatch business command (no egui memory temp plumbing)
            if ui.button("🔄 Refresh").clicked() {
                // Prefer passing the base URL through the existing function parameter for now.
                // The command will fall back to `BusinessConfig::api_url()` if input is unset.
                state_ctx.update::<collects_business::InternalUsersListUsersInput>(|input| {
                    input.api_base_url = Some(Ustr::from(api_base_url));
                });
                state_ctx.dispatch::<RefreshInternalUsersCommand>();
            }

            let should_open_create = ui.button("➕ Create User").clicked();

            // Loading indicator from compute
            if let Some(compute) = state_ctx.cached::<InternalUsersListUsersCompute>()
                && compute.is_loading()
            {
                ui.spinner();
                ui.label("Loading...");
            }

            should_open_create
        })
        .inner;

    // Handle create modal open
    if should_open_create {
        reset_create_user_compute(state_ctx);
        state_ctx.update::<InternalUsersState>(|s| s.open_create_modal());
    }
}

/// Renders error display if present.
#[inline]
fn render_error_display(state_ctx: &mut StateCtx, ui: &mut Ui) {
    // Error is now sourced from the list-users compute (refresh slice).
    let Some(compute) = state_ctx.cached::<InternalUsersListUsersCompute>() else {
        return;
    };

    if let Some(error) = compute.error_message() {
        ui.colored_label(Color32::RED, format!("Error: {error}"));
    }
}

/// Renders the users table and returns any pending actions.
#[inline]
fn render_users_table(state_ctx: &mut StateCtx, ui: &mut Ui) -> (Option<Ustr>, Option<UserAction>) {
    let mut username_to_toggle: Option<Ustr> = None;
    let mut action_to_start: Option<UserAction> = None;

    // Get current time for calculating real-time OTP time remaining
    let now = *state_ctx.state::<Time>().as_ref();

    // Source users from the list-users compute (refresh slice).
    let users: Vec<InternalUserItem> = match state_ctx.cached::<InternalUsersListUsersCompute>() {
        Some(c) => match &c.result {
            InternalUsersListUsersResult::Loaded(users) => users.clone(),
            _ => Vec::new(),
        },
        None => Vec::new(),
    };

    // Users table using native egui_extras TableBuilder
    let state = state_ctx.state::<InternalUsersState>();

    // Prepare user row data outside the table body closure
    let user_data: Vec<_> = users
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

                // Use fixed row height - QR code is rendered outside the table
                body.row(ROW_HEIGHT, |mut row| {
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
    let current_action = state_ctx
        .state::<InternalUsersState>()
        .current_action
        .clone();

    if let UserAction::ShowQrCode(username) = &current_action {
        let state = state_ctx.state_mut::<InternalUsersState>();
        render_qr_expansion(state_ctx, state, api_base_url, username, ui);
    }
}

/// Renders all modal dialogs.
#[inline]
fn render_modals(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) {
    // Create user modal
    let create_modal_open = state_ctx.state::<InternalUsersState>().create_modal_open;
    if create_modal_open {
        show_create_user_modal(state_ctx, ui);
    }

    // Action modals (except QR code which is now inline)
    let current_action = state_ctx
        .state::<InternalUsersState>()
        .current_action
        .clone();
    match &current_action {
        UserAction::EditUsername(username) => {
            show_edit_username_modal(state_ctx, api_base_url, *username, ui);
        }
        UserAction::EditProfile(username) => {
            show_edit_profile_modal(state_ctx, api_base_url, *username, ui);
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

/// Reset the CreateUserCompute to idle state.
pub(crate) fn reset_create_user_compute(state_ctx: &mut StateCtx) {
    // Clear the input using update() for proper dirty propagation
    state_ctx.update::<CreateUserInput>(|input| {
        input.username = None;
    });
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
