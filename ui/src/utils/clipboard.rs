//! Clipboard handling utilities for paste operations.
//!
//! This module provides functionality to handle Ctrl+V (Cmd+V on macOS) paste events
//! and read image data from the clipboard on native platforms.
//!
//! # File URI Support
//!
//! On Linux (especially with file managers like Dolphin), copying an image file
//! often places a `file://` URI in the clipboard rather than the actual image data.
//! This module detects such URIs and loads the image from the filesystem.
//!
//! # Platform-Specific Clipboard Behavior
//!
//! The clipboard implementation varies significantly across platforms. This module
//! uses the `arboard` crate which provides a unified API over platform-specific backends:
//!
//! ## Windows
//! - Uses the Win32 Clipboard API (`OpenClipboard`, `GetClipboardData`, etc.)
//! - Supports CF_DIB and CF_DIBV5 formats for images
//! - Images are typically in BGRA format, converted to RGBA by arboard
//! - Clipboard access requires the calling thread to have a message queue
//!
//! ## macOS
//! - Uses NSPasteboard (Cocoa framework)
//! - Supports TIFF, PNG, and other standard image formats
//! - The `modifiers.command` check maps to Cmd key (⌘) on macOS
//! - Images are decoded from pasteboard data types like `public.tiff` or `public.png`
//!
//! ## Linux (X11)
//! - Uses X11 selections (CLIPBOARD selection by default)
//! - Communicates with the X server to retrieve clipboard data
//! - Supports image/png, image/bmp and other MIME types
//! - May require an active X11 connection; headless environments need Xvfb
//!
//! ## Linux (Wayland)
//! - Uses wl-clipboard or native Wayland protocols via layer-shell
//! - Clipboard data is obtained through the Wayland compositor
//! - Similar MIME type support as X11
//!
//! ## Web (WASM) - Not Yet Supported
//! - Would use the async Clipboard API (`navigator.clipboard`)
//! - Requires HTTPS secure context
//! - Requires user gesture (paste event) due to browser security restrictions
//! - Reading images requires `clipboard-read` permission
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

/// Trait for clipboard image access, enabling mock implementations for testing.
///
/// This trait abstracts clipboard operations to allow:
/// - Production use via `SystemClipboard` (platform-specific implementations)
/// - Test mocking via custom implementations
///
/// See module-level documentation for platform-specific behavior details.
pub trait ClipboardProvider {
    /// Attempts to get an image from the clipboard
    ///
    /// Returns:
    /// - `Ok(Some(image))` if an image is available
    /// - `Ok(None)` if clipboard is accessible but contains no image
    /// - `Err(...)` if clipboard access failed
    fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError>;
}

/// System clipboard implementation using the `arboard` crate.
///
/// This struct provides cross-platform clipboard image access with the following
/// platform-specific backends:
///
/// - **Windows**: Win32 Clipboard API (CF_DIB/CF_DIBV5 formats)
/// - **macOS**: NSPasteboard (Cocoa, supports TIFF/PNG)
/// - **Linux X11**: X11 selections via xcb
/// - **Linux Wayland**: wl-clipboard protocols
///
/// The `arboard` crate handles format conversion (e.g., BGRA to RGBA on Windows)
/// and provides a consistent `ImageData` struct across all platforms.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct SystemClipboard;

#[cfg(not(target_arch = "wasm32"))]
impl ClipboardProvider for SystemClipboard {
    /// Retrieves an image from the system clipboard.
    ///
    /// # Platform Behavior
    ///
    /// - **Windows**: Reads CF_DIB or CF_DIBV5 data, converts BGRA to RGBA
    /// - **macOS**: Reads from NSPasteboard, decodes TIFF/PNG data
    /// - **Linux**: Reads image/png or image/bmp MIME types from X11/Wayland
    ///
    /// # Errors
    ///
    /// Returns `ClipboardError::AccessError` if:
    /// - Clipboard is locked by another application
    /// - No display server connection (Linux headless)
    /// - Image data is corrupted or in unsupported format
    fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        use arboard::Clipboard;

        let mut clipboard =
            Clipboard::new().map_err(|e| ClipboardError::AccessError(e.to_string()))?;

        // Diagnostic: check what content types are available in clipboard
        match clipboard.get_text() {
            Ok(text) => {
                let preview = if text.len() > 100 {
                    format!("{}...", &text[..100])
                } else {
                    text.clone()
                };
                log::trace!(
                    target: "collects_ui::paste",
                    "clipboard_has_text len={} preview={:?}",
                    text.len(),
                    preview
                );
            }
            Err(e) => {
                log::trace!(
                    target: "collects_ui::paste",
                    "clipboard_no_text: {e}"
                );
            }
        }

        match clipboard.get_image() {
            Ok(image_data) => {
                log::trace!(
                    target: "collects_ui::paste",
                    "clipboard_get_image_ok {}x{} bytes={}",
                    image_data.width,
                    image_data.height,
                    image_data.bytes.len()
                );
                Ok(Some(ClipboardImage {
                    width: image_data.width,
                    height: image_data.height,
                    bytes: image_data.bytes.into_owned(),
                }))
            }
            Err(arboard::Error::ContentNotAvailable) => {
                log::trace!(
                    target: "collects_ui::paste",
                    "clipboard_get_image_err: ContentNotAvailable"
                );
                // Try to load image from file:// URI in clipboard text
                if let Ok(text) = clipboard.get_text()
                    && let Some(image) = try_load_image_from_file_uri(&text)
                {
                    return Ok(Some(image));
                }
                Ok(None)
            }
            Err(e) => {
                log::warn!(
                    target: "collects_ui::paste",
                    "clipboard_get_image_err: {e}"
                );
                // Try to load image from file:// URI in clipboard text as fallback
                if let Ok(text) = clipboard.get_text()
                    && let Some(image) = try_load_image_from_file_uri(&text)
                {
                    return Ok(Some(image));
                }
                Err(ClipboardError::AccessError(e.to_string()))
            }
        }
    }
}

/// Attempts to load an image from a file:// URI found in clipboard text.
///
/// On Linux file managers (like Dolphin, Nautilus), copying a file puts
/// a `file://` URI in the clipboard rather than the file contents.
/// This function detects such URIs and loads the image from disk.
///
/// # Arguments
/// * `text` - The clipboard text that may contain a file:// URI
///
/// # Returns
/// * `Some(ClipboardImage)` if a valid image file was found and loaded
/// * `None` if the text is not a file URI or the file couldn't be loaded as an image
#[cfg(not(target_arch = "wasm32"))]
fn try_load_image_from_file_uri(text: &str) -> Option<ClipboardImage> {
    // Clipboard may contain multiple lines (e.g., multiple files selected)
    // Try each line that looks like a file URI
    for line in text.lines() {
        let line = line.trim();
        if let Some(path) = extract_file_path_from_uri(line) {
            log::trace!(
                target: "collects_ui::paste",
                "clipboard_file_uri_detected path={:?}",
                path
            );

            if let Some(image) = load_image_from_path(&path) {
                return Some(image);
            }
        }
    }
    None
}

/// Extracts a filesystem path from a file:// URI.
///
/// Handles URL decoding for paths with special characters (spaces, unicode, etc.)
#[cfg(not(target_arch = "wasm32"))]
fn extract_file_path_from_uri(uri: &str) -> Option<std::path::PathBuf> {
    let uri = uri.trim();

    // Check for file:// prefix (case-insensitive)
    if !uri.to_lowercase().starts_with("file://") {
        return None;
    }

    // Extract the path part after file://
    let path_str = &uri[7..]; // Skip "file://"

    // URL-decode the path (handles %20 for spaces, %C3%A9 for é, etc.)
    let decoded = urlencoding::decode(path_str).ok()?;

    let path = std::path::PathBuf::from(decoded.as_ref());

    // Verify the path exists and is a file
    if path.is_file() {
        Some(path)
    } else {
        log::trace!(
            target: "collects_ui::paste",
            "clipboard_file_uri_not_found path={:?}",
            path
        );
        None
    }
}

/// Loads an image from a filesystem path and converts it to ClipboardImage.
///
/// Supports common image formats: PNG, JPEG, GIF, BMP, WebP, etc.
#[cfg(not(target_arch = "wasm32"))]
fn load_image_from_path(path: &std::path::Path) -> Option<ClipboardImage> {
    use image::GenericImageView;

    match image::open(path) {
        Ok(img) => {
            let rgba = img.to_rgba8();
            let (width, height) = img.dimensions();

            log::trace!(
                target: "collects_ui::paste",
                "clipboard_file_image_loaded path={:?} {}x{}",
                path,
                width,
                height
            );

            Some(ClipboardImage {
                width: width as usize,
                height: height as usize,
                bytes: rgba.into_raw(),
            })
        }
        Err(e) => {
            log::warn!(
                target: "collects_ui::paste",
                "clipboard_file_image_load_error path={:?} error={}",
                path,
                e
            );
            None
        }
    }
}

/// Handles paste keyboard shortcut (Ctrl+V or Cmd+V) and returns pasted image.
///
/// When a paste shortcut is detected and the clipboard contains an image,
/// this function returns the image data for storage in state.
///
/// # Arguments
/// * `ctx` - The egui context to check for input events
///
/// # Returns
///
/// The pasted `ClipboardImage` if one was found, or None.
///
/// # Platform Support
/// * Native (Windows, macOS, Linux): Full support via arboard crate
/// * Web (WASM): Not yet supported - clipboard image API requires async and secure context
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_paste_shortcut(ctx: &Context) -> Option<ClipboardImage> {
    handle_paste_shortcut_with_clipboard(ctx, &SystemClipboard)
}

/// Handles paste shortcut with a custom clipboard provider (for testing)
///
/// # Arguments
/// * `ctx` - The egui context to check for input events
/// * `clipboard` - The clipboard provider to use for reading images
///
/// # Returns
///
/// The pasted `ClipboardImage` if one was found, or None.
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_paste_shortcut_with_clipboard<C: ClipboardProvider>(
    ctx: &Context,
    clipboard: &C,
) -> Option<ClipboardImage> {
    // Use custom consume_key that works around egui issue #4065:
    // On some platforms (notably Wayland), Ctrl+V doesn't fire a `pressed: true` event,
    // but the key release event does come through. So for Ctrl+V specifically,
    // we react to the key release instead of press.
    let paste_pressed = ctx.input_mut(|i| {
        consume_key(i, egui::Modifiers::CTRL, egui::Key::V)
            || consume_key(i, egui::Modifiers::COMMAND, egui::Key::V)
    });

    if paste_pressed {
        log::trace!(target: "collects_ui::paste", "paste_shortcut_detected");
        read_clipboard_image(clipboard)
    } else {
        None
    }
}

/// Custom key consumption that works around https://github.com/emilk/egui/issues/4065
///
/// On some platforms (notably Wayland), `Ctrl+V` doesn't fire a `pressed: true` event,
/// but the `pressed: false` (key release) event does come through.
/// For `Ctrl+V` specifically, this function reacts to the key release instead of press.
#[cfg(not(target_arch = "wasm32"))]
fn consume_key(
    input_state: &mut egui::InputState,
    modifiers: egui::Modifiers,
    key: egui::Key,
) -> bool {
    let mut found = false;

    input_state.events.retain(|event| {
        let is_match = matches!(
            event,
            egui::Event::Key {
                key: ev_key,
                modifiers: ev_mods,
                pressed,
                ..
            } if
                *ev_key == key
                && ev_mods.matches_exact(modifiers)
                // For Ctrl+V, react to key release (pressed: false) to work around #4065
                // For other shortcuts, react to key press (pressed: true)
                && *pressed != (matches!(key, egui::Key::V) && modifiers == egui::Modifiers::CTRL)
        );

        found |= is_match;

        !is_match
    });

    found
}

/// Reads image from clipboard and returns it.
///
/// This function attempts to read an image from the system clipboard.
/// If successful, it logs the image dimensions and returns the image.
/// If no image is found or an error occurs, appropriate messages are logged.
///
/// # Returns
///
/// The `ClipboardImage` if one was found, or None.
#[cfg(not(target_arch = "wasm32"))]
fn read_clipboard_image<C: ClipboardProvider>(clipboard: &C) -> Option<ClipboardImage> {
    match clipboard.get_image() {
        Ok(Some(image)) => {
            let width = image.width;
            let height = image.height;
            let bytes_len = image.bytes.len();

            log::trace!(
                target: "collects_ui::paste",
                "clipboard_image_loaded {}x{} bytes={}",
                width,
                height,
                bytes_len
            );

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

            log::trace!(
                target: "collects_ui::paste",
                "clipboard_image_pasted width={width} height={height} bytes={bytes_len} format={format_info}"
            );

            Some(image)
        }
        Ok(None) => {
            log::trace!(
                target: "collects_ui::paste",
                "clipboard_no_image: clipboard accessible but contains no image data"
            );
            None
        }
        Err(ClipboardError::AccessError(e)) => {
            log::warn!(target: "collects_ui::paste", "clipboard_access_error {e}");
            None
        }
        Err(ClipboardError::NoImageContent) => {
            log::trace!(
                target: "collects_ui::paste",
                "clipboard_no_image_content: clipboard does not contain image content"
            );
            None
        }
    }
}

/// Stub implementation for WASM target.
///
/// Clipboard image access is not yet supported on web platforms.
/// The browser Clipboard API requires async operations and a secure context (HTTPS).
///
/// # Returns
///
/// Always returns None on WASM.
#[cfg(target_arch = "wasm32")]
pub fn handle_paste_shortcut(_ctx: &Context) -> Option<ClipboardImage> {
    // Web clipboard image support requires:
    // 1. HTTPS secure context
    // 2. Async Clipboard API
    // 3. User gesture (paste event)
    // This is left as a placeholder for future implementation.
    None
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

    #[test]
    fn test_extract_file_path_from_uri_valid() {
        let path = super::extract_file_path_from_uri("file:///home/user/image.png");
        // Path extraction works but file won't exist in test environment
        assert!(path.is_none()); // File doesn't exist
    }

    #[test]
    fn test_extract_file_path_from_uri_with_spaces() {
        // URL-encoded space (%20) should be decoded
        let uri = "file:///home/user/my%20image.png";
        // We can't fully test without a real file, but we can verify the function doesn't panic
        let _ = super::extract_file_path_from_uri(uri);
    }

    #[test]
    fn test_extract_file_path_from_uri_not_file() {
        assert!(super::extract_file_path_from_uri("https://example.com/image.png").is_none());
        assert!(super::extract_file_path_from_uri("/home/user/image.png").is_none());
        assert!(super::extract_file_path_from_uri("").is_none());
    }

    #[test]
    fn test_try_load_image_from_file_uri_multiline() {
        // Test that multiline clipboard content is handled (multiple files selected)
        let text = "file:///nonexistent1.png\r\nfile:///nonexistent2.png\r\n";
        // Should not panic, just return None since files don't exist
        assert!(super::try_load_image_from_file_uri(text).is_none());
    }
}
