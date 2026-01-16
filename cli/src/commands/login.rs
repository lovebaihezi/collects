//! Login command implementation.

use anyhow::{Context as _, Result};
use collects_business::{AuthCompute, AuthStatus, LoginCommand as LoginCmd, LoginInput};
use collects_states::StateCtx;
use inquire::Text;
use tracing::{error, info, instrument};

use crate::auth::save_token;
use crate::context::flush_and_await;
use crate::output::Output;

#[instrument(skip_all, name = "login")]
pub async fn run_login(mut ctx: StateCtx) -> Result<()> {
    let out = Output::new();

    out.header("Login to Collects");
    out.newline();

    // Use inquire for better prompts
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

    ctx.enqueue_command::<LoginCmd>();
    flush_and_await(&mut ctx).await;

    let auth = ctx.compute::<AuthCompute>();
    match &auth.status {
        AuthStatus::Authenticated { username: u, token } => {
            info!("Successfully logged in as {u}");
            out.newline();
            out.success(format!("Successfully logged in as {u}"));
            if let Some(t) = token {
                save_token(u, t)?;
                out.success("Token saved to ~/.collects/token");
            }
        }
        AuthStatus::Failed(msg) => {
            error!("Login failed: {msg}");
            out.newline();
            out.error(format!("Login failed: {msg}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
        AuthStatus::NotAuthenticated | AuthStatus::Authenticating => {
            error!("Login did not complete");
            out.newline();
            out.error("Login did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    ctx.shutdown().await;
    Ok(())
}
