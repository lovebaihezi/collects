//! Shared color constants for the UI.

use egui::Color32;

/// Forest green color for healthy/available/success status.
pub const COLOR_GREEN: Color32 = Color32::from_rgb(34, 139, 34);

/// Red color for error/unavailable/failed status.
pub const COLOR_RED: Color32 = Color32::from_rgb(220, 53, 69);

/// Amber color for checking/pending status.
pub const COLOR_AMBER: Color32 = Color32::from_rgb(255, 193, 7);
