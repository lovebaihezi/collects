//! Authentication and token management for the Collects CLI.

use std::io::IsTerminal as _;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use collects_business::{
    AuthCompute, AuthStatus, LoginCommand, LoginInput, PendingTokenValidation, ValidateTokenCommand,
};
use collects_states::StateCtx;
use dirs::home_dir;
use inquire::Text;
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};

use crate::context::flush_and_await;
use crate::output::Output;

/// Stored authentication token and username.
#[derive(Serialize, Deserialize)]
pub struct TokenStore {
    pub token: String,
    pub username: String,
}

/// Get the path to the token file.
pub fn get_token_path() -> Result<PathBuf> {
    let home = home_dir().context("Could not find home directory")?;
    let config_dir = home.join(".collects");
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }
    Ok(config_dir.join("token"))
}

/// Save the authentication token to disk.
pub fn save_token(username: &str, token: &str) -> Result<()> {
    let path = get_token_path()?;
    let store = TokenStore {
        token: token.to_owned(),
        username: username.to_owned(),
    };
    let json = serde_json::to_string(&store)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Load the authentication token from disk.
pub fn load_token() -> Result<Option<TokenStore>> {
    let path = get_token_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let store: TokenStore = serde_json::from_str(&content)?;
    Ok(Some(store))
}

/// Restore session from saved token.
#[instrument(skip_all, name = "restore_session")]
pub async fn restore_session(ctx: &mut StateCtx) -> Result<bool> {
    if let Some(store) = load_token()? {
        ctx.update::<PendingTokenValidation>(|s| {
            s.token = Some(store.token);
        });

        ctx.enqueue_command::<ValidateTokenCommand>();
        flush_and_await(ctx).await;

        let auth = ctx.compute::<AuthCompute>();
        match &auth.status {
            AuthStatus::Authenticated { .. } => Ok(true),
            _ => Ok(false),
        }
    } else {
        Ok(false)
    }
}

/// Ensures the user is authenticated, prompting for login if needed.
///
/// This function:
/// 1. Tries to restore session from saved token
/// 2. If token is expired/invalid, prompts the user to login interactively
/// 3. Returns Ok(()) if authentication succeeds, or exits on failure
#[instrument(skip_all, name = "ensure_authenticated")]
pub async fn ensure_authenticated(ctx: &mut StateCtx) -> Result<()> {
    let out = Output::new();

    // First try to restore existing session
    if restore_session(ctx).await? {
        return Ok(());
    }

    // Session restoration failed - prompt for login
    out.warning("Session expired or not logged in. Please login to continue.");
    out.newline();

    // Check if we're in a terminal that can accept input
    if !std::io::stdin().is_terminal() {
        out.error("Cannot prompt for login: stdin is not a terminal.");
        out.info("Please run 'collects login' first.");
        std::process::exit(1);
    }

    // Prompt for login
    let username = Text::new("Username:")
        .with_help_message("Enter your Collects username")
        .prompt()
        .context("Failed to read username")?;

    let otp = Text::new("OTP Code:")
        .with_help_message("Enter the 6-digit code from your authenticator app")
        .prompt()
        .context("Failed to read OTP")?;

    info!(username = ?username, otp = ?otp, "Attempting login");

    ctx.update::<LoginInput>(|s| {
        s.username = username.clone();
        s.otp = otp.clone();
    });

    ctx.enqueue_command::<LoginCommand>();
    flush_and_await(ctx).await;

    let auth = ctx.compute::<AuthCompute>();
    match &auth.status {
        AuthStatus::Authenticated { username: u, token } => {
            info!("Successfully logged in as {u}");
            out.success(format!("Logged in as {u}"));
            out.newline();
            if let Some(t) = token {
                save_token(u, t)?;
            }
            Ok(())
        }
        AuthStatus::Failed(msg) => {
            error!("Login failed: {msg}");
            out.error(format!("Login failed: {msg}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
        AuthStatus::NotAuthenticated | AuthStatus::Authenticating => {
            error!("Login did not complete");
            out.error("Login did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }
}
