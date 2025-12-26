//! User management module.
//!
//! This module provides user-related functionality including:
//! - OTP (One-Time Password) authentication setup and verification
//! - User creation API endpoints
//! - Storage abstraction for user data (internal use only)

pub mod otp;
pub mod routes;
pub mod storage;

pub use routes::{
    AppState, auth_routes, auth_routes_legacy, internal_routes, internal_routes_legacy,
};
pub use storage::{MockUserStorage, PgUserStorage, StoredUser, UserStorage, UserStorageError};
