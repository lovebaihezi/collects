//! Image diagnostics state for debugging paste and drag-drop functionality.
//!
//! This module provides state for displaying diagnostic information about
//! image paste and drag-drop events across different environments.

use std::any::{Any, TypeId};

use chrono::{DateTime, Utc};
use collects_states::{Command, Compute, ComputeDeps, Dep, State, Updater, assign_impl};

/// Maximum number of events to keep in the diagnostic history.
const MAX_EVENTS: usize = 20;

/// Type of image event recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageEventType {
    /// Image was pasted from clipboard.
    Paste,
    /// Image was dropped via drag-and-drop.
    Drop,
}

impl std::fmt::Display for ImageEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageEventType::Paste => write!(f, "Paste"),
            ImageEventType::Drop => write!(f, "Drop"),
        }
    }
}

/// Result status of an image event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageEventStatus {
    /// Event was successful.
    Success,
    /// Event failed with an error message.
    Failed(String),
}

impl std::fmt::Display for ImageEventStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImageEventStatus::Success => write!(f, "✓ Success"),
            ImageEventStatus::Failed(err) => write!(f, "✗ {err}"),
        }
    }
}

/// A single image diagnostic event.
#[derive(Debug, Clone)]
pub struct ImageDiagEvent {
    /// When the event occurred.
    pub timestamp: DateTime<Utc>,
    /// Type of event (paste or drop).
    pub event_type: ImageEventType,
    /// Whether the event succeeded or failed.
    pub status: ImageEventStatus,
    /// Width of the image (if available).
    pub width: Option<usize>,
    /// Height of the image (if available).
    pub height: Option<usize>,
    /// Size in bytes (if available).
    pub bytes: Option<usize>,
}

impl ImageDiagEvent {
    /// Creates a new successful event.
    pub fn success(event_type: ImageEventType, width: usize, height: usize, bytes: usize) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            status: ImageEventStatus::Success,
            width: Some(width),
            height: Some(height),
            bytes: Some(bytes),
        }
    }

    /// Creates a new failed event.
    pub fn failed(event_type: ImageEventType, error: impl Into<String>) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            status: ImageEventStatus::Failed(error.into()),
            width: None,
            height: None,
            bytes: None,
        }
    }
}

/// State for image diagnostics.
///
/// This state stores diagnostic information about image paste and drag-drop
/// events for debugging purposes.
#[derive(Default, Debug)]
pub struct ImageDiagState {
    /// Whether the diagnostics window is visible.
    show_window: bool,
    /// History of image events (most recent first).
    events: Vec<ImageDiagEvent>,
}

impl ImageDiagState {
    /// Returns whether the diagnostics window should be shown.
    pub fn show_window(&self) -> bool {
        self.show_window
    }

    /// Returns the event history (most recent first).
    pub fn events(&self) -> &[ImageDiagEvent] {
        &self.events
    }

    /// Clears all events from history.
    pub fn clear_events(&mut self) {
        self.events.clear();
    }
}

impl Compute for ImageDiagState {
    fn deps(&self) -> ComputeDeps {
        // No automatic dependencies - this is updated only by commands
        const STATE_IDS: [TypeId; 0] = [];
        const COMPUTE_IDS: [TypeId; 0] = [];
        (&STATE_IDS, &COMPUTE_IDS)
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // No-op: this compute is only updated by commands
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any>) {
        assign_impl(self, new_self);
    }
}

impl State for ImageDiagState {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Command to toggle the image diagnostics window visibility.
///
/// Dispatch via `ctx.dispatch::<ToggleImageDiagCommand>()`.
#[derive(Default, Debug)]
pub struct ToggleImageDiagCommand;

impl Command for ToggleImageDiagCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let current = deps.get_compute_ref::<ImageDiagState>();
        let new_show_window = !current.show_window;

        updater.set(ImageDiagState {
            show_window: new_show_window,
            events: current.events.clone(),
        });
    }
}

/// Command to record a successful image event.
///
/// **Usage Pattern:**
/// 1. Create a new command instance with actual values: `RecordImageEventCommand { ... }`
/// 2. Register it with `ctx.record_command(command)`
/// 3. Dispatch with `ctx.dispatch::<RecordImageEventCommand>()`
///
/// The Default impl exists for initial StateCtx registration but should not
/// be dispatched directly - always register with actual event values first.
#[derive(Debug)]
pub struct RecordImageEventCommand {
    /// Type of event to record.
    pub event_type: ImageEventType,
    /// Width of the image.
    pub width: usize,
    /// Height of the image.
    pub height: usize,
    /// Size in bytes.
    pub bytes: usize,
}

impl Default for RecordImageEventCommand {
    /// Creates a placeholder instance for StateCtx registration.
    /// Do not dispatch this default - always register with actual values first.
    fn default() -> Self {
        Self {
            event_type: ImageEventType::Paste,
            width: 0,
            height: 0,
            bytes: 0,
        }
    }
}

impl Command for RecordImageEventCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let current = deps.get_compute_ref::<ImageDiagState>();

        let event =
            ImageDiagEvent::success(self.event_type.clone(), self.width, self.height, self.bytes);

        let mut events = vec![event];
        events.extend(current.events.iter().cloned());
        events.truncate(MAX_EVENTS);

        updater.set(ImageDiagState {
            show_window: current.show_window,
            events,
        });
    }
}

/// Command to record a failed image event.
///
/// **Usage Pattern:**
/// 1. Create a new command instance with actual values: `RecordImageErrorCommand { ... }`
/// 2. Register it with `ctx.record_command(command)`
/// 3. Dispatch with `ctx.dispatch::<RecordImageErrorCommand>()`
///
/// The Default impl exists for initial StateCtx registration but should not
/// be dispatched directly - always register with actual error values first.
#[derive(Debug)]
pub struct RecordImageErrorCommand {
    /// Type of event to record.
    pub event_type: ImageEventType,
    /// Error message describing what went wrong.
    pub error: String,
}

impl Default for RecordImageErrorCommand {
    /// Creates a placeholder instance for StateCtx registration.
    /// Do not dispatch this default - always register with actual values first.
    fn default() -> Self {
        Self {
            event_type: ImageEventType::Paste,
            error: String::new(),
        }
    }
}

impl Command for RecordImageErrorCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let current = deps.get_compute_ref::<ImageDiagState>();

        let event = ImageDiagEvent::failed(self.event_type.clone(), &self.error);

        let mut events = vec![event];
        events.extend(current.events.iter().cloned());
        events.truncate(MAX_EVENTS);

        updater.set(ImageDiagState {
            show_window: current.show_window,
            events,
        });
    }
}

/// Command to clear the image event history.
///
/// Dispatch via `ctx.dispatch::<ClearImageDiagCommand>()`.
#[derive(Default, Debug)]
pub struct ClearImageDiagCommand;

impl Command for ClearImageDiagCommand {
    fn run(&self, deps: Dep, updater: Updater) {
        let current = deps.get_compute_ref::<ImageDiagState>();

        updater.set(ImageDiagState {
            show_window: current.show_window,
            events: Vec::new(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_diag_state_default() {
        let state = ImageDiagState::default();
        assert!(!state.show_window());
        assert!(state.events().is_empty());
    }

    #[test]
    fn test_image_event_success() {
        let event = ImageDiagEvent::success(ImageEventType::Paste, 100, 200, 80000);
        assert_eq!(event.event_type, ImageEventType::Paste);
        assert!(matches!(event.status, ImageEventStatus::Success));
        assert_eq!(event.width, Some(100));
        assert_eq!(event.height, Some(200));
        assert_eq!(event.bytes, Some(80000));
    }

    #[test]
    fn test_image_event_failed() {
        let event = ImageDiagEvent::failed(ImageEventType::Drop, "Invalid format");
        assert_eq!(event.event_type, ImageEventType::Drop);
        assert!(matches!(event.status, ImageEventStatus::Failed(_)));
        assert!(event.width.is_none());
        assert!(event.height.is_none());
        assert!(event.bytes.is_none());
    }

    #[test]
    fn test_event_type_display() {
        assert_eq!(format!("{}", ImageEventType::Paste), "Paste");
        assert_eq!(format!("{}", ImageEventType::Drop), "Drop");
    }

    #[test]
    fn test_event_status_display() {
        assert_eq!(format!("{}", ImageEventStatus::Success), "✓ Success");
        assert_eq!(
            format!("{}", ImageEventStatus::Failed("error".to_string())),
            "✗ error"
        );
    }
}
