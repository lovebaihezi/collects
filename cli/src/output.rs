//! Terminal output utilities for styled CLI output.
//!
//! This module provides a consistent interface for printing styled output
//! to the terminal, replacing direct `println!` calls with structured output.

use console::{Term, style};
use std::fmt::Display;

/// Terminal output helper for consistent styled output.
pub struct Output {
    term: Term,
}

impl Default for Output {
    fn default() -> Self {
        Self::new()
    }
}

impl Output {
    /// Create a new output helper writing to stdout.
    pub fn new() -> Self {
        Self {
            term: Term::stdout(),
        }
    }

    /// Print a success message with a green checkmark.
    pub fn success(&self, message: impl Display) {
        drop(
            self.term
                .write_line(&format!("{} {}", style("✓").green().bold(), message)),
        );
    }

    /// Print an error message with a red X.
    pub fn error(&self, message: impl Display) {
        drop(
            self.term
                .write_line(&format!("{} {}", style("✗").red().bold(), message)),
        );
    }

    /// Print a warning message with a yellow warning sign.
    pub fn warning(&self, message: impl Display) {
        drop(
            self.term
                .write_line(&format!("{} {}", style("⚠").yellow().bold(), message)),
        );
    }

    /// Print an info message with a blue info icon.
    pub fn info(&self, message: impl Display) {
        drop(
            self.term
                .write_line(&format!("{} {}", style("ℹ").blue().bold(), message)),
        );
    }

    /// Print a plain message without any prefix.
    pub fn print(&self, message: impl Display) {
        drop(self.term.write_line(&message.to_string()));
    }

    /// Print an empty line.
    pub fn newline(&self) {
        drop(self.term.write_line(""));
    }

    /// Print a header with emphasis.
    pub fn header(&self, message: impl Display) {
        drop(
            self.term
                .write_line(&style(message).bold().cyan().to_string()),
        );
    }

    /// Print a subheader.
    pub fn subheader(&self, message: impl Display) {
        drop(self.term.write_line(&style(message).bold().to_string()));
    }

    /// Print a divider line.
    pub fn divider(&self, width: usize) {
        drop(
            self.term
                .write_line(&style("─".repeat(width)).dim().to_string()),
        );
    }

    /// Print a labeled value with indentation.
    pub fn labeled_indent(&self, label: impl Display, value: impl Display, indent: usize) {
        let spaces = " ".repeat(indent);
        drop(
            self.term
                .write_line(&format!("{spaces}{}: {}", style(label).dim(), value)),
        );
    }

    /// Print a file item (📄 for files).
    pub fn file_item(&self, name: impl Display, details: impl Display, size: impl Display) {
        drop(self.term.write_line(&format!(
            "  {} {} ({}) - {}",
            style("📄").bold(),
            style(name).white().bold(),
            style(details).dim(),
            style(size).cyan()
        )));
    }

    /// Print a text item.
    pub fn text_item(&self, name: impl Display, details: impl Display, size: impl Display) {
        drop(self.term.write_line(&format!(
            "  {} {} ({}) - {}",
            style("📝").bold(),
            style(name).white().bold(),
            style(details).dim(),
            style(size).cyan()
        )));
    }

    /// Print a collect/folder header.
    pub fn collect_header(&self, id: impl Display) {
        drop(self.term.write_line(&format!(
            "\n{} Collect: {}",
            style("📁").bold(),
            style(id).cyan()
        )));
    }

    /// Print a clipboard notification.
    pub fn clipboard(&self, filename: impl Display, mime_type: impl Display) {
        drop(self.term.write_line(&format!(
            "{} Adding clipboard image: {} ({})",
            style("📋").bold(),
            style(filename).white(),
            style(mime_type).dim()
        )));
    }

    /// Print a dim/muted message.
    pub fn dim(&self, message: impl Display) {
        drop(self.term.write_line(&style(message).dim().to_string()));
    }

    /// Print a section title for schema/help output.
    pub fn section(&self, title: impl Display) {
        drop(
            self.term
                .write_line(&format!("  {}", style(title).yellow().bold())),
        );
    }

    /// Print section content with indentation.
    pub fn section_content(&self, content: impl Display) {
        drop(self.term.write_line(&format!("    {content}")));
    }

    /// Print an example command.
    pub fn example(&self, description: impl Display, command: impl Display) {
        drop(self.term.write_line(&format!(
            "  {} {}",
            style("#").dim(),
            style(description).dim()
        )));
        drop(
            self.term
                .write_line(&format!("  {}", style(command).green())),
        );
    }

    /// Print a count summary.
    pub fn count(&self, label: impl Display, count: usize) {
        drop(self.term.write_line(&format!(
            "{}: {} item(s)",
            style(label).dim(),
            style(count).cyan().bold()
        )));
    }

    /// Print a total summary line.
    pub fn total(&self, label: impl Display, count: usize) {
        drop(self.term.write_line(&format!(
            "\n{}: {}",
            style(label).bold(),
            style(format!("{count} collect(s)")).cyan()
        )));
    }
}
