//! Row rendering for the internal users table.

use chrono::{DateTime, Utc};
use collects_business::InternalUserItem;
use egui::{Color32, RichText, Stroke, Ui};
use egui_extras::TableRow;
use ustr::Ustr;

use super::cells::{
    render_action_buttons, render_id_cell, render_otp_code_cell, render_otp_toggle_button,
    render_time_remaining_cell, render_username_cell,
};
use crate::widgets::internal::users::api::fetch_user_qr_code;
use crate::widgets::internal::users::qr::generate_qr_image;
use crate::widgets::internal::users::state::{InternalUsersState, UserAction};

/// Data needed to render a user row.
pub struct UserRowData {
    pub index: usize,
    pub user: InternalUserItem,
    pub is_revealed: bool,
    pub time_remaining: u8,
    pub is_qr_expanded: bool,
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
/// - Username
/// - OTP code (revealed or hidden)
/// - Time remaining with color coding
/// - OTP toggle button
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
        // Draw bottom border for the cell
        draw_cell_bottom_border(ui);
    });

    // Username cell
    row.col(|ui| {
        render_username_cell(ui, &data.user.username);
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

                if let Some(error) = &state.action_error {
                    ui.colored_label(Color32::RED, format!("Error: {error}"));
                    ui.add_space(8.0);
                    if ui.button("Close").clicked() {
                        state.close_action();
                    }
                    return;
                }

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
                    }
                } else {
                    // Fetch user data to get QR code
                    state.set_action_in_progress();
                    fetch_user_qr_code(api_base_url, username.as_str(), ctx);
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
    let username_ustr = Ustr::from(&user.username);
    let is_qr_expanded =
        matches!(&state.current_action, UserAction::ShowQrCode(u) if *u == username_ustr);

    UserRowData {
        index,
        user: user.clone(),
        is_revealed,
        time_remaining,
        is_qr_expanded,
    }
}
