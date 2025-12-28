//! Internal-only widgets module.
//!
//! This module contains widgets that are only available in internal builds:
//! - Internal API status display
//! - Users table with OTP codes
//! - Create user modal with QR code

mod internal_api_status;
mod internal_users;

pub use internal_api_status::internal_api_status;
pub use internal_users::{InternalUsersState, internal_users_panel, poll_internal_users_responses};
