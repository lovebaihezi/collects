//! Login page for unauthenticated users.
//!
//! Displays the login form centered on the screen.

use crate::{state::State, widgets};
use egui::{Response, Ui};

/// Renders the login page with a centered login form.
pub fn login_page(state: &mut State, ui: &mut Ui) -> Response {
    widgets::login_widget(&mut state.ctx, ui)
}
