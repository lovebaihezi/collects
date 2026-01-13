#![allow(clippy::exit)]

use std::io::{IsTerminal as _, Read as _};
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context as _, Result};
use clap::{Parser, Subcommand};
use collects_business::{
    Attachment, AuthCompute, AuthStatus, BusinessConfig, CFTokenCompute, ContentCreationStatus,
    CreateContentCommand, CreateContentCompute, CreateContentInput, LoginCommand, LoginInput,
    PendingTokenValidation, ValidateTokenCommand,
};
use collects_states::StateCtx;
use dirs::home_dir;
use serde::{Deserialize, Serialize};
use tokio::time::sleep;

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

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    // Initialize StateCtx
    let mut ctx = StateCtx::new();

    // Initialize required states
    ctx.add_state(LoginInput::default());
    ctx.add_state(CreateContentInput::default());
    ctx.add_state(PendingTokenValidation::default());
    ctx.add_state(BusinessConfig::default());

    // Initialize required computes
    ctx.record_compute(CFTokenCompute::default());
    ctx.record_compute(AuthCompute::default());
    ctx.record_compute(CreateContentCompute::default());

    // Register commands
    ctx.record_command(LoginCommand);
    ctx.record_command(CreateContentCommand);
    ctx.record_command(ValidateTokenCommand);

    // Set BusinessConfig
    // Note: features in Cargo.toml determine the actual URL used by BusinessConfig::default()
    ctx.update::<BusinessConfig>(|s| {
        *s = BusinessConfig::default();
    });

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
                let mut buffer = String::new();
                std::io::stdin().read_to_string(&mut buffer)?;
                if !buffer.is_empty() {
                    body = Some(buffer);
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

    // Loop until authenticated or failed
    loop {
        ctx.sync_computes();
        ctx.flush_commands();
        ctx.sync_computes();

        let auth = ctx.compute::<AuthCompute>();
        match &auth.status {
            AuthStatus::Authenticated { username: u, token } => {
                println!("Successfully logged in as {u}");
                if let Some(t) = token {
                    save_token(u, t)?;
                    println!("Token saved.");
                }
                break;
            }
            AuthStatus::Failed(msg) => {
                eprintln!("Login failed: {msg}");
                std::process::exit(1);
            }
            AuthStatus::NotAuthenticated | AuthStatus::Authenticating => {
                // Waiting
            }
        }

        sleep(Duration::from_millis(50)).await;
    }

    Ok(())
}

async fn run_create(ctx: StateCtx, files: Vec<PathBuf>, title: Option<String>) -> Result<()> {
    // Check for stdin in this mode too?
    // Usually 'create' subcommand might not implicitly read stdin unless specified.
    // But let's support it if piped.
    let mut body = None;
    if !std::io::stdin().is_terminal() {
        let mut buffer = String::new();
        std::io::stdin().read_to_string(&mut buffer)?;
        if !buffer.is_empty() {
            body = Some(buffer);
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

        // Wait for validation
        let mut attempts = 0;
        loop {
            ctx.sync_computes();
            ctx.flush_commands();
            ctx.sync_computes();

            let auth = ctx.compute::<AuthCompute>();
            match &auth.status {
                AuthStatus::Authenticated { .. } => break,
                AuthStatus::NotAuthenticated if attempts > 20 => {
                    // Timeout/Failure after ~1s
                    eprintln!("Session expired or invalid. Please login again.");
                    std::process::exit(1);
                }
                AuthStatus::Failed(e) => {
                    eprintln!("Auth error: {e}");
                    std::process::exit(1);
                }
                _ => {}
            }
            sleep(Duration::from_millis(50)).await;
            attempts += 1;
        }
    } else {
        eprintln!("Not logged in. Please run 'collects login' first.");
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
        let mime_type = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string();

        let data = std::fs::read(&path).context(format!("Failed to read file: {path:?}"))?;

        attachments.push(Attachment {
            filename,
            mime_type,
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

    // 4. Wait for Result
    loop {
        ctx.sync_computes();
        ctx.flush_commands();
        ctx.sync_computes();

        let compute = ctx.compute::<CreateContentCompute>();

        match &compute.status {
            ContentCreationStatus::Idle | ContentCreationStatus::Uploading => {
                // Show progress?
            }
            ContentCreationStatus::Success(ids) => {
                println!("Content created successfully!");
                for id in ids {
                    println!("ID: {id}");
                }
                break;
            }
            ContentCreationStatus::Error(e) => {
                eprintln!("Error creating content: {e}");
                std::process::exit(1);
            }
        }
        sleep(Duration::from_millis(50)).await;
    }

    Ok(())
}
