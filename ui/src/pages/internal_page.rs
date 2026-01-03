//! Internal page for authenticated users (internal builds only).
//!
//! Displays the internal users panel with a clean, Typora-like table style.

use crate::{state::State, widgets};
use collects_business::BusinessConfig;
use egui::{Response, Ui};

/// Renders the internal page for authenticated users in internal builds.
///
/// Shows only the internal users management panel with a clean table layout.
pub fn internal_page(state: &mut State, ui: &mut Ui) -> Response {
    let api_base_url = state
        .ctx
        .state_mut::<BusinessConfig>()
        .api_url()
        .to_string();

    // Show only the internal users panel (Typora-like table style)
    widgets::internal_users_panel(&mut state.internal_users, &mut state.ctx, &api_base_url, ui)
}
