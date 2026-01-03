//! Internal page for authenticated users (internal builds only).
//!
//! Displays user info centered on the screen with the internal users panel.

use crate::{state::State, widgets};
use collects_business::{AuthCompute, BusinessConfig};
use egui::{Align, Layout, Response, Ui};

/// Renders the internal page for authenticated users in internal builds.
///
/// Shows user info centered on the screen with the internal users management panel.
pub fn internal_page(state: &mut State, ui: &mut Ui) -> Response {
    // Get username for display
    let username = state
        .ctx
        .cached::<AuthCompute>()
        .and_then(|c| c.username().map(String::from))
        .unwrap_or_default();

    let api_base_url = state
        .ctx
        .state_mut::<BusinessConfig>()
        .api_url()
        .to_string();

    ui.with_layout(Layout::top_down(Align::Center), |ui| {
        // Show signed-in header centered
        widgets::show_signed_in_header(ui, &username);

        ui.add_space(16.0);

        // Show internal users panel centered
        widgets::internal_users_panel(&mut state.internal_users, &mut state.ctx, &api_base_url, ui);

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
