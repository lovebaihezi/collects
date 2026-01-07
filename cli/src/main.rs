//! Collects CLI - Command-line interface for Collects
//!
//! This CLI tool provides:
//! - Authentication using username and OTP or saved tokens
//! - Clipboard image reading and display in supported terminals

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
    /// Sign in to Collects using username and OTP
    Login {
        /// Username for authentication
        #[arg(short, long)]
        username: String,

        /// OTP code (6 digits) for authentication
        #[arg(short, long)]
        otp: String,
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
        Commands::Login { username, otp } => {
            auth::login(&api_url, &username, &otp)?;
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
