//! Internal users management module.
//!
//! This module contains the internal users panel widget and its submodules:
//! - `modals`: Modal dialogs for user actions
//! - `panel`: Main panel widget and polling functions
//! - `qr`: QR code generation utilities
//! - `table`: Table rendering components (columns, header, row, cells)
//!
//! Notes:
//! - Domain `State`/`Compute`/`Command` live in `collects_business`.
//! - Network IO helpers live in `collects_business::internal_users::api`.
//! - Widgets should remain UI-only (render + dispatch/trigger).

mod modals;
mod panel;
pub(crate) mod qr;
pub mod table;

pub use collects_business::{InternalUsersState, UserAction};
pub use panel::internal_users_panel;
