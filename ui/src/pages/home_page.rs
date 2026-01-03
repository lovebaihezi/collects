//! Home page for authenticated users (non-internal builds).
//!
//! Displays the signed-in header and basic app content.

use crate::{state::State, widgets};
use collects_business::AuthCompute;
use egui::{Response, Ui};

/// Renders the home page for authenticated users.
///
/// Shows the signed-in header with username and basic app content.
pub fn home_page(state: &mut State, ui: &mut Ui) -> Response {
    // Get username for display
    let username = state
        .ctx
        .cached::<AuthCompute>()
        .and_then(|c| c.username().map(String::from))
        .unwrap_or_default();

    ui.vertical(|ui| {
        // Show signed-in header (reusing the shared widget)
        widgets::show_signed_in_header(ui, &username);

        ui.add_space(16.0);
        powered_by_egui_and_eframe(ui);
    })
    .response
}

fn powered_by_egui_and_eframe(ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.label("Powered by ");
        ui.hyperlink_to("egui", "https://github.com/emilk/egui");
        ui.label(" and ");
        ui.hyperlink_to(
            "eframe",
            "https://github.com/emilk/egui/tree/master/crates/eframe",
        );
        ui.label(".");
    });
}
