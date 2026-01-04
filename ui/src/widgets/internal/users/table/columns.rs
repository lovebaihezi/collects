//! Column definitions for the internal users table.

use egui_extras::Column;

/// Fixed column widths for consistent table layout
pub const ID_WIDTH: f32 = 50.0;
pub const OTP_CODE_WIDTH: f32 = 100.0;
pub const TIME_LEFT_WIDTH: f32 = 80.0;
pub const OTP_BUTTON_WIDTH: f32 = 70.0;
pub const ACTIONS_WIDTH: f32 = 180.0;
pub const ROW_HEIGHT: f32 = 30.0;
pub const HEADER_HEIGHT: f32 = 24.0;
pub const QR_ROW_HEIGHT: f32 = 240.0;

/// Table column configuration for the internal users table.
///
/// Returns a vector of column definitions in order:
/// - ID (fixed width with border indicator)
/// - Username (flexible, fills remaining space)
/// - OTP Code (fixed)
/// - Time Left (fixed)
/// - OTP button (fixed)
/// - Actions (fixed)
#[inline]
pub fn table_columns() -> Vec<Column> {
    vec![
        Column::exact(ID_WIDTH),             // ID
        Column::remainder().at_least(100.0), // Username - flexible
        Column::exact(OTP_CODE_WIDTH),       // OTP Code - fixed
        Column::exact(TIME_LEFT_WIDTH),      // Time Left - fixed
        Column::exact(OTP_BUTTON_WIDTH),     // OTP button - fixed
        Column::exact(ACTIONS_WIDTH),        // Actions - fixed
    ]
}
