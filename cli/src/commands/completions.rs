//! Shell completions generation command.

use std::io::Write as _;

use clap::CommandFactory as _;
use clap_complete::{Generator, Shell};

use crate::cli::Cli;

/// Generate shell completions for the specified shell.
pub fn generate_completions(shell: Shell) {
    print_completions(shell);
}

fn print_completions<G: Generator>(generator: G) {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_owned();
    clap_complete::generate(generator, &mut cmd, bin_name, &mut std::io::stdout());
    std::io::stdout().flush().ok();
}
