//! Internal page for authenticated users (internal builds only).
//!
//! Displays only the internal users table using native egui table styling
//! that adapts properly to light and dark themes.
//! The App title and signed-in information are intentionally omitted to
//! provide a focused, data-centric view.

use crate::{state::State, widgets};
use collects_business::BusinessConfig;
use egui::{Response, Ui};

/// Renders the internal page for authenticated users in internal builds.
///
/// Shows only the internal users management panel (table) without the App title
/// or signed-in header, providing a clean, focused data view.
pub fn internal_page(state: &mut State, ui: &mut Ui) -> Response {
    let api_base_url = state.ctx.state::<BusinessConfig>().api_url().to_string();

    ui.vertical(|ui| {
        // Show internal users panel (table only, no header decorations)
        widgets::internal_users_panel(&mut state.ctx, &api_base_url, ui);
    })
    .response
}
