//! Image paste and drag-and-drop diagnostic widget.
//!
//! This module provides a diagnostic window for debugging image paste and
//! drag-and-drop operations across different environments and platforms.
//!
//! # Usage
//!
//! Toggle the diagnostic window with F2 key. The window displays:
//! - Key event detection (Ctrl+V / Cmd+V hotkey detection)
//! - Clipboard access logs (success/failure with detailed error info)
//! - Drop event logs (hover, drop, file info)
//! - Platform and environment information
//! - Statistics (total events, success rate)

use collects_business::{
    ClipboardAccessResult, DiagLogEntry, DiagLogType, DropHoverEvent, DropResult, ImageDiagState,
    KeyEventType, PasteResult,
};
use collects_states::StateCtx;
use egui::{Color32, RichText, ScrollArea, Ui};
use std::collections::HashSet;

/// Actions that can be triggered by the image diagnostics widget
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageDiagAction {
    /// No action
    #[default]
    None,
    /// Clear the diagnostic history
    ClearHistory,
}

/// Renders the image diagnostic window content.
///
/// This widget displays diagnostic information about paste and drop operations,
/// including recent history, success/failure details, and platform information.
///
/// Returns an action if the user requests one (e.g., clear history).
pub fn image_diag_window(state_ctx: &StateCtx, ui: &mut Ui) -> ImageDiagAction {
    let state = state_ctx.state::<ImageDiagState>();

    ui.heading("Image Paste/Drop Diagnostics");
    ui.add_space(4.0);
    ui.label(
        RichText::new("Press F2 to close • Ctrl+V to test paste • Drag files to test drop")
            .small()
            .weak(),
    );
    ui.separator();

    // Platform and environment info
    ui.horizontal(|ui| {
        ui.label("Platform:");
        ui.label(RichText::new(ImageDiagState::platform_info()).strong());
    });

    // Show Linux display server info on Linux
    #[cfg(target_os = "linux")]
    ui.horizontal(|ui| {
        ui.label("Display Server:");
        ui.label(RichText::new(ImageDiagState::linux_display_server_info()).strong());
    });

    ui.horizontal(|ui| {
        ui.label("Environment:");
        ui.label(
            RichText::new(ImageDiagState::env_info())
                .strong()
                .color(env_color(ImageDiagState::env_info())),
        );
    });

    // Current hover state indicator
    if state.is_hovering() {
        ui.horizontal(|ui| {
            ui.label(
                RichText::new("⬇ Files hovering...")
                    .color(Color32::from_rgb(100, 200, 255))
                    .strong(),
            );
        });
    }

    ui.separator();

    // Statistics summary
    ui.collapsing("📊 Statistics", |ui| {
        ui.horizontal(|ui| {
            ui.label("Key events detected:");
            ui.label(
                RichText::new(format!("{}", state.total_key_events()))
                    .strong()
                    .color(Color32::LIGHT_BLUE),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Paste attempts:");
            let success_text = format!(
                "{} ({} successful)",
                state.total_paste_attempts(),
                state.total_paste_successes()
            );
            let color = if state.total_paste_successes() > 0 {
                Color32::GREEN
            } else if state.total_paste_attempts() > 0 {
                Color32::YELLOW
            } else {
                Color32::GRAY
            };
            ui.label(RichText::new(success_text).color(color));
        });

        ui.horizontal(|ui| {
            ui.label("Drop attempts:");
            let success_text = format!(
                "{} ({} successful)",
                state.total_drop_attempts(),
                state.total_drop_successes()
            );
            let color = if state.total_drop_successes() > 0 {
                Color32::GREEN
            } else if state.total_drop_attempts() > 0 {
                Color32::YELLOW
            } else {
                Color32::GRAY
            };
            ui.label(RichText::new(success_text).color(color));
        });

        if state.total_paste_attempts() > 0 {
            let success_rate = (state.total_paste_successes() as f64
                / state.total_paste_attempts() as f64)
                * 100.0;
            ui.horizontal(|ui| {
                ui.label("Paste success rate:");
                ui.label(format!("{:.1}%", success_rate));
            });
        }

        if state.total_drop_attempts() > 0 {
            let success_rate =
                (state.total_drop_successes() as f64 / state.total_drop_attempts() as f64) * 100.0;
            ui.horizontal(|ui| {
                ui.label("Drop success rate:");
                ui.label(format!("{:.1}%", success_rate));
            });
        }
    });

    ui.separator();

    // Unified event log
    ui.label(RichText::new("📋 Event Log (newest first)").strong());

    let log_entries: Vec<_> = state.log_entries().collect();
    if log_entries.is_empty() {
        ui.label(
            RichText::new("No events recorded yet. Try pressing Ctrl+V or drag a file here.")
                .italics()
                .weak(),
        );
    } else {
        ScrollArea::vertical()
            .max_height(300.0)
            .id_salt("event_log")
            .show(ui, |ui| {
                for entry in log_entries {
                    render_log_entry(ui, entry);
                }
            });
    }

    ui.separator();

    // Clear history button - returns action for caller to handle
    let action = if ui.button("🗑 Clear Log").clicked() {
        ImageDiagAction::ClearHistory
    } else {
        ImageDiagAction::None
    };

    ui.separator();

    // Raw input events debug section
    ui.collapsing("🔍 Raw Input Events (Live)", |ui| {
        render_raw_input_events(ui);
    });

    ui.separator();

    // egui debug tools info
    ui.collapsing("ℹ️ Debug Tips", |ui| {
        ui.label("Enable trace logging for detailed diagnostics:");
        ui.label(
            RichText::new("RUST_LOG=collects_ui::paste=trace,collects_ui::drop=trace")
                .monospace()
                .small(),
        );
        ui.add_space(4.0);
        ui.label("Check clipboard contents manually (Linux):");
        ui.label(
            RichText::new("xclip -selection clipboard -o  # X11")
                .monospace()
                .small(),
        );
        ui.label(
            RichText::new("wl-paste                       # Wayland")
                .monospace()
                .small(),
        );
        ui.add_space(4.0);
        ui.label("Common issues:");
        ui.label("• File managers often copy file:// URIs, not image bytes");
        ui.label("• Wayland: Ctrl+V triggers on key release, not press");
        ui.label("• Some apps require explicit 'Copy Image' vs 'Copy'");
    });

    action
}

/// Render live raw input events from egui context
fn render_raw_input_events(ui: &mut Ui) {
    // Extract data from input state first to avoid borrow conflicts
    let (hovered_files_info, dropped_files_info, key_events) = ui.ctx().input(|i| {
        // Collect hovered files info
        let hovered: Vec<(String, String)> = i
            .raw
            .hovered_files
            .iter()
            .map(|f| {
                let path_str = f
                    .path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(no path)".to_string());
                (path_str, f.mime.clone())
            })
            .collect();

        // Collect dropped files info
        let dropped: Vec<(String, String, bool)> = i
            .raw
            .dropped_files
            .iter()
            .map(|f| {
                let path_str = f
                    .path
                    .as_ref()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "(no path)".to_string());
                (f.name.clone(), path_str, f.bytes.is_some())
            })
            .collect();

        // Collect key events
        let relevant_keys: HashSet<egui::Key> = [egui::Key::V, egui::Key::F2].into_iter().collect();
        let keys: Vec<_> = i
            .events
            .iter()
            .filter_map(|e| {
                if let egui::Event::Key {
                    key,
                    modifiers,
                    pressed,
                    ..
                } = e
                {
                    if relevant_keys.contains(key) || modifiers.ctrl || modifiers.command {
                        Some((*key, *modifiers, *pressed))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        (hovered, dropped, keys)
    });

    // Now render with the extracted data
    if !hovered_files_info.is_empty() {
        ui.label(
            RichText::new(format!("⬇ Hovering {} file(s):", hovered_files_info.len()))
                .color(Color32::from_rgb(100, 200, 255)),
        );
        for (idx, (path_str, mime)) in hovered_files_info.iter().enumerate() {
            ui.label(
                RichText::new(format!("  [{}] path={} mime={}", idx, path_str, mime))
                    .small()
                    .monospace(),
            );
        }
    } else {
        ui.label(RichText::new("No files hovering").weak().small());
    }

    if !dropped_files_info.is_empty() {
        ui.label(
            RichText::new(format!(
                "📥 Dropped {} file(s) this frame:",
                dropped_files_info.len()
            ))
            .color(Color32::GREEN),
        );
        for (idx, (name, path_str, has_bytes)) in dropped_files_info.iter().enumerate() {
            ui.label(
                RichText::new(format!(
                    "  [{}] name={} path={} has_bytes={}",
                    idx, name, path_str, has_bytes
                ))
                .small()
                .monospace(),
            );
        }
    }

    if !key_events.is_empty() {
        ui.label(RichText::new("⌨ Key events this frame:").color(Color32::LIGHT_BLUE));
        for (key, mods, pressed) in key_events {
            let state = if pressed { "pressed" } else { "released" };
            let mod_str = format_modifiers(&mods);
            ui.label(
                RichText::new(format!("  {:?} {} {}", key, mod_str, state))
                    .small()
                    .monospace(),
            );
        }
    }
}

/// Format modifier keys for display
fn format_modifiers(mods: &egui::Modifiers) -> String {
    let mut parts = Vec::new();
    if mods.ctrl {
        parts.push("Ctrl");
    }
    if mods.command {
        parts.push("Cmd");
    }
    if mods.alt {
        parts.push("Alt");
    }
    if mods.shift {
        parts.push("Shift");
    }
    if parts.is_empty() {
        String::new()
    } else {
        format!("[{}]", parts.join("+"))
    }
}

/// Render a single log entry
fn render_log_entry(ui: &mut Ui, entry: &DiagLogEntry) {
    let timestamp = entry.timestamp.format("%H:%M:%S%.3f").to_string();

    ui.horizontal_wrapped(|ui| {
        ui.label(RichText::new(&timestamp).small().weak().monospace());
        ui.add_space(4.0);

        match &entry.entry {
            DiagLogType::KeyEvent(key_event) => {
                render_key_event(ui, key_event);
            }
            DiagLogType::ClipboardAccess(result) => {
                render_clipboard_access(ui, result);
            }
            DiagLogType::PasteResult(result) => {
                render_paste_result(ui, result);
            }
            DiagLogType::DropHoverStart(event) => {
                render_drop_hover_start(ui, event);
            }
            DiagLogType::DropHoverEnd => {
                ui.label(RichText::new("⬆ Hover ended").color(Color32::GRAY));
            }
            DiagLogType::DropResult(result) => {
                render_drop_result(ui, result);
            }
            DiagLogType::Info(msg) => {
                ui.label(RichText::new(format!("ℹ {}", msg)).color(Color32::LIGHT_BLUE));
            }
            DiagLogType::Warning(msg) => {
                ui.label(RichText::new(format!("⚠ {}", msg)).color(Color32::YELLOW));
            }
            DiagLogType::Error(msg) => {
                ui.label(RichText::new(format!("❌ {}", msg)).color(Color32::RED));
            }
        }
    });
    ui.add_space(2.0);
}

/// Render a key event
fn render_key_event(ui: &mut Ui, event: &KeyEventType) {
    let (icon, text, color) = match event {
        KeyEventType::CtrlV => (
            "⌨",
            "Ctrl+V detected".to_string(),
            Color32::from_rgb(100, 200, 255),
        ),
        KeyEventType::CmdV => (
            "⌨",
            "Cmd+V detected".to_string(),
            Color32::from_rgb(100, 200, 255),
        ),
        KeyEventType::Press { key, modifiers } => (
            "⌨",
            format!("Key press: {}+{}", modifiers, key),
            Color32::LIGHT_GRAY,
        ),
        KeyEventType::Release { key, modifiers } => (
            "⌨",
            format!("Key release: {}+{}", modifiers, key),
            Color32::GRAY,
        ),
    };
    ui.label(RichText::new(format!("{} {}", icon, text)).color(color));
}

/// Render clipboard access result
fn render_clipboard_access(ui: &mut Ui, result: &ClipboardAccessResult) {
    match result {
        ClipboardAccessResult::ImageFound {
            width,
            height,
            bytes_len,
            format,
        } => {
            ui.label(RichText::new("📋 Clipboard:").color(Color32::GREEN));
            ui.label(format!(
                "Image {}x{} ({}, {})",
                width,
                height,
                format,
                format_bytes(*bytes_len)
            ));
        }
        ClipboardAccessResult::NoImageContent => {
            ui.label(RichText::new("📋 Clipboard: No image content").color(Color32::YELLOW));
        }
        ClipboardAccessResult::TextContent {
            preview,
            is_file_uri,
        } => {
            let prefix = if *is_file_uri {
                "📋 Clipboard: File URI"
            } else {
                "📋 Clipboard: Text"
            };
            ui.label(RichText::new(prefix).color(Color32::YELLOW));
            ui.label(
                RichText::new(truncate_string(preview, 40))
                    .small()
                    .monospace(),
            );
        }
        ClipboardAccessResult::AccessError(err) => {
            ui.label(RichText::new("📋 Clipboard error:").color(Color32::RED));
            ui.label(RichText::new(err).small().color(Color32::RED));
        }
        ClipboardAccessResult::NotSupported => {
            ui.label(
                RichText::new("📋 Clipboard: Not supported on this platform").color(Color32::GRAY),
            );
        }
    }
}

/// Render paste result
fn render_paste_result(ui: &mut Ui, result: &PasteResult) {
    match result {
        PasteResult::Success {
            width,
            height,
            bytes_len,
        } => {
            ui.label(RichText::new("✓ Paste success:").color(Color32::GREEN));
            ui.label(format!(
                "{}x{} ({})",
                width,
                height,
                format_bytes(*bytes_len)
            ));
        }
        PasteResult::NoImageContent => {
            ui.label(RichText::new("✗ Paste: No image in clipboard").color(Color32::YELLOW));
        }
        PasteResult::AccessError(err) => {
            ui.label(RichText::new("✗ Paste error:").color(Color32::RED));
            ui.label(RichText::new(err).small());
        }
        PasteResult::SetImageFailed { width, height } => {
            ui.label(RichText::new("✗ Paste: Set image failed").color(Color32::RED));
            ui.label(format!("({}x{} - texture creation failed)", width, height));
        }
    }
}

/// Render drop hover start
fn render_drop_hover_start(ui: &mut Ui, event: &DropHoverEvent) {
    ui.label(
        RichText::new(format!("⬇ Hover: {} file(s)", event.file_count))
            .color(Color32::from_rgb(100, 200, 255)),
    );
    if !event.file_names.is_empty() {
        let names = event.file_names.join(", ");
        ui.label(RichText::new(truncate_string(&names, 50)).small());
    }
    if !event.mime_types.is_empty() {
        let mimes = event.mime_types.join(", ");
        ui.label(RichText::new(format!("[{}]", mimes)).small().weak());
    }
}

/// Render drop result
fn render_drop_result(ui: &mut Ui, result: &DropResult) {
    match result {
        DropResult::Success {
            file_name,
            width,
            height,
            bytes_len,
        } => {
            ui.label(RichText::new("✓ Drop success:").color(Color32::GREEN));
            if let Some(name) = file_name {
                ui.label(name);
            }
            ui.label(format!(
                "{}x{} ({})",
                width,
                height,
                format_bytes(*bytes_len)
            ));
        }
        DropResult::InvalidImage { file_name, error } => {
            ui.label(RichText::new("✗ Drop: Invalid image").color(Color32::RED));
            if let Some(name) = file_name {
                ui.label(RichText::new(name).small());
            }
            ui.label(RichText::new(error).small().color(Color32::RED));
        }
        DropResult::NoValidFiles { file_count } => {
            ui.label(
                RichText::new(format!("✗ Drop: No valid files ({} dropped)", file_count))
                    .color(Color32::YELLOW),
            );
        }
        DropResult::ReadError { file_name, error } => {
            ui.label(RichText::new("✗ Drop: Read error").color(Color32::RED));
            if let Some(name) = file_name {
                ui.label(RichText::new(name).small());
            }
            ui.label(RichText::new(error).small().color(Color32::RED));
        }
        DropResult::SetImageFailed {
            file_name,
            width,
            height,
        } => {
            ui.label(RichText::new("✗ Drop: Set image failed").color(Color32::RED));
            if let Some(name) = file_name {
                ui.label(RichText::new(name).small());
            }
            ui.label(format!("({}x{} - texture creation failed)", width, height));
        }
    }
}

/// Format bytes as human-readable string
fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Truncate a string with ellipsis
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Get color for environment label
fn env_color(env: &str) -> Color32 {
    match env {
        "production" => Color32::GREEN,
        "internal" | "test-internal" => Color32::from_rgb(255, 165, 0), // Orange
        "test" => Color32::YELLOW,
        "nightly" => Color32::from_rgb(138, 43, 226), // Purple
        "pr" => Color32::from_rgb(100, 149, 237),     // Cornflower blue
        _ => Color32::GRAY,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(100), "100 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1536 * 1024), "1.5 MB");
    }

    #[test]
    fn test_truncate_string() {
        assert_eq!(truncate_string("hello", 10), "hello");
        assert_eq!(truncate_string("hello world", 8), "hello...");
        assert_eq!(truncate_string("hi", 2), "hi");
    }

    #[test]
    fn test_env_color() {
        assert_eq!(env_color("production"), Color32::GREEN);
        assert_eq!(env_color("internal"), Color32::from_rgb(255, 165, 0));
        assert_eq!(env_color("test-internal"), Color32::from_rgb(255, 165, 0));
        assert_eq!(env_color("test"), Color32::YELLOW);
        assert_eq!(env_color("unknown"), Color32::GRAY);
    }
}
