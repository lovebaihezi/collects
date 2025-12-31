//! Clipboard handling utilities for paste operations.
//!
//! This module provides functionality to handle Ctrl+V (Cmd+V on macOS) paste events
//! and read image data from the clipboard on native platforms.
//!
//! # Architecture
//!
//! The module uses a trait-based design for testability:
//! - `ClipboardProvider` trait: Generic interface for clipboard access
//! - `SystemClipboard`: Production implementation using arboard crate
//! - `MockClipboard`: Test implementation for unit testing
//!
//! # Example
//!
//! ```rust,no_run
//! use collects_ui::utils::clipboard::{ClipboardProvider, SystemClipboard};
//!
//! // Production usage
//! let clipboard = SystemClipboard;
//! if let Ok(Some(image)) = clipboard.get_image() {
//!     println!("Image: {}x{}", image.width, image.height);
//! }
//! ```

use egui::Context;

/// Image data from clipboard
#[derive(Debug, Clone)]
pub struct ClipboardImage {
    /// Width of the image in pixels
    pub width: usize,
    /// Height of the image in pixels
    pub height: usize,
    /// Raw image bytes (typically RGBA or RGB format)
    pub bytes: Vec<u8>,
}

/// Error types for clipboard operations
#[derive(Debug)]
pub enum ClipboardError {
    /// Clipboard does not contain image content
    NoImageContent,
    /// Failed to access the clipboard
    AccessError(String),
}

/// Trait for clipboard image access, enabling mock implementations for testing
pub trait ClipboardProvider {
    /// Attempts to get an image from the clipboard
    ///
    /// Returns:
    /// - `Ok(Some(image))` if an image is available
    /// - `Ok(None)` if clipboard is accessible but contains no image
    /// - `Err(...)` if clipboard access failed
    fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError>;
}

/// System clipboard implementation using arboard crate
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct SystemClipboard;

#[cfg(not(target_arch = "wasm32"))]
impl ClipboardProvider for SystemClipboard {
    fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        use arboard::Clipboard;

        let mut clipboard =
            Clipboard::new().map_err(|e| ClipboardError::AccessError(e.to_string()))?;

        match clipboard.get_image() {
            Ok(image_data) => Ok(Some(ClipboardImage {
                width: image_data.width,
                height: image_data.height,
                bytes: image_data.bytes.into_owned(),
            })),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(ClipboardError::AccessError(e.to_string())),
        }
    }
}

/// Handles paste keyboard shortcut (Ctrl+V or Cmd+V) and reads image from clipboard.
///
/// When a paste shortcut is detected and the clipboard contains an image,
/// this function logs the image information (width, height, byte size).
///
/// # Arguments
/// * `ctx` - The egui context to check for input events
///
/// # Platform Support
/// * Native (Windows, macOS, Linux): Full support via arboard crate
/// * Web (WASM): Not yet supported - clipboard image API requires async and secure context
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_paste_shortcut(ctx: &Context) {
    handle_paste_shortcut_with_clipboard(ctx, &SystemClipboard);
}

/// Handles paste shortcut with a custom clipboard provider (for testing)
///
/// # Arguments
/// * `ctx` - The egui context to check for input events
/// * `clipboard` - The clipboard provider to use for reading images
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_paste_shortcut_with_clipboard<C: ClipboardProvider>(ctx: &Context, clipboard: &C) {
    // Check for paste keyboard shortcut: Ctrl+V (Windows/Linux) or Cmd+V (macOS)
    // Using modifiers.command for cross-platform support
    let paste_pressed = ctx.input(|i| {
        i.events.iter().any(|event| {
            matches!(
                event,
                egui::Event::Key {
                    key: egui::Key::V,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command
            )
        })
    });

    if paste_pressed {
        read_and_log_clipboard_image(clipboard);
    }
}

/// Reads image from clipboard and logs its information.
///
/// This function attempts to read an image from the system clipboard.
/// If successful, it logs the image dimensions and byte size.
/// If no image is found or an error occurs, appropriate messages are logged.
#[cfg(not(target_arch = "wasm32"))]
fn read_and_log_clipboard_image<C: ClipboardProvider>(clipboard: &C) {
    match clipboard.get_image() {
        Ok(Some(image)) => {
            let width = image.width;
            let height = image.height;
            let bytes_len = image.bytes.len();

            // Detect format based on bytes per pixel using checked arithmetic
            // to avoid overflow for very large images
            let format_info = width
                .checked_mul(height)
                .and_then(|pixels| {
                    // Check for RGBA (4 bytes per pixel)
                    if pixels.checked_mul(4) == Some(bytes_len) {
                        Some("RGBA")
                    // Check for RGB (3 bytes per pixel)
                    } else if pixels.checked_mul(3) == Some(bytes_len) {
                        Some("RGB")
                    } else {
                        None
                    }
                })
                .unwrap_or("unknown");

            log::info!(
                "Clipboard image pasted: width={width}, height={height}, \
                 bytes={bytes_len}, format={format_info}"
            );
        }
        Ok(None) => {
            log::debug!(
                "No image in clipboard - paste shortcut pressed but clipboard contains other content"
            );
        }
        Err(ClipboardError::AccessError(e)) => {
            log::warn!("Failed to access clipboard: {e}");
        }
        Err(ClipboardError::NoImageContent) => {
            log::debug!("Clipboard does not contain image content");
        }
    }
}

/// Stub implementation for WASM target.
///
/// Clipboard image access is not yet supported on web platforms.
/// The browser Clipboard API requires async operations and a secure context (HTTPS).
#[cfg(target_arch = "wasm32")]
pub fn handle_paste_shortcut(_ctx: &Context) {
    // Web clipboard image support requires:
    // 1. HTTPS secure context
    // 2. Async Clipboard API
    // 3. User gesture (paste event)
    // This is left as a placeholder for future implementation.
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock clipboard for testing - returns a predefined image
    struct MockClipboardWithImage {
        image: ClipboardImage,
    }

    impl ClipboardProvider for MockClipboardWithImage {
        fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Ok(Some(self.image.clone()))
        }
    }

    /// Mock clipboard for testing - returns no image
    struct MockClipboardEmpty;

    impl ClipboardProvider for MockClipboardEmpty {
        fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Ok(None)
        }
    }

    /// Mock clipboard for testing - returns an error
    struct MockClipboardError;

    impl ClipboardProvider for MockClipboardError {
        fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Err(ClipboardError::AccessError("Mock error".to_string()))
        }
    }

    #[test]
    fn test_handle_paste_shortcut_no_panic() {
        // This test verifies that the function doesn't panic when called
        // with a fresh egui context. It won't actually trigger paste since
        // there are no input events, but ensures the code path is valid.
        let ctx = Context::default();
        handle_paste_shortcut(&ctx);
    }

    #[test]
    fn test_mock_clipboard_with_rgba_image() {
        let mock = MockClipboardWithImage {
            image: ClipboardImage {
                width: 100,
                height: 100,
                bytes: vec![0u8; 100 * 100 * 4], // RGBA format
            },
        };

        let result = mock.get_image();
        assert!(result.is_ok());
        let image = result.unwrap().unwrap();
        assert_eq!(image.width, 100);
        assert_eq!(image.height, 100);
        assert_eq!(image.bytes.len(), 100 * 100 * 4);
    }

    #[test]
    fn test_mock_clipboard_with_rgb_image() {
        let mock = MockClipboardWithImage {
            image: ClipboardImage {
                width: 50,
                height: 50,
                bytes: vec![0u8; 50 * 50 * 3], // RGB format
            },
        };

        let result = mock.get_image();
        assert!(result.is_ok());
        let image = result.unwrap().unwrap();
        assert_eq!(image.width, 50);
        assert_eq!(image.height, 50);
        assert_eq!(image.bytes.len(), 50 * 50 * 3);
    }

    #[test]
    fn test_mock_clipboard_empty() {
        let mock = MockClipboardEmpty;
        let result = mock.get_image();
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[test]
    fn test_mock_clipboard_error() {
        let mock = MockClipboardError;
        let result = mock.get_image();
        assert!(result.is_err());
    }

    #[test]
    fn test_handle_paste_with_mock_clipboard() {
        let ctx = Context::default();
        let mock = MockClipboardWithImage {
            image: ClipboardImage {
                width: 100,
                height: 100,
                bytes: vec![0u8; 100 * 100 * 4],
            },
        };

        // This won't trigger actual paste (no input events),
        // but verifies the generic function compiles and runs
        handle_paste_shortcut_with_clipboard(&ctx, &mock);
    }
}
