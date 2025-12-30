//! Internal-only widgets module.
//!
//! This module contains widgets that are only available in internal builds:
//! - Users table with OTP codes
//! - Create user modal with QR code

mod users;

pub use users::{InternalUsersState, internal_users_panel, poll_internal_users_responses};
