//! Internal users management module.
//!
//! This module contains the internal users panel widget and its submodules:
//! - `state`: State management for the internal users panel
//! - `api`: API calls for user management
//! - `modals`: Modal dialogs for user actions
//! - `panel`: Main panel widget and polling functions
//! - `qr`: QR code generation utilities
//! - `table`: Table rendering components (columns, header, row, cells)

mod api;
mod modals;
mod panel;
pub(crate) mod qr;
mod state;
pub mod table;

pub use api::fetch_users;
pub use panel::{internal_users_panel, poll_internal_users_responses};
pub use state::InternalUsersState;

// Re-export internal functions for use by modals
pub(crate) use panel::{reset_create_user_compute, trigger_create_user};
