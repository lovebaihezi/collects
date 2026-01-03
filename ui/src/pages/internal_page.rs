//! Internal page for authenticated users (internal builds only).
//!
//! Displays only the users table using Typora-style design.

use crate::{state::State, widgets};
use collects_business::BusinessConfig;
use egui::{Response, Ui};

/// Renders the internal page for authenticated users in internal builds.
///
/// Shows only the internal users panel (table) without app title or signed-in info.
pub fn internal_page(state: &mut State, ui: &mut Ui) -> Response {
    let api_base_url = state
        .ctx
        .state_mut::<BusinessConfig>()
        .api_url()
        .to_string();

    // Show only the internal users panel (table)
    widgets::internal_users_panel(&mut state.ctx, &api_base_url, ui)
}
