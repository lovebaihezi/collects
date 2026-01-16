#![allow(clippy::exit)]

mod auth;
mod cli;
mod commands;
mod context;
mod output;
mod timing;
mod utils;

use anyhow::Result;
use clap::CommandFactory as _;
use collects_business::BusinessConfig;

use cli::{Cli, Commands};
use commands::{
    generate_completions, print_schema, run_add, run_list, run_login, run_new, run_view,
};
use context::build_state_ctx;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = <Cli as clap::Parser>::parse();

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
            Cli::command().print_help()?;
            Ok(())
        }
    }
}
