//! Input sources for collects: clipboard, stdin, and other content ingestion.
//!
//! This crate provides unified abstractions for various input sources used by
//! the collects CLI and UI applications.
//!
//! # Modules
//!
//! - [`clipboard`]: System clipboard access for images
//! - [`stdin`]: Cross-platform stdin reading with proper EOF handling
//!
//! # Design Philosophy
//!
//! All input sources use trait-based abstractions for testability:
//! - Production implementations work with real system resources
//! - Mock implementations enable unit testing without side effects

pub mod clipboard;
pub mod stdin;

// Re-export commonly used types for convenience
#[cfg(not(target_arch = "wasm32"))]
pub use clipboard::clear_clipboard_image;
pub use clipboard::{ClipboardError, ClipboardImage, ClipboardProvider, SystemClipboard};

pub use stdin::{MockStdinReader, RealStdinReader, StdinReader};
