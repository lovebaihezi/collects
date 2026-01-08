//! User management module.
//!
//! This module provides user-related functionality including:
//! - OTP (One-Time Password) authentication setup and verification
//! - User creation API endpoints
//! - Storage abstraction for user data (internal use only)

pub mod otp;
pub mod routes;
pub mod session_auth;
pub mod storage;

pub use routes::{
    AppState, DeleteUserResponse, GetUserResponse, ListUsersResponse, RevokeOtpResponse,
    UpdateUsernameRequest, UpdateUsernameResponse, UserListItem, auth_routes, internal_routes,
};
pub use session_auth::{RequireAuth, SessionAuthError};
pub use storage::{MockUserStorage, PgUserStorage, StoredUser, UserStorage, UserStorageError};
