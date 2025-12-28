//! User management module.
//!
//! This module provides user-related functionality including:
//! - OTP (One-Time Password) authentication setup and verification
//! - User creation API endpoints
//! - Storage abstraction for user data (internal use only)

pub mod otp;
pub mod routes;
pub mod storage;

pub use routes::{AppState, ListUsersResponse, UserListItem, auth_routes, internal_routes};
pub use storage::{MockUserStorage, PgUserStorage, StoredUser, UserStorage, UserStorageError};
