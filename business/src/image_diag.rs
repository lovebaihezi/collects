//! Image paste and drag-and-drop diagnostics state.
//!
//! This module provides state for tracking and debugging image paste and
//! drag-and-drop operations across different environments and platforms.
//!
//! # Usage
//!
//! Toggle the diagnostic window with Shift+F2 key. The window displays:
//! - Key event detection (Ctrl+V / Cmd+V hotkey detection)
//! - Clipboard access logs (success/failure with detailed error info)
//! - Drop event logs (hover, drop, file info)
//! - Platform and environment information
//!
//! # State Update Pattern
//!
//! This uses the direct `state_ctx.update::<ImageDiagState>()` pattern for
//! synchronous UI-driven updates (toggle window, record events, clear history).
//! This avoids the complexity of Commands with parameters for simple state mutations.

use chrono::{DateTime, Utc};
use collects_states::{State, state_assign_impl};
use std::any::Any;

/// Maximum number of log entries to keep in history
const MAX_LOG_ENTRIES: usize = 50;

/// Type of key event detected
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyEventType {
    /// Ctrl+V detected (Windows/Linux)
    CtrlV,
    /// Cmd+V detected (macOS)
    CmdV,
    /// Key press event
    Press { key: String, modifiers: String },
    /// Key release event
    Release { key: String, modifiers: String },
}

impl std::fmt::Display for KeyEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KeyEventType::CtrlV => write!(f, "Ctrl+V"),
            KeyEventType::CmdV => write!(f, "Cmd+V"),
            KeyEventType::Press { key, modifiers } => write!(f, "Press: {}+{}", modifiers, key),
            KeyEventType::Release { key, modifiers } => write!(f, "Release: {}+{}", modifiers, key),
        }
    }
}

/// Result of clipboard access attempt
#[derive(Debug, Clone)]
pub enum ClipboardAccessResult {
    /// Successfully read image from clipboard
    ImageFound {
        width: usize,
        height: usize,
        bytes_len: usize,
        format: String,
    },
    /// Clipboard accessible but no image content
    NoImageContent,
    /// Clipboard contains text (possibly file URI)
    TextContent { preview: String, is_file_uri: bool },
    /// Failed to access clipboard
    AccessError(String),
    /// Platform not supported
    NotSupported,
}

/// Result of a paste operation (end-to-end)
#[derive(Debug, Clone)]
pub enum PasteResult {
    /// Successfully pasted an image
    Success {
        /// Width of the pasted image
        width: usize,
        /// Height of the pasted image
        height: usize,
        /// Size in bytes
        bytes_len: usize,
    },
    /// Failed to paste - no image content in clipboard
    NoImageContent,
    /// Failed to access clipboard
    AccessError(String),
    /// Failed to set image to preview state
    SetImageFailed { width: usize, height: usize },
}

/// Result of a drop operation
#[derive(Debug, Clone)]
pub enum DropResult {
    /// Successfully dropped an image
    Success {
        /// Name of the dropped file (if available)
        file_name: Option<String>,
        /// Width of the dropped image
        width: usize,
        /// Height of the dropped image
        height: usize,
        /// Size in bytes
        bytes_len: usize,
    },
    /// Dropped file is not a valid image
    InvalidImage {
        /// Name of the dropped file
        file_name: Option<String>,
        /// Error message
        error: String,
    },
    /// No valid files in drop
    NoValidFiles {
        /// Number of files dropped
        file_count: usize,
    },
    /// Failed to read dropped file
    ReadError {
        /// Name of the file
        file_name: Option<String>,
        /// Error message
        error: String,
    },
    /// Failed to set image to preview state
    SetImageFailed {
        file_name: Option<String>,
        width: usize,
        height: usize,
    },
}

/// Drop hover event information
#[derive(Debug, Clone)]
pub struct DropHoverEvent {
    /// Number of files being hovered
    pub file_count: usize,
    /// File names (if available)
    pub file_names: Vec<String>,
    /// MIME types (if available)
    pub mime_types: Vec<String>,
}

/// A single log entry for the diagnostic display
#[derive(Debug, Clone)]
pub struct DiagLogEntry {
    /// When the event occurred
    pub timestamp: DateTime<Utc>,
    /// The log entry type
    pub entry: DiagLogType,
}

/// Types of diagnostic log entries
#[derive(Debug, Clone)]
pub enum DiagLogType {
    /// Key event detected
    KeyEvent(KeyEventType),
    /// Clipboard access attempt
    ClipboardAccess(ClipboardAccessResult),
    /// Paste operation result
    PasteResult(PasteResult),
    /// Drop hover started
    DropHoverStart(DropHoverEvent),
    /// Drop hover ended
    DropHoverEnd,
    /// Drop operation result
    DropResult(DropResult),
    /// Generic info message
    Info(String),
    /// Warning message
    Warning(String),
    /// Error message
    Error(String),
}

/// A single paste operation entry (for summary stats)
#[derive(Debug, Clone)]
pub struct PasteEntry {
    /// When the paste operation occurred
    pub timestamp: DateTime<Utc>,
    /// Result of the paste operation
    pub result: PasteResult,
}

/// A single drop operation entry (for summary stats)
#[derive(Debug, Clone)]
pub struct DropEntry {
    /// When the drop operation occurred
    pub timestamp: DateTime<Utc>,
    /// Result of the drop operation
    pub result: DropResult,
}

/// State for image paste/drop diagnostics
#[derive(Debug, Clone, Default)]
pub struct ImageDiagState {
    /// Whether to show the diagnostic window (toggled by F2 key)
    show_window: bool,
    /// Unified log of all diagnostic events
    log_entries: Vec<DiagLogEntry>,
    /// Recent paste operations (for summary)
    paste_history: Vec<PasteEntry>,
    /// Recent drop operations (for summary)
    drop_history: Vec<DropEntry>,
    /// Total key events detected since app start
    total_key_events: usize,
    /// Total paste attempts since app start
    total_paste_attempts: usize,
    /// Total successful pastes since app start
    total_paste_successes: usize,
    /// Total drop attempts since app start
    total_drop_attempts: usize,
    /// Total successful drops since app start
    total_drop_successes: usize,
    /// Whether currently hovering with files
    is_hovering: bool,
}

impl ImageDiagState {
    /// Create a new empty diagnostic state
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether the diagnostic window should be shown
    pub fn show_window(&self) -> bool {
        self.show_window
    }

    /// Toggle the diagnostic window visibility
    pub fn toggle_window(&mut self) {
        self.show_window = !self.show_window;
    }

    /// Set the diagnostic window visibility
    pub fn set_show_window(&mut self, show: bool) {
        self.show_window = show;
    }

    /// Record a key event
    pub fn record_key_event(&mut self, event_type: KeyEventType) {
        self.total_key_events += 1;
        self.add_log_entry(DiagLogType::KeyEvent(event_type));
    }

    /// Record a clipboard access attempt
    pub fn record_clipboard_access(&mut self, result: ClipboardAccessResult) {
        self.add_log_entry(DiagLogType::ClipboardAccess(result));
    }

    /// Record a paste operation result
    pub fn record_paste(&mut self, result: PasteResult) {
        self.total_paste_attempts += 1;
        if matches!(result, PasteResult::Success { .. }) {
            self.total_paste_successes += 1;
        }

        self.add_log_entry(DiagLogType::PasteResult(result.clone()));

        self.paste_history.push(PasteEntry {
            timestamp: Utc::now(),
            result,
        });

        // Keep only recent entries in summary
        if self.paste_history.len() > 10 {
            self.paste_history.remove(0);
        }
    }

    /// Record drop hover start
    pub fn record_drop_hover_start(&mut self, event: DropHoverEvent) {
        self.is_hovering = true;
        self.add_log_entry(DiagLogType::DropHoverStart(event));
    }

    /// Record drop hover end
    pub fn record_drop_hover_end(&mut self) {
        if self.is_hovering {
            self.is_hovering = false;
            self.add_log_entry(DiagLogType::DropHoverEnd);
        }
    }

    /// Record a drop operation result
    pub fn record_drop(&mut self, result: DropResult) {
        self.is_hovering = false;
        self.total_drop_attempts += 1;
        if matches!(result, DropResult::Success { .. }) {
            self.total_drop_successes += 1;
        }

        self.add_log_entry(DiagLogType::DropResult(result.clone()));

        self.drop_history.push(DropEntry {
            timestamp: Utc::now(),
            result,
        });

        // Keep only recent entries in summary
        if self.drop_history.len() > 10 {
            self.drop_history.remove(0);
        }
    }

    /// Record an info message
    pub fn record_info(&mut self, message: impl Into<String>) {
        self.add_log_entry(DiagLogType::Info(message.into()));
    }

    /// Record a warning message
    pub fn record_warning(&mut self, message: impl Into<String>) {
        self.add_log_entry(DiagLogType::Warning(message.into()));
    }

    /// Record an error message
    pub fn record_error(&mut self, message: impl Into<String>) {
        self.add_log_entry(DiagLogType::Error(message.into()));
    }

    /// Add a log entry with current timestamp
    fn add_log_entry(&mut self, entry: DiagLogType) {
        self.log_entries.push(DiagLogEntry {
            timestamp: Utc::now(),
            entry,
        });

        // Keep only the most recent entries
        if self.log_entries.len() > MAX_LOG_ENTRIES {
            self.log_entries.remove(0);
        }
    }

    /// Get all log entries (most recent first)
    pub fn log_entries(&self) -> impl Iterator<Item = &DiagLogEntry> {
        self.log_entries.iter().rev()
    }

    /// Get paste history (most recent first)
    pub fn paste_history(&self) -> impl Iterator<Item = &PasteEntry> {
        self.paste_history.iter().rev()
    }

    /// Get drop history (most recent first)
    pub fn drop_history(&self) -> impl Iterator<Item = &DropEntry> {
        self.drop_history.iter().rev()
    }

    /// Get total key events detected
    pub fn total_key_events(&self) -> usize {
        self.total_key_events
    }

    /// Get total paste attempts
    pub fn total_paste_attempts(&self) -> usize {
        self.total_paste_attempts
    }

    /// Get total successful pastes
    pub fn total_paste_successes(&self) -> usize {
        self.total_paste_successes
    }

    /// Get total drop attempts
    pub fn total_drop_attempts(&self) -> usize {
        self.total_drop_attempts
    }

    /// Get total successful drops
    pub fn total_drop_successes(&self) -> usize {
        self.total_drop_successes
    }

    /// Whether currently hovering with files
    pub fn is_hovering(&self) -> bool {
        self.is_hovering
    }

    /// Clear all history and logs
    pub fn clear_history(&mut self) {
        self.log_entries.clear();
        self.paste_history.clear();
        self.drop_history.clear();
    }

    /// Get platform info string (static, compile-time)
    pub fn platform_info() -> &'static str {
        #[cfg(target_os = "windows")]
        {
            "Windows (Win32 Clipboard API)"
        }
        #[cfg(target_os = "macos")]
        {
            "macOS (NSPasteboard)"
        }
        #[cfg(all(target_os = "linux", not(target_arch = "wasm32")))]
        {
            "Linux"
        }
        #[cfg(target_arch = "wasm32")]
        {
            "Web (WASM) - Limited support"
        }
        #[cfg(not(any(
            target_os = "windows",
            target_os = "macos",
            target_os = "linux",
            target_arch = "wasm32"
        )))]
        {
            "Unknown platform"
        }
    }

    /// Get Linux display server info (runtime detection)
    ///
    /// Returns information about whether the app is running on X11, Wayland, or unknown.
    /// This is determined by checking environment variables at runtime.
    #[cfg(all(target_os = "linux", not(target_arch = "wasm32")))]
    pub fn linux_display_server_info() -> String {
        let wayland_display = std::env::var("WAYLAND_DISPLAY").ok();
        let x11_display = std::env::var("DISPLAY").ok();
        let session_type = std::env::var("XDG_SESSION_TYPE").ok();

        let mut parts = Vec::new();

        // Determine display server
        let server = if wayland_display.is_some() {
            "Wayland"
        } else if x11_display.is_some() {
            "X11"
        } else {
            "Unknown"
        };
        parts.push(format!("Display: {}", server));

        // Add session type if available
        if let Some(st) = &session_type {
            parts.push(format!("Session: {}", st));
        }

        // Add specific display values for debugging
        if let Some(wd) = &wayland_display {
            parts.push(format!("WAYLAND_DISPLAY={}", wd));
        }
        if let Some(xd) = &x11_display {
            parts.push(format!("DISPLAY={}", xd));
        }

        parts.join(" | ")
    }

    /// Get Linux display server info (non-Linux stub)
    #[cfg(not(all(target_os = "linux", not(target_arch = "wasm32"))))]
    pub fn linux_display_server_info() -> String {
        "N/A (not Linux)".to_string()
    }

    /// Get environment info string
    pub fn env_info() -> &'static str {
        #[cfg(feature = "env_test_internal")]
        {
            "test-internal"
        }
        #[cfg(all(feature = "env_internal", not(feature = "env_test_internal")))]
        {
            "internal"
        }
        #[cfg(all(
            feature = "env_test",
            not(feature = "env_internal"),
            not(feature = "env_test_internal")
        ))]
        {
            "test"
        }
        #[cfg(all(
            feature = "env_nightly",
            not(feature = "env_internal"),
            not(feature = "env_test_internal"),
            not(feature = "env_test")
        ))]
        {
            "nightly"
        }
        #[cfg(all(
            feature = "env_pr",
            not(feature = "env_internal"),
            not(feature = "env_test_internal"),
            not(feature = "env_test"),
            not(feature = "env_nightly")
        ))]
        {
            "pr"
        }
        #[cfg(not(any(
            feature = "env_test",
            feature = "env_test_internal",
            feature = "env_internal",
            feature = "env_nightly",
            feature = "env_pr"
        )))]
        {
            "production"
        }
    }
}

impl State for ImageDiagState {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        state_assign_impl(self, new_self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_diag_state_default() {
        let state = ImageDiagState::default();
        assert!(!state.show_window());
        assert_eq!(state.total_key_events(), 0);
        assert_eq!(state.total_paste_attempts(), 0);
        assert_eq!(state.total_paste_successes(), 0);
        assert_eq!(state.total_drop_attempts(), 0);
        assert_eq!(state.total_drop_successes(), 0);
        assert!(!state.is_hovering());
    }

    #[test]
    fn test_toggle_window() {
        let mut state = ImageDiagState::default();
        assert!(!state.show_window());

        state.toggle_window();
        assert!(state.show_window());

        state.toggle_window();
        assert!(!state.show_window());
    }

    #[test]
    fn test_record_key_event() {
        let mut state = ImageDiagState::default();
        state.record_key_event(KeyEventType::CtrlV);

        assert_eq!(state.total_key_events(), 1);
        assert_eq!(state.log_entries.len(), 1);

        if let DiagLogType::KeyEvent(ref ev) = state.log_entries[0].entry {
            assert_eq!(*ev, KeyEventType::CtrlV);
        } else {
            panic!("Expected KeyEvent");
        }
    }

    #[test]
    fn test_record_clipboard_access() {
        let mut state = ImageDiagState::default();
        state.record_clipboard_access(ClipboardAccessResult::ImageFound {
            width: 100,
            height: 100,
            bytes_len: 40000,
            format: "RGBA".to_string(),
        });

        assert_eq!(state.log_entries.len(), 1);
    }

    #[test]
    fn test_record_paste_success() {
        let mut state = ImageDiagState::default();
        state.record_paste(PasteResult::Success {
            width: 100,
            height: 100,
            bytes_len: 40000,
        });

        assert_eq!(state.total_paste_attempts(), 1);
        assert_eq!(state.total_paste_successes(), 1);
        assert_eq!(state.paste_history.len(), 1);
        assert_eq!(state.log_entries.len(), 1);
    }

    #[test]
    fn test_record_paste_failure() {
        let mut state = ImageDiagState::default();
        state.record_paste(PasteResult::NoImageContent);

        assert_eq!(state.total_paste_attempts(), 1);
        assert_eq!(state.total_paste_successes(), 0);
    }

    #[test]
    fn test_record_drop_success() {
        let mut state = ImageDiagState::default();
        state.record_drop(DropResult::Success {
            file_name: Some("test.png".to_string()),
            width: 200,
            height: 200,
            bytes_len: 160000,
        });

        assert_eq!(state.total_drop_attempts(), 1);
        assert_eq!(state.total_drop_successes(), 1);
        assert_eq!(state.drop_history.len(), 1);
    }

    #[test]
    fn test_drop_hover_tracking() {
        let mut state = ImageDiagState::default();
        assert!(!state.is_hovering());

        state.record_drop_hover_start(DropHoverEvent {
            file_count: 1,
            file_names: vec!["test.png".to_string()],
            mime_types: vec!["image/png".to_string()],
        });
        assert!(state.is_hovering());

        state.record_drop_hover_end();
        assert!(!state.is_hovering());
    }

    #[test]
    fn test_log_limit() {
        let mut state = ImageDiagState::default();

        // Add more than MAX_LOG_ENTRIES
        for i in 0..60 {
            state.record_info(format!("Test log {}", i));
        }

        assert_eq!(state.log_entries.len(), MAX_LOG_ENTRIES);
    }

    #[test]
    fn test_clear_history() {
        let mut state = ImageDiagState::default();
        state.record_key_event(KeyEventType::CtrlV);
        state.record_paste(PasteResult::NoImageContent);
        state.record_drop(DropResult::NoValidFiles { file_count: 0 });

        state.clear_history();

        assert_eq!(state.log_entries.len(), 0);
        assert_eq!(state.paste_history.len(), 0);
        assert_eq!(state.drop_history.len(), 0);
        // Totals are preserved
        assert_eq!(state.total_key_events(), 1);
        assert_eq!(state.total_paste_attempts(), 1);
        assert_eq!(state.total_drop_attempts(), 1);
    }

    #[test]
    fn test_platform_info() {
        let info = ImageDiagState::platform_info();
        assert!(!info.is_empty());
    }

    #[test]
    fn test_env_info() {
        let info = ImageDiagState::env_info();
        assert!(!info.is_empty());
    }

    #[test]
    fn test_linux_display_server_info() {
        // This function should return a non-empty string on all platforms
        let info = ImageDiagState::linux_display_server_info();
        assert!(!info.is_empty());

        // On Linux, it should contain "Display:" prefix
        #[cfg(target_os = "linux")]
        {
            assert!(
                info.contains("Display:"),
                "Linux display server info should contain 'Display:' but got: {}",
                info
            );
        }

        // On non-Linux, it should indicate N/A
        #[cfg(not(target_os = "linux"))]
        {
            assert!(
                info.contains("N/A"),
                "Non-Linux should return N/A but got: {}",
                info
            );
        }
    }

    #[test]
    fn test_key_event_type_display() {
        assert_eq!(format!("{}", KeyEventType::CtrlV), "Ctrl+V");
        assert_eq!(format!("{}", KeyEventType::CmdV), "Cmd+V");
        assert_eq!(
            format!(
                "{}",
                KeyEventType::Press {
                    key: "V".to_string(),
                    modifiers: "Ctrl".to_string()
                }
            ),
            "Press: Ctrl+V"
        );
    }
}
