//! Collects CLI - Command-line interface for Collects
//!
//! This CLI tool provides:
//! - Authentication using username and OTP or saved tokens
//! - Token-based authentication for CI/CD (via COLLECTS_TOKEN env var)
//! - Clipboard image reading and display in supported terminals
//!
//! # Authentication Methods
//!
//! The CLI supports multiple authentication methods (checked in order):
//!
//! 1. **Environment variable** (`COLLECTS_TOKEN`): Best for CI/CD pipelines
//! 2. **Token from stdin** (`--with-token`): For scripted authentication
//! 3. **OTP login** (`-u <user> -o <otp>`): Interactive authentication
//! 4. **Saved token**: From config file at `$XDG_CONFIG_HOME/collects/config.toml`
//!
//! # Examples
//!
//! ```bash
//! # Interactive login with OTP
//! collects login -u myuser -o 123456
//!
//! # Token-based login for CI/CD
//! echo $MY_TOKEN | collects login --with-token
//!
//! # Use in GitHub Actions with environment variable
//! COLLECTS_TOKEN=${{ secrets.COLLECTS_TOKEN }} collects status
//! ```

#![warn(clippy::all, rust_2018_idioms)]

mod auth;
mod clipboard;
mod config;
mod terminal;

use anyhow::Result;
use clap::{Parser, Subcommand};

/// Collects CLI - Command-line tool for Collects
#[derive(Parser)]
#[command(name = "collects")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Override the API server URL
    #[arg(long, env = "COLLECTS_API_URL")]
    api_url: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Sign in to Collects using username and OTP, or with a token
    ///
    /// Supports multiple authentication methods:
    /// - OTP: collects login -u <username> -o <otp>
    /// - Token from stdin: echo $TOKEN | collects login --with-token
    /// - Token from env: COLLECTS_TOKEN=xxx collects login --with-token
    Login {
        /// Username for OTP authentication
        #[arg(short, long, required_unless_present = "with_token")]
        username: Option<String>,

        /// OTP code (6 digits) for authentication
        #[arg(short, long, required_unless_present = "with_token")]
        otp: Option<String>,

        /// Read token from stdin (for CI/CD pipelines)
        /// Token can also be provided via COLLECTS_TOKEN environment variable
        #[arg(long, conflicts_with_all = ["username", "otp"])]
        with_token: bool,
    },

    /// Sign out and remove saved credentials
    Logout,

    /// Show current authentication status
    Status,

    /// Read and display image from system clipboard
    #[command(name = "clipboard-image")]
    ClipboardImage {
        /// Output format: auto, kitty, iterm, sixel, or file
        #[arg(short, long, default_value = "auto")]
        format: String,

        /// Output file path (only used when format is 'file')
        #[arg(short, long)]
        output: Option<String>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine API URL
    let api_url = cli.api_url.unwrap_or_else(|| {
        if cfg!(feature = "env_test") {
            "https://collects-test.lqxclqxc.com".to_string()
        } else if cfg!(feature = "env_test_internal") {
            "https://collects-test-internal.lqxclqxc.com".to_string()
        } else if cfg!(feature = "env_pr") {
            "https://collects-pr.lqxclqxc.com".to_string()
        } else if cfg!(feature = "env_internal") {
            "https://collects-internal.lqxclqxc.com".to_string()
        } else if cfg!(feature = "env_nightly") {
            "https://collects-nightly.lqxclqxc.com".to_string()
        } else {
            "https://collects.lqxclqxc.com".to_string()
        }
    });

    match cli.command {
        Commands::Login {
            username,
            otp,
            with_token,
        } => {
            if with_token {
                auth::login_with_token(&api_url)?;
            } else {
                // Safe to unwrap because clap ensures these are present when with_token is false
                auth::login(&api_url, &username.unwrap(), &otp.unwrap())?;
            }
        }
        Commands::Logout => {
            auth::logout()?;
        }
        Commands::Status => {
            auth::status(&api_url)?;
        }
        Commands::ClipboardImage { format, output } => {
            clipboard::show_clipboard_image(&format, output.as_deref())?;
        }
    }

    Ok(())
}
