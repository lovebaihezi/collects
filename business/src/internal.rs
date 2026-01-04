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
    /// Seconds remaining until the OTP code expires (1-30).
    pub time_remaining: u8,
    /// The user's nickname (optional).
    pub nickname: Option<String>,
    /// The user's avatar URL (optional).
    pub avatar_url: Option<String>,
    /// When the user was created (ISO 8601 format).
    pub created_at: String,
    /// When the user was last updated (ISO 8601 format).
    pub updated_at: String,
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

/// Response from getting a single user with QR code info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetUserResponse {
    /// The username.
    pub username: String,
    /// The current OTP code for this user.
    pub current_otp: String,
    /// Seconds remaining until the OTP code expires (1-30).
    pub time_remaining: u8,
    /// The otpauth URL for QR code generation.
    pub otpauth_url: String,
}

/// Request to update a username.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUsernameRequest {
    /// The new username.
    pub new_username: String,
}

/// Response from updating a username.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateUsernameResponse {
    /// The old username.
    pub old_username: String,
    /// The new username.
    pub new_username: String,
}

/// Response from revoking OTP (regenerating secret).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeOtpResponse {
    /// The username.
    pub username: String,
    /// The new secret key for OTP generation (base32 encoded).
    pub secret: String,
    /// The otpauth URL for QR code generation.
    pub otpauth_url: String,
}

/// Response from deleting a user.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteUserResponse {
    /// The username that was deleted.
    pub username: String,
    /// Whether the deletion was successful.
    pub deleted: bool,
}

/// Request to update user's profile (nickname and avatar URL).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProfileRequest {
    /// The new nickname (optional, pass null to remove).
    pub nickname: Option<String>,
    /// The new avatar URL (optional, pass null to remove).
    pub avatar_url: Option<String>,
}

/// Response from updating user's profile.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProfileResponse {
    /// The username.
    pub username: String,
    /// The updated nickname.
    pub nickname: Option<String>,
    /// The updated avatar URL.
    pub avatar_url: Option<String>,
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
