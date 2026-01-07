//! Authentication handling for the CLI.
//!
//! Provides login/logout functionality using OTP verification or token-based auth.
//!
//! # Authentication Methods
//!
//! 1. **COLLECTS_TOKEN environment variable**: Highest priority, for CI/CD
//! 2. **Token from stdin** (`--with-token`): For scripted authentication
//! 3. **OTP login** (`-u <user> -o <otp>`): Interactive authentication
//! 4. **Saved token**: From config file

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, IsTerminal};

use crate::config::Config;

/// Environment variable name for token-based authentication.
pub const COLLECTS_TOKEN_ENV: &str = "COLLECTS_TOKEN";

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

/// Get the current authentication token.
///
/// Checks sources in order of priority:
/// 1. COLLECTS_TOKEN environment variable
/// 2. Saved token from config file
///
/// Returns (token, source_description)
pub fn get_token() -> Option<(String, &'static str)> {
    // Priority 1: Environment variable (for CI/CD)
    if let Ok(token) = std::env::var(COLLECTS_TOKEN_ENV)
        && !token.is_empty()
    {
        return Some((token, "environment variable"));
    }

    // Priority 2: Config file
    if let Ok(config) = Config::load()
        && let Some(token) = config.get_token()
    {
        return Some((token.to_string(), "config file"));
    }

    None
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

/// Log in using a token (from env var or stdin).
///
/// This method is designed for CI/CD pipelines where interactive login is not possible.
/// The token is read from:
/// 1. COLLECTS_TOKEN environment variable (if set)
/// 2. stdin (if piped)
pub fn login_with_token(api_url: &str) -> Result<()> {
    // Try to get token from environment variable first
    let token = if let Ok(env_token) = std::env::var(COLLECTS_TOKEN_ENV) {
        if !env_token.is_empty() {
            println!(
                "Using token from {} environment variable",
                COLLECTS_TOKEN_ENV
            );
            env_token
        } else {
            read_token_from_stdin()?
        }
    } else {
        read_token_from_stdin()?
    };

    let token = token.trim().to_string();
    if token.is_empty() {
        bail!(
            "No token provided. Set {} environment variable or pipe token to stdin.",
            COLLECTS_TOKEN_ENV
        );
    }

    println!("Validating token...");

    // Validate the token with the server
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/api/auth/validate-token", api_url);

    let response = client
        .post(&url)
        .json(&ValidateTokenRequest {
            token: token.clone(),
        })
        .send()
        .context("Failed to connect to server")?;

    let body: ValidateTokenResponse = response.json().context("Failed to parse server response")?;

    if !body.valid {
        bail!("Token validation failed: invalid or expired token");
    }

    let username = body.username.unwrap_or_else(|| "unknown".to_string());

    // Save token to config
    let mut config = Config::load().unwrap_or_default();
    config.set_auth(&username, &token);
    config.save()?;

    println!("✓ Successfully authenticated as '{}'", username);
    println!("  Token saved to: {}", Config::config_path()?.display());

    Ok(())
}

/// Read token from stdin.
fn read_token_from_stdin() -> Result<String> {
    let stdin = io::stdin();

    // Check if stdin is a terminal (interactive) or piped
    if stdin.is_terminal() {
        bail!(
            "No token provided. Either:\n\
             - Set {} environment variable, or\n\
             - Pipe token to stdin: echo $TOKEN | collects login --with-token",
            COLLECTS_TOKEN_ENV
        );
    }

    // Read from piped stdin
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .context("Failed to read token from stdin")?;

    Ok(line)
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

    // Note about environment variable
    if std::env::var(COLLECTS_TOKEN_ENV).is_ok() {
        println!(
            "\n⚠ Note: {} environment variable is still set.\n\
               Unset it to fully sign out.",
            COLLECTS_TOKEN_ENV
        );
    }

    Ok(())
}

/// Show current authentication status.
pub fn status(api_url: &str) -> Result<()> {
    // Check for token from any source
    let (token, source) = match get_token() {
        Some((t, s)) => (t, s),
        None => {
            println!("Not signed in.");
            println!("\nTo sign in, use one of:");
            println!("  collects login -u <username> -o <otp>");
            println!("  echo $TOKEN | collects login --with-token");
            println!("  COLLECTS_TOKEN=xxx collects status");
            return Ok(());
        }
    };

    println!("Checking authentication status...");
    println!("  Token source: {}", source);

    // Validate the token with the server
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/api/auth/validate-token", api_url);

    let response = client
        .post(&url)
        .json(&ValidateTokenRequest { token })
        .send();

    match response {
        Ok(resp) => {
            let body: ValidateTokenResponse = resp.json().context("Failed to parse response")?;

            if body.valid {
                let username = body.username.unwrap_or_else(|| "unknown".to_string());
                println!("✓ Signed in as '{}'", username);

                // Show config path if token is from config
                if source == "config file" {
                    println!("  Config file: {}", Config::config_path()?.display());
                }
            } else {
                println!("✗ Token is invalid or expired");
                if source == "environment variable" {
                    println!("  Check the {} value.", COLLECTS_TOKEN_ENV);
                } else {
                    println!("  Use 'collects login' to sign in again.");
                }
            }
        }
        Err(e) => {
            println!("⚠ Could not verify token with server: {}", e);
            println!("  Token source: {}", source);
            if source == "config file" {
                println!("  Config file: {}", Config::config_path()?.display());
            }
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

    #[test]
    fn test_collects_token_env_constant() {
        assert_eq!(COLLECTS_TOKEN_ENV, "COLLECTS_TOKEN");
    }
}
