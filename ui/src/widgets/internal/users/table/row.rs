//! Row rendering for the internal users table.

use chrono::{DateTime, Utc};
use collects_business::InternalUserItem;
use collects_states::StateCtx;
use egui::{Color32, RichText, Stroke, Ui};
use egui_extras::TableRow;
use ustr::Ustr;

use super::cells::{
    render_action_buttons, render_avatar_cell, render_id_cell, render_nickname_cell,
    render_otp_code_cell, render_otp_toggle_button, render_time_remaining_cell,
    render_timestamp_cell, render_username_cell,
};
use crate::widgets::internal::users::qr::generate_qr_image;
use collects_business::{
    GetUserQrCommand, InternalUsersActionCompute, InternalUsersActionInput,
    InternalUsersActionKind, InternalUsersActionState, InternalUsersState,
    ResetInternalUsersActionCommand, UserAction,
};

/// Data needed to render a user row.
pub struct UserRowData {
    pub index: usize,
    pub user: InternalUserItem,
    pub is_revealed: bool,
    pub time_remaining: u8,
}

/// Result of rendering a user row.
pub struct UserRowResult {
    pub toggle_otp: bool,
    pub action: Option<UserAction>,
}

/// Renders a single user row with all cells.
///
/// This function renders a complete row including:
/// - ID with border indicator
/// - Avatar (icon or placeholder)
/// - Username
/// - Nickname
/// - OTP code (revealed or hidden)
/// - Time remaining with color coding
/// - OTP toggle button
/// - Created timestamp
/// - Updated timestamp
/// - Action buttons
///
/// If QR code is expanded, also renders the QR inline below the row data.
#[inline]
pub fn render_user_row(row: &mut TableRow<'_, '_>, data: &UserRowData) -> UserRowResult {
    let mut result = UserRowResult {
        toggle_otp: false,
        action: None,
    };

    // ID cell with border indicator
    row.col(|ui| {
        render_id_cell(ui, data.index);
        draw_cell_bottom_border(ui);
    });

    // Avatar cell
    row.col(|ui| {
        render_avatar_cell(ui, data.user.avatar_url.as_deref());
        draw_cell_bottom_border(ui);
    });

    // Username cell
    row.col(|ui| {
        render_username_cell(ui, &data.user.username);
        draw_cell_bottom_border(ui);
    });

    // Nickname cell
    row.col(|ui| {
        render_nickname_cell(ui, data.user.nickname.as_deref());
        draw_cell_bottom_border(ui);
    });

    // OTP code cell
    row.col(|ui| {
        render_otp_code_cell(ui, &data.user.current_otp, data.is_revealed);
        draw_cell_bottom_border(ui);
    });

    // Time remaining cell
    row.col(|ui| {
        render_time_remaining_cell(ui, data.time_remaining);
        draw_cell_bottom_border(ui);
    });

    // OTP toggle button
    row.col(|ui| {
        if render_otp_toggle_button(ui, data.is_revealed) {
            result.toggle_otp = true;
        }
        draw_cell_bottom_border(ui);
    });

    // Created timestamp cell
    row.col(|ui| {
        render_timestamp_cell(ui, &data.user.created_at);
        draw_cell_bottom_border(ui);
    });

    // Updated timestamp cell
    row.col(|ui| {
        render_timestamp_cell(ui, &data.user.updated_at);
        draw_cell_bottom_border(ui);
    });

    // Action buttons
    row.col(|ui| {
        let username_ustr = Ustr::from(&data.user.username);
        result.action = render_action_buttons(ui, username_ustr);
        draw_cell_bottom_border(ui);
    });

    result
}

/// Draws a bottom border line for a cell.
#[inline]
fn draw_cell_bottom_border(ui: &mut Ui) {
    let rect = ui.available_rect_before_wrap();
    let border_color = ui.visuals().widgets.noninteractive.bg_stroke.color;
    ui.painter().hline(
        rect.left()..=rect.right(),
        rect.bottom(),
        Stroke::new(1.0, border_color),
    );
}

/// Renders the QR code expansion below the table.
///
/// This shows the QR code inline instead of in a modal window.
#[inline]
pub fn render_qr_expansion(
    state_ctx: &mut StateCtx,
    state: &mut InternalUsersState,
    api_base_url: &str,
    username: &Ustr,
    ui: &mut Ui,
) {
    let ctx = ui.ctx().clone();

    // Draw expansion frame
    egui::Frame::NONE
        .fill(ui.visuals().extreme_bg_color)
        .inner_margin(egui::Margin::same(12))
        .corner_radius(4.0)
        .stroke(egui::Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .show(ui, |ui| {
            ui.vertical_centered(|ui| {
                ui.label(format!("QR Code for: {}", username));
                ui.add_space(8.0);

                // Prefer the typed action compute for error/loading/data when available.
                if let Some(action_compute) = state_ctx.cached::<InternalUsersActionCompute>() {
                    match action_compute.state() {
                        InternalUsersActionState::Error {
                            kind: InternalUsersActionKind::GetUserQr,
                            user,
                            message,
                        } if *user == *username => {
                            ui.colored_label(Color32::RED, format!("Error: {message}"));
                            ui.add_space(8.0);
                            if ui.button("Close").clicked() {
                                state.close_action();
                                state_ctx.dispatch::<ResetInternalUsersActionCommand>();
                            }
                            return;
                        }
                        InternalUsersActionState::InFlight {
                            kind: InternalUsersActionKind::GetUserQr,
                            user,
                        } if *user == *username => {
                            ui.horizontal(|ui| {
                                ui.spinner();
                                ui.label("Loading QR code...");
                            });
                            return;
                        }
                        InternalUsersActionState::Success {
                            kind: InternalUsersActionKind::GetUserQr,
                            user,
                            data: Some(otpauth_url),
                        } if *user == *username => {
                            // Keep local cached copies in state for texture caching + existing UI flow.
                            if state.qr_code_data.as_deref() != Some(otpauth_url.as_str()) {
                                state.qr_code_data = Some(otpauth_url.clone());
                                state.action_in_progress = false;
                            }
                        }
                        _ => {}
                    }
                } else if let Some(error) = &state.action_error {
                    // Legacy fallback until all callers are migrated.
                    ui.colored_label(Color32::RED, format!("Error: {error}"));
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        state.close_action();
                    }
                    return;
                }

                // Legacy fallback loading state until all callers are migrated.
                if state.action_in_progress {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Loading QR code...");
                    });
                    return;
                }

                if let Some(otpauth_url) = &state.qr_code_data {
                    ui.label("Scan this QR code with Google Authenticator:");
                    ui.add_space(4.0);

                    // Generate QR code texture if not cached
                    if state.qr_texture.is_none()
                        && let Some(qr_image) = generate_qr_image(otpauth_url, 180)
                    {
                        state.qr_texture = Some(ctx.load_texture(
                            "qr_code_inline",
                            qr_image,
                            egui::TextureOptions::NEAREST,
                        ));
                    }

                    // Display QR code with white background
                    egui::Frame::NONE
                        .fill(Color32::WHITE)
                        .inner_margin(egui::Margin::same(6))
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
                        state_ctx.dispatch::<ResetInternalUsersActionCommand>();
                    }
                } else {
                    // Fetch user QR code via business Command + Compute (no egui temp memory plumbing).
                    state.set_action_in_progress();

                    // Provide the base URL via business input state (kept consistent with refresh).
                    state_ctx.update::<InternalUsersActionInput>(|input| {
                        input.api_base_url = Some(Ustr::from(api_base_url));
                        input.username = Some(*username);
                        // Clear unrelated fields defensively.
                        input.new_username = None;
                        input.nickname = None;
                        input.avatar_url = None;
                    });

                    state_ctx.dispatch::<GetUserQrCommand>();
                }
            });
        });
}

/// Prepares user row data from the state.
#[inline]
pub fn prepare_user_row_data(
    index: usize,
    user: &InternalUserItem,
    state: &InternalUsersState,
    now: DateTime<Utc>,
) -> UserRowData {
    let is_revealed = state.is_otp_revealed(&user.username);
    let time_remaining = state.calculate_time_remaining(user.time_remaining, now);

    UserRowData {
        index,
        user: user.clone(),
        is_revealed,
        time_remaining,
    }
}
