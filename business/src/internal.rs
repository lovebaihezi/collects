//! Internal environment functionality.
//!
//! This module provides functionality only available in internal builds
//! (env_internal and env_test_internal features). It includes:
//! - Internal API status checking
//! - User management (listing users, current OTP codes)
//! - User creation with QR code display

use serde::{Deserialize, Serialize};

/// Check if we're in an internal build environment.
#[inline]
pub const fn is_internal_build() -> bool {
    cfg!(any(feature = "env_internal", feature = "env_test_internal"))
}

/// A user item from the internal API.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalUserItem {
    /// The username.
    pub username: String,
    /// The current OTP code for this user.
    pub current_otp: String,
}

/// Response from listing internal users.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListUsersResponse {
    /// List of users with their current OTP codes.
    pub users: Vec<InternalUserItem>,
}

/// Response from creating a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserResponse {
    /// The username of the created user.
    pub username: String,
    /// The secret key for OTP generation (base32 encoded).
    pub secret: String,
    /// The otpauth URL for QR code generation.
    pub otpauth_url: String,
}

/// Request to create a new user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateUserRequest {
    /// The username for the new user.
    pub username: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_internal_build() {
        // This test verifies the function compiles and can be called.
        // The actual value depends on feature flags at compile time.
        let _result: bool = is_internal_build();
    }
}
