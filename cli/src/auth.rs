//! Authentication handling for the CLI.
//!
//! Provides login/logout functionality using OTP verification.

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::config::Config;

/// Request payload for OTP verification.
#[derive(Debug, Serialize)]
struct VerifyOtpRequest {
    username: String,
    code: String,
}

/// Response from OTP verification endpoint.
#[derive(Debug, Deserialize)]
struct VerifyOtpResponse {
    valid: bool,
    message: Option<String>,
    token: Option<String>,
}

/// Request payload for token validation.
#[derive(Debug, Serialize)]
struct ValidateTokenRequest {
    token: String,
}

/// Response from token validation endpoint.
#[derive(Debug, Deserialize)]
struct ValidateTokenResponse {
    valid: bool,
    username: Option<String>,
    #[allow(dead_code)]
    message: Option<String>,
}

/// Log in using username and OTP code.
pub fn login(api_url: &str, username: &str, otp: &str) -> Result<()> {
    // Validate inputs
    if username.trim().is_empty() {
        bail!("Username cannot be empty");
    }

    let otp = otp.trim();
    if otp.len() != 6 || !otp.chars().all(|c| c.is_ascii_digit()) {
        bail!("OTP code must be exactly 6 digits");
    }

    println!("Signing in as '{}'...", username);

    // Make the API request
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/api/auth/verify-otp", api_url);

    let response = client
        .post(&url)
        .json(&VerifyOtpRequest {
            username: username.to_string(),
            code: otp.to_string(),
        })
        .send()
        .context("Failed to connect to server")?;

    let _status = response.status();
    let body: VerifyOtpResponse = response.json().context("Failed to parse server response")?;

    if !body.valid {
        let msg = body
            .message
            .unwrap_or_else(|| "Invalid username or OTP code".to_string());
        bail!("Authentication failed: {}", msg);
    }

    // Save token to config
    let token = body
        .token
        .context("Server did not return a session token")?;
    let mut config = Config::load().unwrap_or_default();
    config.set_auth(username, &token);
    config.save()?;

    println!("✓ Successfully signed in as '{}'", username);
    println!("  Token saved to: {}", Config::config_path()?.display());

    Ok(())
}

/// Log out and remove saved credentials.
pub fn logout() -> Result<()> {
    let mut config = Config::load().unwrap_or_default();

    if !config.has_token() {
        println!("Not currently signed in.");
        return Ok(());
    }

    let username = config.get_username().unwrap_or("unknown").to_string();
    config.clear_auth();
    config.save()?;

    println!("✓ Signed out '{}'", username);
    println!(
        "  Credentials removed from: {}",
        Config::config_path()?.display()
    );

    Ok(())
}

/// Show current authentication status.
pub fn status(api_url: &str) -> Result<()> {
    let config = Config::load().unwrap_or_default();

    if !config.has_token() {
        println!("Not signed in.");
        println!("\nUse 'collects login -u <username> -o <otp>' to sign in.");
        return Ok(());
    }

    let token = config.get_token().context("Token not found")?;
    let saved_username = config.get_username().unwrap_or("unknown");

    println!("Checking authentication status...");

    // Validate the token with the server
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/api/auth/validate-token", api_url);

    let response = client
        .post(&url)
        .json(&ValidateTokenRequest {
            token: token.to_string(),
        })
        .send();

    match response {
        Ok(resp) => {
            let body: ValidateTokenResponse = resp.json().context("Failed to parse response")?;

            if body.valid {
                let username = body.username.unwrap_or_else(|| saved_username.to_string());
                println!("✓ Signed in as '{}'", username);
                println!("  Config file: {}", Config::config_path()?.display());
            } else {
                println!("✗ Token is invalid or expired");
                println!("  Use 'collects login' to sign in again.");
            }
        }
        Err(e) => {
            println!("⚠ Could not verify token with server: {}", e);
            println!("  Locally saved as: '{}'", saved_username);
            println!("  Config file: {}", Config::config_path()?.display());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verify_otp_request_serialization() {
        let request = VerifyOtpRequest {
            username: "testuser".to_string(),
            code: "123456".to_string(),
        };

        let json = serde_json::to_string(&request).expect("Should serialize");
        assert!(json.contains("testuser"));
        assert!(json.contains("123456"));
    }

    #[test]
    fn test_verify_otp_response_deserialization() {
        let json = r#"{"valid": true, "token": "test-token"}"#;
        let response: VerifyOtpResponse = serde_json::from_str(json).expect("Should deserialize");
        assert!(response.valid);
        assert_eq!(response.token, Some("test-token".to_string()));
    }

    #[test]
    fn test_validate_token_request_serialization() {
        let request = ValidateTokenRequest {
            token: "test-token".to_string(),
        };

        let json = serde_json::to_string(&request).expect("Should serialize");
        assert!(json.contains("test-token"));
    }
}
