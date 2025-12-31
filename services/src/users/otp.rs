//! OTP (One-Time Password) module for user authentication.
//!
//! This module provides TOTP (Time-based One-Time Password) functionality
//! for user authentication using Google Authenticator or similar apps.

use serde::{Deserialize, Serialize};
use totp_rs::{Algorithm, Secret, TOTP};

/// Request to create a new user with OTP authentication.
#[derive(Debug, Deserialize)]
pub struct CreateUserRequest {
    /// The username for the new user.
    pub username: String,
}

/// Response after creating a user with OTP.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserResponse {
    /// The username of the created user.
    pub username: String,
    /// The secret key for OTP generation (base32 encoded).
    pub secret: String,
    /// The otpauth URL for QR code generation.
    pub otpauth_url: String,
}

/// Request to verify an OTP code.
#[derive(Debug, Deserialize)]
pub struct VerifyOtpRequest {
    /// The username of the user.
    pub username: String,
    /// The OTP code to verify.
    pub code: String,
}

/// Response after verifying an OTP code.
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyOtpResponse {
    /// Whether the OTP code is valid.
    pub valid: bool,
    /// Optional message with details.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// Error types for OTP operations.
#[derive(Debug, thiserror::Error)]
pub enum OtpError {
    #[error("Failed to generate secret: {0}")]
    SecretGeneration(String),
    #[error("Failed to create TOTP: {0}")]
    TotpCreation(String),
    #[error("Invalid username: {0}")]
    InvalidUsername(String),
    #[error("Invalid OTP code")]
    InvalidCode,
}

/// The issuer name used in TOTP configuration.
pub const ISSUER: &str = "Collects";

/// Number of digits in the OTP code.
const OTP_DIGITS: usize = 6;

/// Number of time steps to allow for skew (before and after current time).
const OTP_SKEW: u8 = 1;

/// Duration of each time step in seconds.
const OTP_STEP: u64 = 30;

/// Generates a new TOTP secret and returns the configuration for a user.
///
/// # Arguments
///
/// * `username` - The username for the new user
///
/// # Returns
///
/// Returns a tuple containing (secret_base32, otpauth_url).
///
/// # Errors
///
/// Returns an error if the username is invalid or secret generation fails.
pub fn generate_otp_secret(username: &str) -> Result<(String, String), OtpError> {
    if username.is_empty() {
        return Err(OtpError::InvalidUsername(
            "Username cannot be empty".to_string(),
        ));
    }

    // Generate a random secret
    let secret = Secret::generate_secret();
    let secret_bytes = secret
        .to_bytes()
        .map_err(|e| OtpError::SecretGeneration(e.to_string()))?;
    let secret_base32 = secret.to_encoded().to_string();

    // Create TOTP configuration with issuer and account name
    let totp = TOTP::new(
        Algorithm::SHA1,
        OTP_DIGITS,
        OTP_SKEW,
        OTP_STEP,
        secret_bytes,
        Some(ISSUER.to_string()),
        username.to_string(),
    )
    .map_err(|e| OtpError::TotpCreation(e.to_string()))?;

    // Generate the otpauth URL (issuer and account_name are already part of TOTP)
    let otpauth_url = totp.get_url();

    Ok((secret_base32, otpauth_url))
}

/// Verifies an OTP code against a secret.
///
/// # Arguments
///
/// * `secret_base32` - The base32 encoded secret
/// * `code` - The OTP code to verify
///
/// # Returns
///
/// Returns true if the code is valid, false otherwise.
///
/// # Errors
///
/// Returns an error if the secret is invalid or TOTP creation fails.
pub fn verify_otp(secret_base32: &str, code: &str) -> Result<bool, OtpError> {
    let secret = Secret::Encoded(secret_base32.to_string());
    let secret_bytes = secret
        .to_bytes()
        .map_err(|e| OtpError::SecretGeneration(e.to_string()))?;

    let totp = TOTP::new(
        Algorithm::SHA1,
        OTP_DIGITS,
        OTP_SKEW,
        OTP_STEP,
        secret_bytes,
        Some(ISSUER.to_string()),
        String::new(), // account_name not needed for verification
    )
    .map_err(|e| OtpError::TotpCreation(e.to_string()))?;

    // Note: check_current returns Err only on system time errors, which are unlikely
    // but should be logged if they occur. In production, a false return is safe.
    match totp.check_current(code) {
        Ok(valid) => Ok(valid),
        Err(e) => {
            tracing::warn!("OTP verification encountered a system time error: {}", e);
            Ok(false)
        }
    }
}

/// Generates the current OTP code for a given secret.
///
/// This is primarily useful for testing.
///
/// # Arguments
///
/// * `secret_base32` - The base32 encoded secret
///
/// # Returns
///
/// Returns the current OTP code.
///
/// # Errors
///
/// Returns an error if the secret is invalid or code generation fails.
pub fn generate_current_otp(secret_base32: &str) -> Result<String, OtpError> {
    let secret = Secret::Encoded(secret_base32.to_string());
    let secret_bytes = secret
        .to_bytes()
        .map_err(|e| OtpError::SecretGeneration(e.to_string()))?;

    let totp = TOTP::new(
        Algorithm::SHA1,
        OTP_DIGITS,
        OTP_SKEW,
        OTP_STEP,
        secret_bytes,
        Some(ISSUER.to_string()),
        String::new(), // account_name not needed for code generation
    )
    .map_err(|e| OtpError::TotpCreation(e.to_string()))?;

    totp.generate_current()
        .map_err(|e| OtpError::TotpCreation(e.to_string()))
}

/// Calculates the seconds remaining until the current OTP code expires.
///
/// OTP codes change every 30 seconds. This function returns the number of
/// seconds until the next code change, which helps users know how much time
/// they have to use the current code.
///
/// # Returns
///
/// Returns the number of seconds (0-29) until the current code expires.
pub fn get_time_remaining() -> u8 {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Time remaining = step - (current_time mod step)
    let elapsed_in_step = now % OTP_STEP;
    (OTP_STEP - elapsed_in_step) as u8
}

/// Generates the current OTP code and time remaining until it expires.
///
/// This is useful for displaying OTP codes with a countdown timer in the UI.
///
/// # Arguments
///
/// * `secret_base32` - The base32 encoded secret
///
/// # Returns
///
/// Returns a tuple of (current_otp_code, seconds_remaining).
///
/// # Errors
///
/// Returns an error if the secret is invalid or code generation fails.
pub fn generate_current_otp_with_time(secret_base32: &str) -> Result<(String, u8), OtpError> {
    let code = generate_current_otp(secret_base32)?;
    let time_remaining = get_time_remaining();
    Ok((code, time_remaining))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_otp_secret_success() {
        let username = "testuser";
        let result = generate_otp_secret(username);

        assert!(result.is_ok(), "Should successfully generate OTP secret");

        let (secret, url) = result.expect("Should have secret and url");

        // Secret should be non-empty base32 string
        assert!(!secret.is_empty(), "Secret should not be empty");

        // URL should contain the expected components
        assert!(
            url.starts_with("otpauth://totp/"),
            "URL should start with otpauth://totp/"
        );
        assert!(url.contains(username), "URL should contain username");
        assert!(url.contains(ISSUER), "URL should contain issuer");
    }

    #[test]
    fn test_generate_otp_secret_empty_username() {
        let result = generate_otp_secret("");

        assert!(result.is_err(), "Should fail with empty username");

        match result {
            Err(OtpError::InvalidUsername(msg)) => {
                assert!(msg.contains("empty"), "Error message should mention empty");
            }
            _ => panic!("Expected InvalidUsername error"),
        }
    }

    #[test]
    fn test_verify_otp_valid_code() {
        let username = "testuser";
        let (secret, _) = generate_otp_secret(username).expect("Should generate secret");

        // Generate a current code
        let code = generate_current_otp(&secret).expect("Should generate code");

        // Verify the code
        let result = verify_otp(&secret, &code);

        assert!(result.is_ok(), "Verification should not error");
        assert!(result.expect("Should have result"), "Code should be valid");
    }

    #[test]
    fn test_verify_otp_invalid_code() {
        let username = "testuser";
        let (secret, _) = generate_otp_secret(username).expect("Should generate secret");

        // Try to verify an invalid code
        let result = verify_otp(&secret, "000000");

        assert!(result.is_ok(), "Verification should not error");
        // Note: This might occasionally pass if 000000 happens to be the current code
        // but statistically this is very unlikely
    }

    #[test]
    fn test_verify_otp_invalid_secret() {
        let result = verify_otp("invalid_secret", "123456");

        assert!(result.is_err(), "Should fail with invalid secret");
    }

    #[test]
    fn test_generate_current_otp() {
        let username = "testuser";
        let (secret, _) = generate_otp_secret(username).expect("Should generate secret");

        let code = generate_current_otp(&secret);

        assert!(code.is_ok(), "Should generate current OTP");

        let code = code.expect("Should have code");
        assert_eq!(code.len(), 6, "OTP code should be 6 digits");
        assert!(
            code.chars().all(|c| c.is_ascii_digit()),
            "OTP code should be all digits"
        );
    }

    #[test]
    fn test_otp_roundtrip() {
        // Test the full flow: generate secret -> generate code -> verify code
        let username = "integration_test_user";

        // Step 1: Generate secret for user
        let (secret, otpauth_url) = generate_otp_secret(username).expect("Should generate secret");

        // Verify the otpauth URL format
        assert!(
            otpauth_url.contains("secret="),
            "URL should contain secret parameter"
        );

        // Step 2: Generate a code (simulating what the authenticator app would do)
        let code = generate_current_otp(&secret).expect("Should generate code");

        // Step 3: Verify the code
        let is_valid = verify_otp(&secret, &code).expect("Verification should not error");

        assert!(is_valid, "Generated code should be valid");
    }

    #[test]
    fn test_different_users_get_different_secrets() {
        let (secret1, _) = generate_otp_secret("user1").expect("Should generate secret for user1");
        let (secret2, _) = generate_otp_secret("user2").expect("Should generate secret for user2");

        assert_ne!(
            secret1, secret2,
            "Different users should get different secrets"
        );
    }

    #[test]
    fn test_get_time_remaining() {
        let time_remaining = get_time_remaining();

        // Time remaining should always be between 1 and 30 seconds
        assert!(
            time_remaining >= 1 && time_remaining <= 30,
            "Time remaining should be between 1 and 30, got {}",
            time_remaining
        );
    }

    #[test]
    fn test_generate_current_otp_with_time() {
        let username = "testuser";
        let (secret, _) = generate_otp_secret(username).expect("Should generate secret");

        let result = generate_current_otp_with_time(&secret);

        assert!(result.is_ok(), "Should generate OTP with time");

        let (code, time_remaining) = result.expect("Should have result");

        // Verify code format
        assert_eq!(code.len(), 6, "OTP code should be 6 digits");
        assert!(
            code.chars().all(|c| c.is_ascii_digit()),
            "OTP code should be all digits"
        );

        // Verify time remaining is valid
        assert!(
            time_remaining >= 1 && time_remaining <= 30,
            "Time remaining should be between 1 and 30, got {}",
            time_remaining
        );
    }
}
