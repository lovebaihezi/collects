//! Table header rendering for the internal users table.

use egui::Ui;
use egui_extras::TableRow;

/// Header column labels.
const HEADERS: [&str; 6] = ["ID", "Username", "OTP Code", "Time Left", "OTP", "Actions"];

/// Renders the table header with centered, bold labels.
#[inline]
pub fn render_table_header(header: &mut TableRow<'_, '_>) {
    for label in HEADERS {
        header.col(|ui| {
            render_header_cell(ui, label);
        });
    }
}

/// Renders a single header cell with centered, bold text.
#[inline]
fn render_header_cell(ui: &mut Ui, label: &str) {
    ui.centered_and_justified(|ui| {
        ui.strong(label);
    });
}
