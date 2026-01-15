#![allow(clippy::exit)]

mod timing;

use std::io::{IsTerminal as _, Write as _};
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use clap::{CommandFactory as _, Parser, Subcommand};
use clap_complete::{Generator, Shell};
use collects_business::{
    Attachment, AuthCompute, AuthStatus, BusinessConfig, CFTokenCompute, ContentCreationStatus,
    ContentItem, CreateContentCommand, CreateContentCompute, CreateContentInput, GetContentCommand,
    GetContentCompute, GetContentInput, GetContentStatus, GetViewUrlCommand, GetViewUrlCompute,
    GetViewUrlInput, GetViewUrlStatus, ListContentsCommand, ListContentsCompute, ListContentsInput,
    ListContentsStatus, LoginCommand, LoginInput, PendingTokenValidation, ValidateTokenCommand,
};
use collects_states::StateCtx;
use dirs::home_dir;
use inquire::{Confirm, Password, Select, Text};
use serde::{Deserialize, Serialize};
use tracing::{error, info, instrument};
use ustr::Ustr;

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

    /// Show timing/latency information
    #[arg(long, global = true)]
    timing: bool,

    /// Enable verbose debug output
    #[arg(long, short = 'v', global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Login to Collects
    Login,
    /// Create new content interactively
    Create {
        /// Attach files
        #[arg(long, short = 'f')]
        file: Vec<PathBuf>,

        /// Title for the content
        #[arg(long, short = 't')]
        title: Option<String>,

        /// Interactive mode (prompt for all fields)
        #[arg(long, short = 'I')]
        interactive: bool,
    },
    /// List your contents
    List {
        /// Maximum number of items to return (1-100)
        #[arg(long, short = 'l', default_value = "20")]
        limit: i32,

        /// Offset for pagination
        #[arg(long, short = 'o', default_value = "0")]
        offset: i32,

        /// Filter by status: active, archived, trashed
        #[arg(long, short = 's')]
        status: Option<String>,

        /// Interactive mode (select content to view)
        #[arg(long, short = 'I')]
        interactive: bool,
    },
    /// View content by ID
    View {
        /// Content ID (UUID)
        id: Option<String>,

        /// Get download URL instead of inline view
        #[arg(long, short = 'd')]
        download: bool,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        #[arg(value_enum)]
        shell: Shell,
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

    // List contents states and computes
    ctx.add_state(ListContentsInput::default());
    ctx.record_compute(ListContentsCompute::default());

    // Get content states and computes
    ctx.add_state(GetContentInput::default());
    ctx.record_compute(GetContentCompute::default());

    // Get view URL states and computes
    ctx.add_state(GetViewUrlInput::default());
    ctx.record_compute(GetViewUrlCompute::default());

    // Commands
    ctx.record_command(LoginCommand);
    ctx.record_command(ValidateTokenCommand);
    ctx.record_command(CreateContentCommand);
    ctx.record_command(ListContentsCommand);
    ctx.record_command(GetContentCommand);
    ctx.record_command(GetViewUrlCommand);

    ctx
}

/// Await all pending tasks in the `JoinSet` and sync computes.
#[instrument(skip_all, name = "await_tasks")]
async fn await_pending_tasks(ctx: &mut StateCtx) {
    while ctx.task_count() > 0 {
        if ctx.task_set_mut().join_next().await.is_some() {
            ctx.sync_computes();
        }
    }
}

/// Flush commands and await all spawned tasks.
#[instrument(skip_all, name = "flush")]
async fn flush_and_await(ctx: &mut StateCtx) {
    ctx.sync_computes();
    ctx.flush_commands();
    await_pending_tasks(ctx).await;
    ctx.sync_computes();
}

/// Restore session from saved token
#[instrument(skip_all, name = "restore_session")]
async fn restore_session(ctx: &mut StateCtx) -> Result<bool> {
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing with timing support
    timing::init_tracing(cli.verbose, cli.timing);

    let ctx = build_state_ctx(BusinessConfig::default());

    match cli.command {
        Some(Commands::Login) => run_login(ctx).await,
        Some(Commands::Create {
            file,
            title,
            interactive,
        }) => {
            if interactive {
                run_create_interactive(ctx).await
            } else {
                run_create(ctx, file, title).await
            }
        }
        Some(Commands::List {
            limit,
            offset,
            status,
            interactive,
        }) => run_list(ctx, limit, offset, status, interactive).await,
        Some(Commands::View { id, download }) => run_view(ctx, id, download).await,
        Some(Commands::Completions { shell }) => {
            generate_completions(shell);
            Ok(())
        }
        None => {
            // Default: Create content
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
                use clap::CommandFactory as _;
                Cli::command().print_help()?;
                return Ok(());
            }

            run_create_impl(ctx, files, cli.title, body).await
        }
    }
}

fn generate_completions<G: Generator>(generator: G) {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_owned();
    clap_complete::generate(generator, &mut cmd, bin_name, &mut std::io::stdout());
    std::io::stdout().flush().ok();
}

#[instrument(skip_all, name = "login")]
async fn run_login(mut ctx: StateCtx) -> Result<()> {
    println!("Login to Collects\n");

    // Use inquire for better prompts
    let username = Text::new("Username:")
        .with_help_message("Enter your Collects username")
        .prompt()
        .context("Failed to read username")?;

    let otp = Password::new("OTP Code:")
        .with_display_mode(inquire::PasswordDisplayMode::Masked)
        .with_help_message("Enter the 6-digit code from your authenticator app")
        .without_confirmation()
        .prompt()
        .context("Failed to read OTP")?;

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
            println!("\n✓ Successfully logged in as {u}");
            if let Some(t) = token {
                save_token(u, t)?;
                println!("✓ Token saved to ~/.collects/token");
            }
        }
        AuthStatus::Failed(msg) => {
            error!("Login failed: {msg}");
            eprintln!("\n✗ Login failed: {msg}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        AuthStatus::NotAuthenticated | AuthStatus::Authenticating => {
            error!("Login did not complete");
            eprintln!("\n✗ Login did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    ctx.shutdown().await;
    Ok(())
}

#[instrument(skip_all, name = "create_interactive")]
#[allow(clippy::too_many_lines)]
async fn run_create_interactive(mut ctx: StateCtx) -> Result<()> {
    // Restore session first
    if !restore_session(&mut ctx).await? {
        eprintln!("✗ Not logged in. Please run 'collects login' first.");
        std::process::exit(1);
    }

    println!("Create New Content\n");

    // Title (optional)
    let title = Text::new("Title:")
        .with_help_message("Press Enter to skip")
        .prompt_skippable()
        .context("Failed to read title")?
        .filter(|s: &String| !s.trim().is_empty());

    // Description (optional)
    let description = Text::new("Description:")
        .with_help_message("Press Enter to skip")
        .prompt_skippable()
        .context("Failed to read description")?
        .filter(|s: &String| !s.trim().is_empty());

    // Content type selection
    let content_type = Select::new("Content type:", vec!["Text/Note", "File Upload", "Both"])
        .with_help_message("Select the type of content to create")
        .prompt()
        .context("Failed to select content type")?;

    let mut body = None;
    let mut attachments = Vec::new();

    // Handle text content
    if content_type == "Text/Note" || content_type == "Both" {
        let edit_method = Select::new(
            "How would you like to enter the text?",
            vec!["Open $EDITOR", "Type directly"],
        )
        .prompt()
        .context("Failed to select edit method")?;

        if edit_method == "Open $EDITOR" {
            // Use $EDITOR environment variable
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_owned());
            let temp_file = std::env::temp_dir().join("collects_content.txt");
            std::fs::write(&temp_file, "")?;

            let status = std::process::Command::new(&editor)
                .arg(&temp_file)
                .status()
                .context("Failed to open editor")?;

            if status.success() {
                let text = std::fs::read_to_string(&temp_file)?;
                std::fs::remove_file(&temp_file).ok();
                if !text.trim().is_empty() {
                    body = Some(text);
                }
            } else {
                std::fs::remove_file(&temp_file).ok();
                eprintln!("Editor exited with error");
            }
        } else {
            // Single line text input
            let text = Text::new("Content body:")
                .with_help_message("Enter text content (single line)")
                .prompt()
                .context("Failed to read content body")?;

            if !text.trim().is_empty() {
                body = Some(text);
            }
        }
    }

    // Handle file uploads
    if content_type == "File Upload" || content_type == "Both" {
        let file_path = Text::new("File path(s):")
            .with_help_message("Enter file path(s) separated by spaces")
            .prompt()
            .context("Failed to read file path")?;

        for path_str in file_path.split_whitespace() {
            let path = PathBuf::from(path_str);
            if path.exists() {
                let filename = path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "unnamed".to_owned());

                let mime_type = mime_guess::from_path(&path)
                    .first_or_octet_stream()
                    .to_string();

                match std::fs::read(&path) {
                    Ok(data) => {
                        attachments.push(Attachment {
                            filename,
                            mime_type,
                            data,
                        });
                    }
                    Err(e) => {
                        eprintln!("Warning: Could not read file {path_str}: {e}");
                    }
                }
            } else {
                eprintln!("Warning: File not found: {path_str}");
            }
        }
    }

    // Confirmation
    if body.is_none() && attachments.is_empty() {
        eprintln!("\n✗ No content to create (no text or files provided)");
        std::process::exit(1);
    }

    println!("\n--- Summary ---");
    if let Some(t) = &title {
        println!("Title: {t}");
    }
    if let Some(d) = &description {
        println!("Description: {d}");
    }
    if let Some(ref b) = body {
        println!("Body: {} characters", b.len());
    }
    if !attachments.is_empty() {
        println!("Attachments: {} file(s)", attachments.len());
        for a in &attachments {
            println!("  - {} ({})", a.filename, a.mime_type);
        }
    }

    let confirmed = Confirm::new("Create this content?")
        .with_default(true)
        .prompt()
        .context("Failed to confirm")?;

    if !confirmed {
        println!("Cancelled.");
        ctx.shutdown().await;
        return Ok(());
    }

    // Create the content
    ctx.update::<CreateContentInput>(|s| {
        s.title = title;
        s.description = description;
        s.body = body;
        s.attachments = attachments;
    });

    ctx.enqueue_command::<CreateContentCommand>();
    flush_and_await(&mut ctx).await;

    let compute = ctx.compute::<CreateContentCompute>();
    match &compute.status {
        ContentCreationStatus::Success(ids) => {
            println!("\n✓ Content created successfully!");
            for id in ids {
                println!("  ID: {id}");
            }
        }
        ContentCreationStatus::Error(e) => {
            eprintln!("\n✗ Error creating content: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("\n✗ Content creation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    ctx.shutdown().await;
    Ok(())
}

#[instrument(skip_all, name = "create", fields(file_count = files.len()))]
async fn run_create(ctx: StateCtx, files: Vec<PathBuf>, title: Option<String>) -> Result<()> {
    // Check for stdin
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

#[instrument(skip_all, name = "create_impl", fields(file_count = files.len(), has_body = body.is_some()))]
async fn run_create_impl(
    mut ctx: StateCtx,
    files: Vec<PathBuf>,
    title: Option<String>,
    body: Option<String>,
) -> Result<()> {
    // Restore session
    if !restore_session(&mut ctx).await? {
        eprintln!("✗ Not logged in. Please run 'collects login' first.");
        std::process::exit(1);
    }

    // Prepare attachments
    let mut attachments = Vec::new();
    for path in files {
        let filename = path
            .file_name()
            .context("Invalid filename")?
            .to_string_lossy()
            .to_string();

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

    ctx.enqueue_command::<CreateContentCommand>();
    flush_and_await(&mut ctx).await;

    let compute = ctx.compute::<CreateContentCompute>();
    match &compute.status {
        ContentCreationStatus::Success(ids) => {
            println!("✓ Content created successfully!");
            for id in ids {
                println!("  ID: {id}");
            }
        }
        ContentCreationStatus::Error(e) => {
            eprintln!("✗ Error creating content: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("✗ Content creation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    ctx.shutdown().await;
    Ok(())
}

#[instrument(skip_all, name = "list", fields(limit, offset, status))]
async fn run_list(
    mut ctx: StateCtx,
    limit: i32,
    offset: i32,
    status: Option<String>,
    interactive: bool,
) -> Result<()> {
    // Restore session
    if !restore_session(&mut ctx).await? {
        eprintln!("✗ Not logged in. Please run 'collects login' first.");
        std::process::exit(1);
    }

    ctx.update::<ListContentsInput>(|s| {
        s.limit = Some(limit);
        s.offset = Some(offset);
        s.status = status.map(|st| Ustr::from(&st));
    });

    ctx.enqueue_command::<ListContentsCommand>();
    flush_and_await(&mut ctx).await;

    let compute = ctx.compute::<ListContentsCompute>();
    match &compute.status {
        ListContentsStatus::Success(items) => {
            if items.is_empty() {
                println!("No contents found.");
                ctx.shutdown().await;
                return Ok(());
            }

            if interactive {
                // Interactive mode: let user select content to view
                let options: Vec<String> = items
                    .iter()
                    .map(|item| {
                        let kind_icon = if item.is_file() { "📄" } else { "📝" };
                        let size = format_size(item.file_size);
                        format!(
                            "{} {} ({}) - {} [{}]",
                            kind_icon, item.title, item.content_type, size, item.id
                        )
                    })
                    .collect();

                let selection = Select::new("Select content to view:", options)
                    .with_help_message("Use arrow keys to navigate, Enter to select")
                    .prompt_skippable()
                    .context("Failed to select content")?;

                if let Some(selected) = selection {
                    // Extract ID from the selected string
                    if let Some(id_start) = selected.rfind('[') {
                        let id = &selected[id_start + 1..selected.len() - 1];
                        ctx.shutdown().await;
                        // Re-create context for view
                        let new_ctx = build_state_ctx(BusinessConfig::default());
                        return run_view(new_ctx, Some(id.to_owned()), false).await;
                    }
                }
            } else {
                // Non-interactive: just print the list
                println!(
                    "\n{:<36}  {:<30}  {:<15}  {:<10}  Status",
                    "ID", "Title", "Type", "Size"
                );
                println!("{}", "-".repeat(100));

                for item in items {
                    let title = if item.title.len() > 28 {
                        format!("{}...", &item.title.as_str()[..25])
                    } else {
                        item.title.to_string()
                    };
                    let size = format_size(item.file_size);
                    let content_type = if item.content_type.len() > 13 {
                        format!("{}...", &item.content_type.as_str()[..10])
                    } else {
                        item.content_type.to_string()
                    };

                    println!(
                        "{:<36}  {:<30}  {:<15}  {:<10}  {}",
                        item.id, title, content_type, size, item.status
                    );
                }
                println!("\nTotal: {} item(s)", items.len());
            }
        }
        ListContentsStatus::Error(e) => {
            eprintln!("✗ Error listing contents: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("✗ List operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    ctx.shutdown().await;
    Ok(())
}

#[instrument(skip_all, name = "view", fields(content_id = id.as_deref().unwrap_or("interactive")))]
async fn run_view(mut ctx: StateCtx, id: Option<String>, download: bool) -> Result<()> {
    // Restore session
    if !restore_session(&mut ctx).await? {
        eprintln!("✗ Not logged in. Please run 'collects login' first.");
        std::process::exit(1);
    }

    // Get ID interactively if not provided
    let content_id = match id {
        Some(id) => id,
        None => Text::new("Content ID:")
            .with_help_message("Enter the UUID of the content to view")
            .prompt()
            .context("Failed to read content ID")?,
    };

    // First, get the content details
    ctx.update::<GetContentInput>(|s| {
        s.id = Ustr::from(&content_id);
    });

    ctx.enqueue_command::<GetContentCommand>();
    flush_and_await(&mut ctx).await;

    let get_compute = ctx.compute::<GetContentCompute>();
    match &get_compute.status {
        GetContentStatus::Success(item) => {
            print_content_details(item);

            // For files, get the view URL
            if item.is_file() {
                let disposition = if download {
                    Ustr::from("attachment")
                } else {
                    Ustr::from("inline")
                };

                ctx.update::<GetViewUrlInput>(|s| {
                    s.content_id = Ustr::from(&content_id);
                    s.disposition = disposition;
                });

                ctx.enqueue_command::<GetViewUrlCommand>();
                flush_and_await(&mut ctx).await;

                let url_compute = ctx.compute::<GetViewUrlCompute>();
                match &url_compute.status {
                    GetViewUrlStatus::Success(data) => {
                        println!("\n📎 View URL (expires at {}):", data.expires_at);
                        println!("   {}", data.url);
                    }
                    GetViewUrlStatus::NotFound => {
                        eprintln!("\n✗ Could not generate view URL: Content not found");
                    }
                    GetViewUrlStatus::Error(e) => {
                        eprintln!("\n✗ Could not generate view URL: {e}");
                    }
                    _ => {}
                }
            }
        }
        GetContentStatus::NotFound => {
            eprintln!("✗ Content not found: {content_id}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        GetContentStatus::Error(e) => {
            eprintln!("✗ Error getting content: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("✗ Get content operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    ctx.shutdown().await;
    Ok(())
}

fn print_content_details(item: &ContentItem) {
    let kind_icon = if item.is_file() { "📄" } else { "📝" };

    println!("\n{kind_icon} Content Details");
    println!("{}", "=".repeat(50));
    println!("ID:          {}", item.id);
    println!("Title:       {}", item.title);
    if let Some(desc) = &item.description {
        println!("Description: {desc}");
    }
    println!("Type:        {}", item.content_type);
    println!("Kind:        {}", item.kind);
    println!("Size:        {}", format_size(item.file_size));
    println!("Status:      {}", item.status);
    println!("Visibility:  {}", item.visibility);
    println!("Created:     {}", item.created_at);
    println!("Updated:     {}", item.updated_at);

    if let Some(body) = &item.body {
        println!("\n📝 Content Body:");
        println!("{}", "-".repeat(50));
        println!("{body}");
    }
}

fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
