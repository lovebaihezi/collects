#![allow(clippy::exit)]

mod timing;

use std::io::{IsTerminal as _, Write as _};
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use clap::{CommandFactory as _, Parser, Subcommand};
use clap_complete::{Generator, Shell};
use collects_business::{
    AddGroupContentsCommand, AddGroupContentsCompute, AddGroupContentsInput,
    AddGroupContentsStatus, Attachment, AuthCompute, AuthStatus, BusinessConfig, CFTokenCompute,
    ContentCreationStatus, ContentItem, CreateContentCommand, CreateContentCompute,
    CreateContentInput, CreateGroupCommand, CreateGroupCompute, CreateGroupInput,
    CreateGroupStatus, GetContentCommand, GetContentCompute, GetContentInput, GetContentStatus,
    GetGroupContentsCommand, GetGroupContentsCompute, GetGroupContentsInput,
    GetGroupContentsStatus, GroupItem, ListGroupsCommand, ListGroupsCompute, ListGroupsInput,
    ListGroupsStatus, LoginCommand, LoginInput, PendingTokenValidation, ValidateTokenCommand,
};
use collects_clipboard::{ClipboardProvider as _, SystemClipboard, clear_clipboard_image};
use collects_states::StateCtx;
use dirs::home_dir;
use inquire::{Select, Text};
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
    /// Create a new collect (group) with content
    New {
        /// Title for the collect
        #[arg(long, short = 't')]
        title: String,

        /// Attach files
        #[arg(long, short = 'f')]
        file: Vec<PathBuf>,

        /// Read text content from stdin
        #[arg(long)]
        stdin: bool,
    },
    /// Add content to an existing collect (group)
    Add {
        /// Collect ID (UUID)
        id: String,

        /// Attach files
        #[arg(long, short = 'f')]
        file: Vec<PathBuf>,

        /// Read text content from stdin
        #[arg(long)]
        stdin: bool,
    },

    /// Show what can be added to collects (schema information)
    Schema,
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

    // Group creation states and computes
    ctx.add_state(CreateGroupInput::default());
    ctx.record_compute(CreateGroupCompute::default());

    // Add-to-group states and computes
    ctx.add_state(AddGroupContentsInput::default());
    ctx.record_compute(AddGroupContentsCompute::default());

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
    ctx.record_command(CreateGroupCommand);
    ctx.record_command(AddGroupContentsCommand);
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
        Some(Commands::New { title, file, stdin }) => run_new(ctx, title, file, stdin).await,
        Some(Commands::Add { id, file, stdin }) => run_add(ctx, id, file, stdin).await,

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
            use clap::CommandFactory as _;
            Cli::command().print_help()?;
            Ok(())
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

    println!("When creating or adding content to a collect, you can provide:\n");

    println!("  TITLE/DESCRIPTION");
    println!("    Not settable via CLI; titles come from filenames or defaults.\n");

    println!("  BODY (optional)");
    println!("    Text content, provided via:");
    println!("    - stdin: echo 'content' | collects new -t 'My Collect' --stdin");
    println!("    - stdin (add): echo 'content' | collects add <collect_id> --stdin\n");

    println!("  ATTACHMENTS (optional)");
    println!("    Files to upload with the content:");
    println!("    - File flag: --file, -f <PATH> (can be repeated)");
    println!("    - Clipboard: Images in clipboard are automatically attached\n");

    println!("Examples:");
    println!("  # Create a collect with text from stdin");
    println!("  echo 'My note content' | collects new -t 'My Collect' --stdin\n");

    println!("  # Add a file to an existing collect");
    println!("  collects add <collect_id> -f image.png\n");

    println!("  # Paste from clipboard (image) into a new collect");
    println!("  collects new -t 'Clipboard image'\n");

    println!("  # Multiple files");
    println!("  collects new -t 'Multiple files' -f file1.txt -f file2.png");
}

#[instrument(skip_all, name = "create_contents", fields(has_body = body.is_some(), attachment_count = attachments.len()))]
async fn create_contents_for_inputs(
    ctx: &mut StateCtx,
    title: Option<String>,
    body: Option<String>,
    attachments: Vec<Attachment>,
) -> Result<Vec<String>> {
    ctx.update::<CreateContentInput>(|s| {
        s.title = title;
        s.description = None;
        s.body = body;
        s.attachments = attachments;
    });

    ctx.enqueue_command::<CreateContentCommand>();
    flush_and_await(ctx).await;

    let compute = ctx.compute::<CreateContentCompute>();
    match &compute.status {
        ContentCreationStatus::Success(ids) => Ok(ids.clone()),
        ContentCreationStatus::Error(e) => {
            Err(anyhow::anyhow!(format!("Error creating content: {e}")))
        }
        _ => Err(anyhow::anyhow!("Content creation did not complete")),
    }
}

#[instrument(skip_all, name = "new_collect", fields(title = %title, file_count = files.len(), stdin))]
async fn run_new(mut ctx: StateCtx, title: String, files: Vec<PathBuf>, stdin: bool) -> Result<()> {
    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

    let mut body = None;
    if stdin {
        eprintln!("Reading stdin... Press Ctrl+D to finish.");
        use std::io::Read as _;
        let mut buffer = Vec::new();
        std::io::stdin().read_to_end(&mut buffer)?;
        if !buffer.is_empty() {
            body = Some(String::from_utf8(buffer)?);
        }
    }

    let clipboard_image = read_clipboard_image_if_available();
    let had_clipboard = clipboard_image.is_some();

    if files.is_empty() && body.is_none() && clipboard_image.is_none() {
        eprintln!("✗ No content to add (no files, stdin, or clipboard image)");
        ctx.shutdown().await;
        std::process::exit(1);
    }

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

    if let Some(clip_attachment) = clipboard_image {
        println!(
            "📋 Adding clipboard image: {} ({})",
            clip_attachment.filename, clip_attachment.mime_type
        );
        attachments.push(clip_attachment);
    }

    ctx.update::<CreateGroupInput>(|s| {
        s.name = Some(title.clone());
        s.description = None;
        s.visibility = None;
    });

    ctx.enqueue_command::<CreateGroupCommand>();
    flush_and_await(&mut ctx).await;

    let group = match &ctx.compute::<CreateGroupCompute>().status {
        CreateGroupStatus::Success(group) => group.clone(),
        CreateGroupStatus::Error(e) => {
            eprintln!("✗ Error creating collect: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("✗ Collect creation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    let content_ids = match create_contents_for_inputs(&mut ctx, None, body, attachments).await {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("✗ {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    ctx.update::<AddGroupContentsInput>(|s| {
        s.group_id = Some(group.id);
        s.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
    });

    ctx.enqueue_command::<AddGroupContentsCommand>();
    flush_and_await(&mut ctx).await;

    match &ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            println!("✓ Collect created: {} ({added} item(s))", group.id);
        }
        AddGroupContentsStatus::Error(e) => {
            eprintln!("✗ Error adding content to collect: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("✗ Add-to-collect operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    if had_clipboard && let Err(e) = clear_clipboard_image() {
        log::debug!("Failed to clear clipboard: {e}");
    }

    ctx.shutdown().await;
    Ok(())
}

#[instrument(skip_all, name = "add_collect", fields(collect_id = id.as_str(), file_count = files.len(), stdin))]
async fn run_add(mut ctx: StateCtx, id: String, files: Vec<PathBuf>, stdin: bool) -> Result<()> {
    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

    let mut body = None;
    if stdin {
        eprintln!("Reading stdin... Press Ctrl+D to finish.");
        use std::io::Read as _;
        let mut buffer = Vec::new();
        std::io::stdin().read_to_end(&mut buffer)?;
        if !buffer.is_empty() {
            body = Some(String::from_utf8(buffer)?);
        }
    }

    let clipboard_image = read_clipboard_image_if_available();
    let had_clipboard = clipboard_image.is_some();

    if files.is_empty() && body.is_none() && clipboard_image.is_none() {
        eprintln!("✗ No content to add (no files, stdin, or clipboard image)");
        ctx.shutdown().await;
        std::process::exit(1);
    }

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

    if let Some(clip_attachment) = clipboard_image {
        println!(
            "📋 Adding clipboard image: {} ({})",
            clip_attachment.filename, clip_attachment.mime_type
        );
        attachments.push(clip_attachment);
    }

    let content_ids = match create_contents_for_inputs(&mut ctx, None, body, attachments).await {
        Ok(ids) => ids,
        Err(e) => {
            eprintln!("✗ {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    ctx.update::<AddGroupContentsInput>(|s| {
        s.group_id = Some(Ustr::from(&id));
        s.content_ids = content_ids.iter().map(|cid| Ustr::from(cid)).collect();
    });

    ctx.enqueue_command::<AddGroupContentsCommand>();
    flush_and_await(&mut ctx).await;

    match &ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            println!("✓ Added {added} item(s) to collect {id}");
        }
        AddGroupContentsStatus::Error(e) => {
            eprintln!("✗ Error adding content to collect: {e}");
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            eprintln!("✗ Add-to-collect operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    if had_clipboard && let Err(e) = clear_clipboard_image() {
        log::debug!("Failed to clear clipboard: {e}");
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
    if item.is_text()
        && let Some(body) = &item.body
    {
        let preview = truncate_preview(body, 120);
        if !preview.is_empty() {
            println!("     Preview: {preview}");
        }
    }
}

fn truncate_preview(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= max_len {
        return trimmed.to_owned();
    }
    let truncated: String = trimmed.chars().take(max_len - 1).collect();
    format!("{truncated}…")
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
