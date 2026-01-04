//! Cell rendering functions for the internal users table.
//!
//! Each function renders a specific type of cell content with
//! centered alignment and appropriate styling.

use chrono::DateTime;
use egui::{Color32, RichText, Ui};
use ustr::Ustr;

use crate::widgets::internal::users::state::UserAction;

/// Renders the ID cell with a border indicator.
///
/// The ID is displayed as a row index with a left border for visual separation.
#[inline]
pub fn render_id_cell(ui: &mut Ui, id: usize) {
    // Draw left border indicator
    let rect = ui.available_rect_before_wrap();
    let border_color = ui.visuals().widgets.noninteractive.bg_stroke.color;
    ui.painter().vline(
        rect.left(),
        rect.top()..=rect.bottom(),
        egui::Stroke::new(2.0, border_color),
    );

    ui.centered_and_justified(|ui| {
        ui.label(RichText::new(format!("{}", id + 1)).monospace());
    });
}

/// Renders the username cell.
#[inline]
pub fn render_username_cell(ui: &mut Ui, username: &str) {
    ui.centered_and_justified(|ui| {
        ui.label(username);
    });
}

/// Renders the nickname cell.
#[inline]
pub fn render_nickname_cell(ui: &mut Ui, nickname: Option<&str>) {
    ui.centered_and_justified(|ui| {
        if let Some(name) = nickname {
            ui.label(name);
        } else {
            ui.label(RichText::new("-").weak());
        }
    });
}

/// Renders the avatar cell (shows avatar icon or placeholder).
#[inline]
pub fn render_avatar_cell(ui: &mut Ui, avatar_url: Option<&str>) {
    ui.centered_and_justified(|ui| {
        if let Some(url) = avatar_url {
            // Show avatar icon indicator when URL is present
            ui.label(RichText::new("🖼️").size(16.0)).on_hover_text(url);
        } else {
            // Show placeholder
            ui.label(RichText::new("👤").size(16.0).weak());
        }
    });
}

/// Renders a timestamp cell (created_at or updated_at).
#[inline]
pub fn render_timestamp_cell(ui: &mut Ui, timestamp: &str) {
    ui.centered_and_justified(|ui| {
        // Parse and format the timestamp to be more readable
        let display = format_timestamp(timestamp);
        ui.label(RichText::new(display).small());
    });
}

/// Formats a timestamp string to a more readable format.
fn format_timestamp(timestamp: &str) -> String {
    // Try to parse RFC3339 format and display more compactly
    if let Ok(dt) = DateTime::parse_from_rfc3339(timestamp) {
        dt.format("%Y-%m-%d %H:%M").to_string()
    } else {
        // If parsing fails, just show the raw timestamp truncated
        if timestamp.len() > 16 {
            timestamp[..16].to_string()
        } else {
            timestamp.to_string()
        }
    }
}

/// Renders the OTP code cell with reveal/hide functionality.
#[inline]
pub fn render_otp_code_cell(ui: &mut Ui, otp_code: &str, is_revealed: bool) {
    ui.centered_and_justified(|ui| {
        if is_revealed {
            ui.label(RichText::new(otp_code).monospace());
        } else {
            ui.label(RichText::new("••••••").monospace());
        }
    });
}

/// Renders the time remaining cell with color coding.
///
/// Colors:
/// - Red: 5 seconds or less (critical)
/// - Orange: 10 seconds or less (warning)
/// - Green: more than 10 seconds (safe)
#[inline]
pub fn render_time_remaining_cell(ui: &mut Ui, time_remaining: u8) {
    let time_color = get_time_color(time_remaining);

    ui.centered_and_justified(|ui| {
        ui.label(
            RichText::new(format!("{time_remaining}s"))
                .monospace()
                .color(time_color),
        );
    });
}

/// Gets the appropriate color for the time remaining value.
#[inline]
fn get_time_color(time_remaining: u8) -> Color32 {
    if time_remaining <= 5 {
        Color32::RED // Critical: 5 seconds or less
    } else if time_remaining <= 10 {
        Color32::from_rgb(255, 165, 0) // Warning: 10 seconds or less
    } else {
        Color32::from_rgb(34, 139, 34) // Safe: more than 10 seconds
    }
}

/// Renders the OTP reveal/hide toggle button.
///
/// Returns `true` if the button was clicked.
#[inline]
pub fn render_otp_toggle_button(ui: &mut Ui, is_revealed: bool) -> bool {
    let button_text = if is_revealed { "Hide" } else { "Reveal" };
    ui.centered_and_justified(|ui| ui.button(button_text).clicked())
        .inner
}

/// Renders the action buttons cell.
///
/// Returns the action to start if any button was clicked.
#[inline]
pub fn render_action_buttons(ui: &mut Ui, username: Ustr) -> Option<UserAction> {
    let mut action = None;

    ui.centered_and_justified(|ui| {
        ui.horizontal(|ui| {
            if ui.button("📱 QR").on_hover_text("Show QR Code").clicked() {
                action = Some(UserAction::ShowQrCode(username));
            }
            if ui.button("✏️").on_hover_text("Edit Username").clicked() {
                action = Some(UserAction::EditUsername(username));
            }
            if ui
                .button("📝")
                .on_hover_text("Edit Nickname/Avatar")
                .clicked()
            {
                action = Some(UserAction::EditProfile(username));
            }
            if ui.button("🔄").on_hover_text("Revoke OTP").clicked() {
                action = Some(UserAction::RevokeOtp(username));
            }
            if ui.button("🗑️").on_hover_text("Delete User").clicked() {
                action = Some(UserAction::DeleteUser(username));
            }
        });
    });

    action
}

#[cfg(test)]
mod cells_test {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn test_format_timestamp_rfc3339() {
        // Test valid RFC3339 format
        let timestamp = "2026-01-04T08:21:57.005Z";
        let result = format_timestamp(timestamp);
        assert_eq!(result, "2026-01-04 08:21");
    }

    #[test]
    fn test_format_timestamp_with_offset() {
        // Test RFC3339 with timezone offset
        let timestamp = "2026-01-04T10:21:57+02:00";
        let result = format_timestamp(timestamp);
        assert_eq!(result, "2026-01-04 10:21");
    }

    #[test]
    fn test_format_timestamp_invalid_truncates() {
        // Test invalid format (should truncate to 16 chars)
        let timestamp = "some invalid timestamp string";
        let result = format_timestamp(timestamp);
        assert_eq!(result, "some invalid tim");
    }

    #[test]
    fn test_format_timestamp_short_invalid() {
        // Test short invalid string (should return as-is)
        let timestamp = "short";
        let result = format_timestamp(timestamp);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_get_time_color_critical() {
        // 5 seconds or less should be red
        assert_eq!(get_time_color(0), Color32::RED);
        assert_eq!(get_time_color(5), Color32::RED);
    }

    #[test]
    fn test_get_time_color_warning() {
        // 6-10 seconds should be orange
        let orange = Color32::from_rgb(255, 165, 0);
        assert_eq!(get_time_color(6), orange);
        assert_eq!(get_time_color(10), orange);
    }

    #[test]
    fn test_get_time_color_safe() {
        // More than 10 seconds should be green
        let green = Color32::from_rgb(34, 139, 34);
        assert_eq!(get_time_color(11), green);
        assert_eq!(get_time_color(30), green);
    }

    #[test]
    fn test_render_nickname_cell_with_value() {
        let mut harness = Harness::new_ui(|ui| {
            render_nickname_cell(ui, Some("TestNickname"));
        });
        harness.run();
        // Verify the cell renders without panicking
    }

    #[test]
    fn test_render_nickname_cell_empty() {
        let mut harness = Harness::new_ui(|ui| {
            render_nickname_cell(ui, None);
        });
        harness.run();
        // Verify the cell renders without panicking (shows "-")
    }

    #[test]
    fn test_render_avatar_cell_with_url() {
        let mut harness = Harness::new_ui(|ui| {
            render_avatar_cell(ui, Some("https://example.com/avatar.png"));
        });
        harness.run();
        // Verify the cell renders without panicking (shows "🖼️")
    }

    #[test]
    fn test_render_avatar_cell_empty() {
        let mut harness = Harness::new_ui(|ui| {
            render_avatar_cell(ui, None);
        });
        harness.run();
        // Verify the cell renders without panicking (shows "👤")
    }

    #[test]
    fn test_render_timestamp_cell() {
        let mut harness = Harness::new_ui(|ui| {
            render_timestamp_cell(ui, "2026-01-04T08:21:57.005Z");
        });
        harness.run();
        // Verify the cell renders without panicking
    }
}
