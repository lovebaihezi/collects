//! Main panel for internal users management.

use collects_business::{
    CreateUserCommand, CreateUserCompute, CreateUserInput, InternalUserItem,
    InternalUsersActionCompute, InternalUsersActionKind, InternalUsersActionState,
    InternalUsersListUsersCompute, InternalUsersListUsersInput, InternalUsersListUsersResult,
    RefreshInternalUsersCommand,
};
use collects_states::{StateCtx, Time};
use egui::{Color32, Response, Ui};
use egui_extras::TableBuilder;
use std::any::TypeId;
use std::time::Duration;
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
    // Auto-hide any revealed one-time passcodes whose deadlines have expired (deadline is based on the Time state).
    //
    // TODO(perf): If the user list becomes large, scanning OTP deadlines every frame may become
    // noticeable. Consider a small scheduler state (e.g. next_deadline) so you only scan when the
    // next deadline is reached, or trigger a lightweight recompute when Time crosses that boundary.
    let now = *state_ctx.state::<Time>().as_ref();
    state_ctx.update::<InternalUsersState>(|s| {
        s.auto_hide_expired_otps(now);
    });

    // Fetch once when the panel is first opened:
    // - if we have never loaded the list yet (`cached == None` or result == Idle)
    // - and if we are not already loading
    request_initial_users_refresh_if_needed(state_ctx, api_base_url);

    // Auto-refresh when any OTP has cycled (time_remaining reached 0 and wrapped).
    // This ensures the displayed OTP codes stay fresh across 30-second boundaries.
    request_refresh_if_otp_stale(state_ctx, api_base_url);

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
            let now = *state_ctx.state::<Time>().as_ref();
            state_ctx.update::<InternalUsersState>(|s| s.toggle_otp_visibility_at(username, now));
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

    // Request continuous repaints when OTPs are revealed (for live countdown) or when
    // an OTP fetch is in-flight. This ensures the time remaining display updates in real-time.
    //
    // NOTE: This must run AFTER the toggle action is applied (above) so that on the frame
    // where the user clicks "Reveal", we correctly detect the newly-revealed state and
    // schedule the next repaint.
    let has_revealed_otps = !state_ctx
        .state::<InternalUsersState>()
        .revealed_otps
        .is_empty()
        && state_ctx
            .state::<InternalUsersState>()
            .revealed_otps
            .values()
            .any(|&revealed| revealed);

    let otp_fetch_in_flight = state_ctx
        .cached::<InternalUsersActionCompute>()
        .is_some_and(|c| {
            matches!(
                c.state(),
                InternalUsersActionState::InFlight {
                    kind: InternalUsersActionKind::GetUserOtp,
                    ..
                }
            )
        });

    if has_revealed_otps || otp_fetch_in_flight {
        // Request repaint after a short delay to update the countdown timer.
        // Using 100ms provides smooth updates without excessive CPU usage.
        ui.ctx().request_repaint_after(Duration::from_millis(100));
    }

    response.response
}

/// Enqueue an initial refresh the first time this panel is shown.
///
/// This avoids auto-refreshing based on OTP-cycle staleness (OTP codes are unstable) and instead
/// loads the table once on entry. Commands should be flushed end-of-frame by the app loop.
#[inline]
fn request_initial_users_refresh_if_needed(state_ctx: &mut StateCtx, api_base_url: &str) {
    let Some(compute) = state_ctx.cached::<InternalUsersListUsersCompute>() else {
        // Compute not registered yet; nothing to do here.
        return;
    };

    // Don't enqueue if already loading.
    if compute.is_loading() {
        return;
    }

    // Only fetch on first open / not-yet-loaded state.
    if !matches!(compute.result, InternalUsersListUsersResult::Idle) {
        return;
    }

    state_ctx.update::<InternalUsersListUsersInput>(|input| {
        input.api_base_url = Some(Ustr::from(api_base_url));
    });

    // Enqueue only; do NOT flush mid-frame from within widget code.
    state_ctx.enqueue_command::<RefreshInternalUsersCommand>();
}

/// Auto-refresh when any revealed OTP has become stale (crossed a 30-second cycle boundary).
///
/// This ensures users see fresh OTP codes after the countdown reaches 0.
/// Only triggers refresh if:
/// - At least one OTP is currently revealed
/// - That OTP's cycle has elapsed (is_otp_stale returns true)
/// - No refresh is already in progress
#[inline]
fn request_refresh_if_otp_stale(state_ctx: &mut StateCtx, api_base_url: &str) {
    let Some(compute) = state_ctx.cached::<InternalUsersListUsersCompute>() else {
        return;
    };

    // Don't enqueue if already loading.
    if compute.is_loading() {
        return;
    }

    // Get the loaded users
    let users: Vec<InternalUserItem> = match &compute.result {
        InternalUsersListUsersResult::Loaded(users) => users.clone(),
        _ => return,
    };

    let now = *state_ctx.state::<Time>().as_ref();
    let state = state_ctx.state::<InternalUsersState>();

    // Check if any revealed OTP has become stale
    let any_stale = users.iter().any(|user| {
        let username = Ustr::from(&user.username);
        let is_revealed = state.is_otp_revealed_at(&username, now);
        is_revealed && state.is_otp_stale(user.time_remaining, now)
    });

    if !any_stale {
        return;
    }

    state_ctx.update::<InternalUsersListUsersInput>(|input| {
        input.api_base_url = Some(Ustr::from(api_base_url));
    });

    // Enqueue only; flush at end-of-frame in the app loop.
    state_ctx.enqueue_command::<RefreshInternalUsersCommand>();
}

/// Renders the controls row with Refresh and Create buttons.
#[inline]
fn render_controls_row(state_ctx: &mut StateCtx, api_base_url: &str, ui: &mut Ui) {
    let should_open_create = ui
        .horizontal(|ui| {
            // Refresh: enqueue business command (no egui memory temp plumbing)
            if ui.button("🔄 Refresh").clicked() {
                // Prefer passing the base URL through the existing function parameter for now.
                // The command will fall back to `BusinessConfig::api_url()` if input is unset.
                state_ctx.update::<collects_business::InternalUsersListUsersInput>(|input| {
                    input.api_base_url = Some(Ustr::from(api_base_url));
                });
                // Enqueue only; flush at end-of-frame in the app loop.
                state_ctx.enqueue_command::<RefreshInternalUsersCommand>();
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
                    let result = render_user_row(state_ctx, &mut row, data);

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

    // Enqueue only; flush at end-of-frame in the app loop.
    state_ctx.enqueue_command::<CreateUserCommand>();
}
