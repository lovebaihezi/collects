//! Shared color constants for the UI.

use egui::Color32;

/// Forest green color for healthy/available/success status.
pub const COLOR_GREEN: Color32 = Color32::from_rgb(34, 139, 34);

/// Red color for error/unavailable/failed status.
pub const COLOR_RED: Color32 = Color32::from_rgb(220, 53, 69);

/// Amber color for checking/pending status.
pub const COLOR_AMBER: Color32 = Color32::from_rgb(255, 193, 7);

// --- Typora-like table colors ---

/// Table header background color (light gray).
pub const TABLE_HEADER_BG: Color32 = Color32::from_rgb(246, 246, 246);

/// Table row stripe color (alternating rows).
pub const TABLE_ROW_STRIPE: Color32 = Color32::from_rgb(242, 242, 242);

/// Table border color.
pub const TABLE_BORDER: Color32 = Color32::from_rgb(136, 136, 136);
