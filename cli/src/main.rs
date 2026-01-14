#![allow(clippy::exit)]

use std::io::IsTerminal as _;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use collects_business::{
    Attachment, AuthCompute, AuthStatus, BusinessConfig, CFTokenCompute, ContentCreationStatus,
    CreateContentCommand, CreateContentCompute, CreateContentInput, LoginCommand, LoginInput,
    PendingTokenValidation, ValidateTokenCommand,
};
use collects_states::StateCtx;
use dirs::home_dir;
use log::{error, info};
use serde::{Deserialize, Serialize};

#[derive(Parser)]
#[command(name = "collects")]
#[command(about = "CLI for Collects", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Read content from stdin (implied if no subcommand and input is piped)
    #[arg(long, short = 'i')]
    stdin: bool,

    /// Attach files
    #[arg(long, short = 'f')]
    file: Vec<PathBuf>,

    /// Title for the content
    #[arg(long, short = 't')]
    title: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Login to Collects
    Login,
    /// Create new content (default if no command)
    Create {
        /// Attach files
        #[arg(long, short = 'f')]
        file: Vec<PathBuf>,

        /// Title for the content
        #[arg(long, short = 't')]
        title: Option<String>,
    },
}

#[derive(Serialize, Deserialize)]
struct TokenStore {
    token: String,
    username: String,
}

fn get_token_path() -> Result<PathBuf> {
    let home = home_dir().context("Could not find home directory")?;
    let config_dir = home.join(".collects");
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)?;
    }
    Ok(config_dir.join("token"))
}

fn save_token(username: &str, token: &str) -> Result<()> {
    let path = get_token_path()?;
    let store = TokenStore {
        token: token.to_owned(),
        username: username.to_owned(),
    };
    let json = serde_json::to_string(&store)?;
    std::fs::write(path, json)?;
    Ok(())
}

fn load_token() -> Result<Option<TokenStore>> {
    let path = get_token_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(path)?;
    let store: TokenStore = serde_json::from_str(&content)?;
    Ok(Some(store))
}

/// Initialize `StateCtx` with CLI-relevant states, computes, and commands.
///
/// This mirrors the pattern used in `State::build()` from the UI crate,
/// but only registers components needed for CLI operations.
fn build_state_ctx(config: BusinessConfig) -> StateCtx {
    let mut ctx = StateCtx::new();

    // Business config
    ctx.add_state(config);

    // Login states and computes
    ctx.add_state(LoginInput::default());
    ctx.add_state(PendingTokenValidation::default());
    ctx.record_compute(CFTokenCompute::default());
    ctx.record_compute(AuthCompute::default());

    // Content creation states and computes
    ctx.add_state(CreateContentInput::default());
    ctx.record_compute(CreateContentCompute::default());

    // Commands
    ctx.record_command(LoginCommand);
    ctx.record_command(ValidateTokenCommand);
    ctx.record_command(CreateContentCommand);

    ctx
}

/// Await all pending tasks in the `JoinSet` and sync computes.
///
/// This replaces the sleep-polling pattern with proper async awaiting.
async fn await_pending_tasks(ctx: &mut StateCtx) {
    while ctx.task_count() > 0 {
        // Wait for the next task to complete
        if ctx.task_set_mut().join_next().await.is_some() {
            // Apply any updates from completed tasks
            ctx.sync_computes();
        }
    }
}

/// Flush commands and await all spawned tasks.
///
/// This is the CLI equivalent of the end-of-frame pattern used in `CollectsApp`:
/// 1. `sync_computes()` - apply any pending async results
/// 2. `flush_commands()` - execute queued commands (may spawn async tasks)
/// 3. await pending tasks and sync their results
async fn flush_and_await(ctx: &mut StateCtx) {
    ctx.sync_computes();
    ctx.flush_commands();
    await_pending_tasks(ctx).await;
    ctx.sync_computes();
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    // Initialize StateCtx like CollectsApp does
    let ctx = build_state_ctx(BusinessConfig::default());

    match cli.command {
        Some(Commands::Login) => run_login(ctx).await,
        Some(Commands::Create { file, title }) => run_create(ctx, file, title).await,
        None => {
            // Default: Create content
            // Check if we have files or stdin
            let files = cli.file;
            let mut body = None;

            // Check if stdin has data
            if !std::io::stdin().is_terminal() {
                use std::io::Read as _;
                let mut buffer = Vec::new();
                std::io::stdin().read_to_end(&mut buffer)?;
                if !buffer.is_empty() {
                    body = Some(String::from_utf8(buffer)?);
                }
            }

            if files.is_empty() && body.is_none() {
                // Print help if no input
                use clap::CommandFactory as _;
                Cli::command().print_help()?;
                return Ok(());
            }

            run_create_impl(ctx, files, cli.title, body).await
        }
    }
}

async fn run_login(mut ctx: StateCtx) -> Result<()> {
    println!("Login to Collects");

    // Prompt for credentials
    let username = rpassword::prompt_password("Username: ")?;
    let otp = rpassword::prompt_password("OTP Code: ")?;

    ctx.update::<LoginInput>(|s| {
        s.username = username.clone();
        s.otp = otp;
    });

    ctx.enqueue_command::<LoginCommand>();
    flush_and_await(&mut ctx).await;

    let auth = ctx.compute::<AuthCompute>();
    match &auth.status {
        AuthStatus::Authenticated { username: u, token } => {
            info!("Successfully logged in as {u}");
            if let Some(t) = token {
                save_token(u, t)?;
                info!("Token saved.");
            }
        }
        AuthStatus::Failed(msg) => {
            error!("Login failed: {msg}");
            std::process::exit(1);
        }
        AuthStatus::NotAuthenticated | AuthStatus::Authenticating => {
            error!("Login did not complete");
            std::process::exit(1);
        }
    }

    ctx.shutdown().await;
    Ok(())
}

async fn run_create(ctx: StateCtx, files: Vec<PathBuf>, title: Option<String>) -> Result<()> {
    // Check for stdin in this mode too
    let mut body = None;
    if !std::io::stdin().is_terminal() {
        use std::io::Read as _;
        let mut buffer = Vec::new();
        std::io::stdin().read_to_end(&mut buffer)?;
        if !buffer.is_empty() {
            body = Some(String::from_utf8(buffer)?);
        }
    }

    run_create_impl(ctx, files, title, body).await
}

async fn run_create_impl(
    mut ctx: StateCtx,
    files: Vec<PathBuf>,
    title: Option<String>,
    body: Option<String>,
) -> Result<()> {
    // 1. Restore Session
    if let Some(store) = load_token()? {
        ctx.update::<PendingTokenValidation>(|s| {
            s.token = Some(store.token);
        });
        ctx.enqueue_command::<ValidateTokenCommand>();
        flush_and_await(&mut ctx).await;

        let auth = ctx.compute::<AuthCompute>();
        match &auth.status {
            AuthStatus::Authenticated { .. } => {
                // Session restored successfully
            }
            AuthStatus::Failed(e) => {
                error!("Auth error: {e}");
                std::process::exit(1);
            }
            AuthStatus::NotAuthenticated | AuthStatus::Authenticating => {
                error!("Session expired or invalid. Please login again.");
                std::process::exit(1);
            }
        }
    } else {
        error!("Not logged in. Please run 'collects login' first.");
        std::process::exit(1);
    }

    // 2. Prepare Input
    let mut attachments = Vec::new();
    for path in files {
        let filename = path
            .file_name()
            .context("Invalid filename")?
            .to_string_lossy()
            .to_string();

        // Simple mime guess
        let mime_type = mime_guess::from_path(&path).first_or_octet_stream().clone();

        let data = std::fs::read(&path).context(format!("Failed to read file: {path:?}"))?;

        attachments.push(Attachment {
            filename,
            mime_type: mime_type.to_string(),
            data,
        });
    }

    ctx.update::<CreateContentInput>(|s| {
        s.title = title;
        s.description = None;
        s.body = body;
        s.attachments = attachments;
    });

    // 3. Dispatch Command
    ctx.enqueue_command::<CreateContentCommand>();
    flush_and_await(&mut ctx).await;

    // 4. Check Result
    let compute = ctx.compute::<CreateContentCompute>();

    match &compute.status {
        ContentCreationStatus::Success(ids) => {
            info!("Content created successfully!");
            for id in ids {
                info!("ID: {id}");
            }
        }
        ContentCreationStatus::Error(e) => {
            error!("Error creating content: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        ContentCreationStatus::Idle | ContentCreationStatus::Uploading => {
            error!("Content creation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    ctx.shutdown().await;
    Ok(())
}
