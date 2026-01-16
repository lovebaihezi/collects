use std::path::PathBuf;

use clap::{Parser, Subcommand};
use clap_complete::Shell;

#[derive(Parser)]
#[command(name = "collects")]
#[command(about = "CLI for Collects", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Show timing/latency information
    #[arg(long, global = true)]
    pub timing: bool,

    /// Enable verbose debug output
    #[arg(long, short = 'v', global = true)]
    pub verbose: bool,
}

#[derive(Subcommand)]
pub enum Commands {
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
