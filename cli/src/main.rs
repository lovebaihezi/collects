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
    GetContentCompute, GetContentInput, GetContentStatus, GetGroupContentsCommand,
    GetGroupContentsCompute, GetGroupContentsInput, GetGroupContentsStatus, GroupItem,
    ListGroupsCommand, ListGroupsCompute, ListGroupsInput, ListGroupsStatus, LoginCommand,
    LoginInput, PendingTokenValidation, ValidateTokenCommand,
};
use collects_clipboard::{ClipboardProvider as _, SystemClipboard};
use collects_states::StateCtx;
use dirs::home_dir;
use inquire::{Confirm, Select, Text};
use serde::{Deserialize, Serialize};
use tabled::settings::Style;
use tabled::{Table, Tabled};
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
    // TODO: Add `New` command to create a new collect (group)
    // collects new -t "My Collect" → creates an empty collect
    // collects new -t "My Collect" -f file.png → creates collect with files
    //
    // TODO: Add `Add` command to add content to an existing collect
    // collects add <collect_id> -f file.png → adds file to collect
    // collects add <collect_id> --stdin → adds text content from stdin
    /// Create new content (note: creates orphan content, not a collect)
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

        /// Skip clipboard image reading
        #[arg(long)]
        no_clipboard: bool,
    },
    /// Show what can be added to collects (schema information)
    Schema,
    // TODO: Rename to `Contents` or remove once `New` command exists
    // Users should use `collects new` to create collects, not `collects create`
    /// List your collects (groups)
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

        /// Interactive mode (select collect to view)
        #[arg(long, short = 'I')]
        interactive: bool,
    },
    /// View a collect (group) and its files
    View {
        /// Collect ID (UUID)
        id: Option<String>,
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

    // List groups (collects) states and computes
    ctx.add_state(ListGroupsInput::default());
    ctx.record_compute(ListGroupsCompute::default());

    // Get content states and computes
    ctx.add_state(GetContentInput::default());
    ctx.record_compute(GetContentCompute::default());

    // Get group contents states and computes
    ctx.add_state(GetGroupContentsInput::default());
    ctx.record_compute(GetGroupContentsCompute::default());

    // Commands
    ctx.record_command(LoginCommand);
    ctx.record_command(ValidateTokenCommand);
    ctx.record_command(CreateContentCommand);
    ctx.record_command(ListGroupsCommand);
    ctx.record_command(GetGroupContentsCommand);
    ctx.record_command(GetContentCommand);

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

/// Ensures the user is authenticated, prompting for login if needed.
///
/// This function:
/// 1. Tries to restore session from saved token
/// 2. If token is expired/invalid, prompts the user to login interactively
/// 3. Returns Ok(()) if authentication succeeds, or exits on failure
#[instrument(skip_all, name = "ensure_authenticated")]
async fn ensure_authenticated(ctx: &mut StateCtx) -> Result<()> {
    // First try to restore existing session
    if restore_session(ctx).await? {
        return Ok(());
    }

    // Session restoration failed - prompt for login
    eprintln!("⚠ Session expired or not logged in. Please login to continue.\n");

    // Check if we're in a terminal that can accept input
    if !std::io::stdin().is_terminal() {
        eprintln!("✗ Cannot prompt for login: stdin is not a terminal.");
        eprintln!("  Please run 'collects login' first.");
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
            println!("✓ Logged in as {u}\n");
            if let Some(t) = token {
                save_token(u, t)?;
            }
            Ok(())
        }
        AuthStatus::Failed(msg) => {
            error!("Login failed: {msg}");
            eprintln!("✗ Login failed: {msg}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        AuthStatus::NotAuthenticated | AuthStatus::Authenticating => {
            error!("Login did not complete");
            eprintln!("✗ Login did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
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
            no_clipboard,
        }) => {
            if interactive {
                run_create_interactive(ctx).await
            } else {
                run_create(ctx, file, title, !no_clipboard).await
            }
        }
        Some(Commands::Schema) => {
            print_schema();
            Ok(())
        }
        Some(Commands::List {
            limit,
            offset,
            status,
            interactive,
        }) => run_list(ctx, limit, offset, status, interactive).await,
        Some(Commands::View { id }) => run_view(ctx, id).await,
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

            // Always try to read clipboard image
            let clipboard_image = read_clipboard_image_if_available();

            if files.is_empty() && body.is_none() && clipboard_image.is_none() {
                use clap::CommandFactory as _;
                Cli::command().print_help()?;
                return Ok(());
            }

            run_create_impl(ctx, files, cli.title, body, clipboard_image).await
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
    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

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
async fn run_create(
    ctx: StateCtx,
    files: Vec<PathBuf>,
    title: Option<String>,
    read_clipboard: bool,
) -> Result<()> {
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

    // Read clipboard image if enabled
    let clipboard_image = if read_clipboard {
        read_clipboard_image_if_available()
    } else {
        None
    };

    run_create_impl(ctx, files, title, body, clipboard_image).await
}

/// Attempts to read an image from the clipboard, returning None on failure.
fn read_clipboard_image_if_available() -> Option<Attachment> {
    let clipboard = SystemClipboard;
    match clipboard.get_image() {
        Ok(Some(image)) => {
            info!(
                "Clipboard image found: {}x{} ({})",
                image.width, image.height, image.filename
            );
            Some(Attachment {
                filename: image.filename,
                mime_type: image.mime_type,
                data: image.bytes,
            })
        }
        Ok(None) => {
            log::debug!("No image in clipboard");
            None
        }
        Err(e) => {
            log::debug!("Failed to read clipboard: {e}");
            None
        }
    }
}

/// Prints schema information about what can be added to collects.
fn print_schema() {
    println!("Collects Content Schema");
    println!("========================\n");

    println!("When creating content, you can provide:\n");

    println!("  TITLE (optional)");
    println!("    A short title for the content.");
    println!("    Flag: --title, -t <TEXT>\n");

    println!("  DESCRIPTION (optional)");
    println!("    A longer description (interactive mode only).\n");

    println!("  BODY (optional)");
    println!("    Text content, provided via:");
    println!("    - stdin: echo 'content' | collects create");
    println!("    - Interactive mode: opens $EDITOR or prompts for input\n");

    println!("  ATTACHMENTS (optional)");
    println!("    Files to upload with the content:");
    println!("    - File flag: --file, -f <PATH> (can be repeated)");
    println!("    - Clipboard: Images in clipboard are automatically attached");
    println!("    - Skip clipboard: --no-clipboard\n");

    println!("Supported attachment types:");
    println!("  - Images: PNG, JPEG, GIF, BMP, WebP, TIFF, ICO");
    println!("  - Documents: PDF, TXT, MD, and other text files");
    println!("  - Any other file type (stored as application/octet-stream)\n");

    println!("Examples:");
    println!("  # Create text note from stdin");
    println!("  echo 'My note content' | collects create -t 'My Note'\n");

    println!("  # Upload a file");
    println!("  collects create -f image.png -t 'Screenshot'\n");

    println!("  # Paste from clipboard (image)");
    println!("  collects create -t 'Clipboard image'\n");

    println!("  # Interactive mode");
    println!("  collects create -I\n");

    println!("  # Multiple files");
    println!("  collects create -f file1.txt -f file2.png -t 'Multiple files'");
}

#[instrument(skip_all, name = "create_impl", fields(file_count = files.len(), has_body = body.is_some(), has_clipboard = clipboard_image.is_some()))]
async fn run_create_impl(
    mut ctx: StateCtx,
    files: Vec<PathBuf>,
    title: Option<String>,
    body: Option<String>,
    clipboard_image: Option<Attachment>,
) -> Result<()> {
    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

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

    // Add clipboard image if available
    if let Some(clip_attachment) = clipboard_image {
        println!(
            "📋 Adding clipboard image: {} ({})",
            clip_attachment.filename, clip_attachment.mime_type
        );
        attachments.push(clip_attachment);
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

#[allow(clippy::too_many_lines)]
#[instrument(skip_all, name = "list", fields(limit, offset, status = status.as_deref().unwrap_or("all")))]
async fn run_list(
    mut ctx: StateCtx,
    limit: i32,
    offset: i32,
    status: Option<String>,
    interactive: bool,
) -> Result<()> {
    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

    ctx.update::<ListGroupsInput>(|s| {
        s.limit = Some(limit);
        s.offset = Some(offset);
        s.status = status.map(|st| Ustr::from(&st));
    });

    ctx.enqueue_command::<ListGroupsCommand>();
    flush_and_await(&mut ctx).await;

    let compute = ctx.compute::<ListGroupsCompute>();
    let groups = match &compute.status {
        ListGroupsStatus::Success(groups) => groups.clone(),
        ListGroupsStatus::Error(e) => {
            eprintln!("✗ Error listing collects: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("✗ List operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    if groups.is_empty() {
        println!("No collects found.");
        ctx.shutdown().await;
        return Ok(());
    }

    // Fetch file counts for each group (Option 2: N+1 calls)
    let mut group_file_counts: Vec<(GroupItem, usize)> = Vec::new();
    for group in &groups {
        ctx.update::<GetGroupContentsInput>(|s| {
            s.group_id = Some(group.id);
        });
        ctx.enqueue_command::<GetGroupContentsCommand>();
        flush_and_await(&mut ctx).await;

        let contents_compute = ctx.compute::<GetGroupContentsCompute>();
        let file_count = match &contents_compute.status {
            GetGroupContentsStatus::Success(items) => items.len(),
            _ => 0,
        };
        group_file_counts.push((group.clone(), file_count));
    }

    if interactive {
        // Interactive mode: let user select collect to view
        let options: Vec<String> = group_file_counts
            .iter()
            .map(|(group, file_count)| {
                format!("📁 {} ({} files) [{}]", group.name, file_count, group.id)
            })
            .collect();

        let selection = Select::new("Select collect to view:", options)
            .with_help_message("Use arrow keys to navigate, Enter to select")
            .prompt_skippable()
            .context("Failed to select collect")?;

        if let Some(selected) = selection {
            // Extract ID from the selected string
            if let Some(id_start) = selected.rfind('[') {
                let id = &selected[id_start + 1..selected.len() - 1];
                ctx.shutdown().await;
                // Re-create context for view
                let new_ctx = build_state_ctx(BusinessConfig::default());
                return run_view(new_ctx, Some(id.to_owned())).await;
            }
        }
    } else {
        // Non-interactive: just print the list using tabled
        #[derive(Tabled)]
        struct ListRow {
            #[tabled(rename = "ID")]
            id: String,
            #[tabled(rename = "Name")]
            name: String,
            #[tabled(rename = "Description")]
            description: String,
            #[tabled(rename = "Files")]
            file_count: usize,
            #[tabled(rename = "Status")]
            status: String,
        }

        fn truncate_str(s: &str, max_len: usize) -> String {
            if s.chars().count() > max_len {
                let truncated: String = s.chars().take(max_len - 3).collect();
                format!("{truncated}...")
            } else {
                s.to_owned()
            }
        }

        let rows: Vec<ListRow> = group_file_counts
            .iter()
            .map(|(group, file_count)| ListRow {
                id: group.id.to_string(),
                name: truncate_str(group.name.as_str(), 24),
                description: group
                    .description
                    .as_ref()
                    .map(|d| truncate_str(d.as_str(), 24))
                    .unwrap_or_default(),
                file_count: *file_count,
                status: group.status.to_string(),
            })
            .collect();

        let mut table = Table::new(&rows);
        table.with(Style::rounded());
        println!("\n{table}");
        println!("\nTotal: {} collect(s)", groups.len());
    }

    ctx.shutdown().await;
    Ok(())
}

#[instrument(skip_all, name = "view", fields(collect_id = id.as_deref().unwrap_or("interactive")))]
async fn run_view(mut ctx: StateCtx, id: Option<String>) -> Result<()> {
    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

    // Get ID interactively if not provided
    let group_id = match id {
        Some(id) => id,
        None => Text::new("Collect ID:")
            .with_help_message("Enter the UUID of the collect to view")
            .prompt()
            .context("Failed to read collect ID")?,
    };

    // Get contents in the group
    ctx.update::<GetGroupContentsInput>(|s| {
        s.group_id = Some(Ustr::from(&group_id));
    });

    ctx.enqueue_command::<GetGroupContentsCommand>();
    flush_and_await(&mut ctx).await;

    let contents_compute = ctx.compute::<GetGroupContentsCompute>();
    let items = match &contents_compute.status {
        GetGroupContentsStatus::Success(items) => items.clone(),
        GetGroupContentsStatus::NotFound => {
            eprintln!("✗ Collect not found: {group_id}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        GetGroupContentsStatus::Error(e) => {
            eprintln!("✗ Error getting collect: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("✗ Get collect operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    println!("\n📁 Collect: {}", group_id);
    println!("{}", "=".repeat(50));

    if items.is_empty() {
        println!("No files in this collect.");
        ctx.shutdown().await;
        return Ok(());
    }

    println!("Files: {} item(s)\n", items.len());

    // Fetch details for each content item
    for group_content in &items {
        ctx.update::<GetContentInput>(|s| {
            s.id = group_content.content_id;
        });

        ctx.enqueue_command::<GetContentCommand>();
        flush_and_await(&mut ctx).await;

        let content_compute = ctx.compute::<GetContentCompute>();
        match &content_compute.status {
            GetContentStatus::Success(item) => {
                print_content_summary(item);
            }
            GetContentStatus::NotFound => {
                println!("  ⚠️  {} (content not found)", group_content.content_id);
            }
            GetContentStatus::Error(e) => {
                println!("  ⚠️  {} (error: {})", group_content.content_id, e);
            }
            _ => {}
        }
    }

    ctx.shutdown().await;
    Ok(())
}

fn print_content_summary(item: &ContentItem) {
    let kind_icon = if item.is_file() { "📄" } else { "📝" };
    let size = format_size(item.file_size);

    println!(
        "  {} {} ({}) - {}",
        kind_icon, item.title, item.content_type, size
    );
    println!("     ID: {}", item.id);
    if let Some(desc) = &item.description {
        println!("     Description: {desc}");
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
