//! Clipboard image access for collects UI and CLI.
//!
//! This module provides a common interface for reading images from the system clipboard,
//! usable by both the native UI (egui/eframe) and CLI applications.
//!
//! # Architecture
//!
//! The module uses a trait-based design for testability:
//! - [`ClipboardProvider`]: Generic interface for clipboard access
//! - [`SystemClipboard`]: Production implementation using arboard crate (native only)
//!
//! # Platform Support
//!
//! - **Windows**: Uses Win32 Clipboard API (`CF_DIB/CF_DIBV5` formats)
//! - **macOS**: Uses `NSPasteboard` (Cocoa, supports TIFF/PNG)
//! - **Linux X11**: Uses X11 selections via xcb
//! - **Linux Wayland**: Uses wl-clipboard protocols
//! - **Web (WASM)**: Not yet supported
//!
//! # File URI Support
//!
//! On Linux file managers (like Dolphin, Nautilus), copying an image file
//! often places a `file://` URI in the clipboard rather than the actual image data.
//! This module detects such URIs and loads the image from the filesystem.
//!
//! # Example
//!
//! ```rust,no_run
//! use collects_input::clipboard::{ClipboardProvider, SystemClipboard};
//!
//! let clipboard = SystemClipboard;
//! match clipboard.get_image() {
//!     Ok(Some(image)) => {
//!         println!("Found image: {}x{}", image.width, image.height);
//!         println!("Format: {}, Filename: {}", image.mime_type, image.filename);
//!     }
//!     Ok(None) => println!("No image in clipboard"),
//!     Err(e) => eprintln!("Error: {e}"),
//! }
//! ```
//!
//! If you want to preserve the original encoded bytes as much as possible, prefer
//! [`ClipboardProvider::get_image_payload`]. UI preview can still downconvert later.

/// Image payload retrieved from the clipboard, preserved as encoded bytes when possible.
///
/// This is the preferred type for **storage/export** because it avoids forcing a
/// downconversion to RGBA at the clipboard boundary.
#[derive(Debug, Clone)]
pub struct ClipboardImagePayload {
    /// Encoded image bytes (ideally the original clipboard representation).
    ///
    /// Note: depending on platform/clipboard APIs, this may be synthesized (e.g. encoded
    /// from a provided bitmap). Callers should treat this as best-effort.
    pub bytes: Vec<u8>,
    /// MIME type of the encoded payload (e.g., "image/png", "image/jpeg").
    pub mime_type: String,
    /// Suggested filename for the image.
    pub filename: String,
    /// Whether this payload was synthesized (e.g. bitmap -> encoded PNG) because the
    /// platform did not provide the original encoded bytes.
    pub synthesized: bool,
}

/// Image data retrieved from the clipboard.
#[derive(Debug, Clone)]
pub struct ClipboardImage {
    /// Width of the image in pixels.
    pub width: usize,
    /// Height of the image in pixels.
    pub height: usize,
    /// Raw image bytes (PNG format for storage, or RGBA for display).
    pub bytes: Vec<u8>,
    /// MIME type of the image (e.g., "image/png").
    pub mime_type: String,
    /// Suggested filename for the image.
    pub filename: String,
}

/// Error types for clipboard operations.
#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    /// Clipboard does not contain image content.
    #[error("No image content in clipboard")]
    NoImageContent,
    /// Failed to access the clipboard.
    #[error("Clipboard access error: {0}")]
    AccessError(String),
    /// Image encoding/decoding failed.
    #[error("Image processing error: {0}")]
    ImageError(String),
}

/// Trait for clipboard image access, enabling mock implementations for testing.
///
/// This trait abstracts clipboard operations to allow:
/// - Production use via [`SystemClipboard`] (platform-specific implementations)
/// - Test mocking via custom implementations
pub trait ClipboardProvider {
    /// Attempts to get an image from the clipboard.
    ///
    /// # Returns
    /// - `Ok(Some(image))` if an image is available
    /// - `Ok(None)` if clipboard is accessible but contains no image
    /// - `Err(...)` if clipboard access failed
    fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError>;

    /// Returns raw RGBA bytes suitable for display (without PNG encoding).
    ///
    /// This is useful for UI contexts where you want to display the image
    /// without the overhead of PNG encoding/decoding.
    fn get_image_rgba(&self) -> Result<Option<ClipboardImage>, ClipboardError>;

    /// Returns an encoded image payload suitable for storage/export.
    ///
    /// Goal: preserve the original encoded bytes whenever the platform clipboard provides them.
    /// If the platform only provides a bitmap, implementations may synthesize a payload
    /// (e.g. encode bitmap as PNG) as a best-effort fallback.
    ///
    /// # Returns
    /// - `Ok(Some(payload))` if an image payload is available
    /// - `Ok(None)` if clipboard is accessible but contains no image
    /// - `Err(...)` if clipboard access failed
    fn get_image_payload(&self) -> Result<Option<ClipboardImagePayload>, ClipboardError> {
        // Back-compat default: use `get_image()` which returns PNG bytes for storage
        // in the current `SystemClipboard` implementation.
        //
        // Mark as synthesized because `get_image()` may encode from a bitmap.
        self.get_image().map(|opt| {
            opt.map(|img| ClipboardImagePayload {
                bytes: img.bytes,
                mime_type: img.mime_type,
                filename: img.filename,
                synthesized: true,
            })
        })
    }
}

/// System clipboard implementation using the `arboard` crate.
///
/// This struct provides cross-platform clipboard image access.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default, Clone, Copy)]
pub struct SystemClipboard;

#[cfg(not(target_arch = "wasm32"))]
impl ClipboardProvider for SystemClipboard {
    fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        use arboard::Clipboard;

        let mut clipboard =
            Clipboard::new().map_err(|e| ClipboardError::AccessError(e.to_string()))?;

        match clipboard.get_image() {
            Ok(image_data) => {
                // Convert RGBA data to PNG for storage
                let png_data =
                    encode_rgba_to_png(image_data.width, image_data.height, &image_data.bytes)?;

                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let filename = format!("clipboard_{timestamp}.png");

                Ok(Some(ClipboardImage {
                    width: image_data.width,
                    height: image_data.height,
                    bytes: png_data,
                    mime_type: "image/png".to_owned(),
                    filename,
                }))
            }
            Err(arboard::Error::ContentNotAvailable) => {
                // Try to load image from file:// URI in clipboard text
                if let Ok(text) = clipboard.get_text()
                    && let Some(image) = try_load_image_from_file_uri(&text, true)
                {
                    return Ok(Some(image));
                }
                Ok(None)
            }
            Err(e) => {
                // Try fallback to file URI before reporting error
                if let Ok(text) = clipboard.get_text()
                    && let Some(image) = try_load_image_from_file_uri(&text, true)
                {
                    return Ok(Some(image));
                }
                Err(ClipboardError::AccessError(e.to_string()))
            }
        }
    }

    fn get_image_rgba(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        use arboard::Clipboard;

        let mut clipboard =
            Clipboard::new().map_err(|e| ClipboardError::AccessError(e.to_string()))?;

        match clipboard.get_image() {
            Ok(image_data) => {
                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let filename = format!("clipboard_{timestamp}.png");

                Ok(Some(ClipboardImage {
                    width: image_data.width,
                    height: image_data.height,
                    bytes: image_data.bytes.into_owned(),
                    mime_type: "image/rgba".to_owned(),
                    filename,
                }))
            }
            Err(arboard::Error::ContentNotAvailable) => {
                // Try to load image from file:// URI in clipboard text
                if let Ok(text) = clipboard.get_text()
                    && let Some(image) = try_load_image_from_file_uri(&text, false)
                {
                    return Ok(Some(image));
                }
                Ok(None)
            }
            Err(e) => {
                // Try fallback to file URI before reporting error
                if let Ok(text) = clipboard.get_text()
                    && let Some(image) = try_load_image_from_file_uri(&text, false)
                {
                    return Ok(Some(image));
                }
                Err(ClipboardError::AccessError(e.to_string()))
            }
        }
    }
    fn get_image_payload(&self) -> Result<Option<ClipboardImagePayload>, ClipboardError> {
        use arboard::Clipboard;

        let mut clipboard =
            Clipboard::new().map_err(|e| ClipboardError::AccessError(e.to_string()))?;

        // 1) Prefer original file bytes if the clipboard contains `file://...` URIs.
        // This preserves the source file's encoding/bit depth/metadata.
        if let Ok(text) = clipboard.get_text()
            && let Some(payload) = try_load_payload_from_file_uri(&text)
        {
            return Ok(Some(payload));
        }

        // 2) Otherwise, fall back to synthesized payload:
        // read bitmap and encode as PNG (best-effort).
        match clipboard.get_image() {
            Ok(image_data) => {
                let png_data =
                    encode_rgba_to_png(image_data.width, image_data.height, &image_data.bytes)?;

                let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
                let filename = format!("clipboard_{timestamp}.png");

                Ok(Some(ClipboardImagePayload {
                    bytes: png_data,
                    mime_type: "image/png".to_owned(),
                    filename,
                    synthesized: true,
                }))
            }
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(e) => Err(ClipboardError::AccessError(e.to_string())),
        }
    }
}

/// Clears clipboard contents by overwriting with empty text (best-effort).
#[cfg(not(target_arch = "wasm32"))]
pub fn clear_clipboard_image() -> Result<(), ClipboardError> {
    use arboard::Clipboard;

    let mut clipboard = Clipboard::new().map_err(|e| ClipboardError::AccessError(e.to_string()))?;
    clipboard
        .set_text("")
        .map_err(|e| ClipboardError::AccessError(e.to_string()))
}

/// Encodes RGBA pixel data to PNG format.
#[cfg(not(target_arch = "wasm32"))]
fn encode_rgba_to_png(
    width: usize,
    height: usize,
    rgba_data: &[u8],
) -> Result<Vec<u8>, ClipboardError> {
    use image::{ImageBuffer, Rgba};

    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width as u32, height as u32, rgba_data.to_vec())
            .ok_or_else(|| ClipboardError::ImageError("Invalid image dimensions".to_owned()))?;

    let mut cursor = std::io::Cursor::new(Vec::new());
    img.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| ClipboardError::ImageError(format!("Failed to encode PNG: {e}")))?;

    Ok(cursor.into_inner())
}

/// Attempts to load an image from a file:// URI found in clipboard text.
///
/// On Linux file managers (like Dolphin, Nautilus), copying a file puts
/// a `file://` URI in the clipboard rather than the file contents.
#[cfg(not(target_arch = "wasm32"))]
fn try_load_image_from_file_uri(text: &str, encode_png: bool) -> Option<ClipboardImage> {
    // Clipboard may contain multiple lines (e.g., multiple files selected)
    // Try each line that looks like a file URI
    for line in text.lines() {
        let line = line.trim();
        if let Some(path) = extract_file_path_from_uri(line) {
            log::trace!(
                target: "collects_input::clipboard",
                "file_uri_detected path={path:?}",
            );

            if let Some(image) = load_image_from_path(&path, encode_png) {
                return Some(image);
            }
        }
    }
    None
}

/// Attempts to load an image payload (original encoded bytes) from a file:// URI found in clipboard text.
///
/// This preserves the original file bytes, so we don't lose bit depth/metadata by decoding+re-encoding.
#[cfg(not(target_arch = "wasm32"))]
fn try_load_payload_from_file_uri(text: &str) -> Option<ClipboardImagePayload> {
    for line in text.lines() {
        let line = line.trim();
        let path = extract_file_path_from_uri(line)?;

        log::trace!(
            target: "collects_input::clipboard",
            "file_uri_payload_detected path={path:?}",
        );

        if let Some(payload) = load_payload_from_path(&path) {
            return Some(payload);
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
            target: "collects_input::clipboard",
            "file_uri_not_found path={path:?}",
        );
        None
    }
}

/// Loads an image from a filesystem path and converts it to `ClipboardImage`.
///
/// Supports common image formats: PNG, JPEG, GIF, BMP, WebP, etc.
#[cfg(not(target_arch = "wasm32"))]
fn load_image_from_path(path: &std::path::Path, encode_png: bool) -> Option<ClipboardImage> {
    use image::GenericImageView as _;

    // Check if it's an image file by extension
    let extension = path.extension()?.to_str()?.to_lowercase();
    let is_image = matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "ico"
    );

    if !is_image {
        return None;
    }

    match image::open(path) {
        Ok(img) => {
            let (width, height) = img.dimensions();
            let rgba = img.to_rgba8();

            let filename = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "clipboard_image.png".to_owned());

            if encode_png {
                // Encode to PNG for storage
                let mut cursor = std::io::Cursor::new(Vec::new());
                if rgba.write_to(&mut cursor, image::ImageFormat::Png).is_err() {
                    return None;
                }

                Some(ClipboardImage {
                    width: width as usize,
                    height: height as usize,
                    bytes: cursor.into_inner(),
                    mime_type: "image/png".to_owned(),
                    filename,
                })
            } else {
                // Return raw RGBA bytes for display
                Some(ClipboardImage {
                    width: width as usize,
                    height: height as usize,
                    bytes: rgba.into_raw(),
                    mime_type: "image/rgba".to_owned(),
                    filename,
                })
            }
        }
        Err(e) => {
            log::trace!(
                target: "collects_input::clipboard",
                "failed_to_load_image path={path:?} error={e}",
            );
            None
        }
    }
}

/// Loads an encoded image payload from a filesystem path without decoding/re-encoding.
///
/// This preserves original file bytes and sets MIME type based on extension (best-effort).
#[cfg(not(target_arch = "wasm32"))]
fn load_payload_from_path(path: &std::path::Path) -> Option<ClipboardImagePayload> {
    use std::fs;

    // Best-effort image check by extension
    let extension = path.extension()?.to_str()?.to_lowercase();
    let is_image = matches!(
        extension.as_str(),
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "ico"
    );
    if !is_image {
        return None;
    }

    let bytes = fs::read(path).ok()?;

    let mime_type = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "bmp" => "image/bmp",
        "webp" => "image/webp",
        "tif" | "tiff" => "image/tiff",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
    .to_owned();

    let filename = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "clipboard_image".to_owned());

    Some(ClipboardImagePayload {
        bytes,
        mime_type,
        filename,
        synthesized: false,
    })
}

/// Clears clipboard contents on WASM (no-op, clipboard not yet supported).
#[cfg(target_arch = "wasm32")]
pub fn clear_clipboard_image() -> Result<(), ClipboardError> {
    Ok(())
}

/// Stub implementation for WASM (clipboard not yet supported).
#[cfg(target_arch = "wasm32")]
#[derive(Default, Clone, Copy)]
pub struct SystemClipboard;

#[cfg(target_arch = "wasm32")]
impl ClipboardProvider for SystemClipboard {
    fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        // Clipboard image API on web requires async and secure context
        // Not yet implemented
        Ok(None)
    }

    fn get_image_rgba(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
        Ok(None)
    }

    fn get_image_payload(&self) -> Result<Option<ClipboardImagePayload>, ClipboardError> {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockClipboardWithImage {
        image: ClipboardImage,
    }

    impl ClipboardProvider for MockClipboardWithImage {
        fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Ok(Some(self.image.clone()))
        }

        fn get_image_rgba(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Ok(Some(self.image.clone()))
        }

        fn get_image_payload(&self) -> Result<Option<ClipboardImagePayload>, ClipboardError> {
            Ok(Some(ClipboardImagePayload {
                bytes: self.image.bytes.clone(),
                mime_type: self.image.mime_type.clone(),
                filename: self.image.filename.clone(),
                synthesized: true,
            }))
        }
    }

    struct MockClipboardEmpty;

    impl ClipboardProvider for MockClipboardEmpty {
        fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Ok(None)
        }

        fn get_image_rgba(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Ok(None)
        }

        fn get_image_payload(&self) -> Result<Option<ClipboardImagePayload>, ClipboardError> {
            Ok(None)
        }
    }

    struct MockClipboardError;

    impl ClipboardProvider for MockClipboardError {
        fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Err(ClipboardError::AccessError("Mock error".to_owned()))
        }

        fn get_image_rgba(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
            Err(ClipboardError::AccessError("Mock error".to_owned()))
        }

        fn get_image_payload(&self) -> Result<Option<ClipboardImagePayload>, ClipboardError> {
            Err(ClipboardError::AccessError("Mock error".to_owned()))
        }
    }

    #[test]
    fn test_mock_clipboard_with_image() {
        let mock = MockClipboardWithImage {
            image: ClipboardImage {
                width: 100,
                height: 100,
                bytes: vec![0; 100 * 100 * 4],
                mime_type: "image/png".to_owned(),
                filename: "test.png".to_owned(),
            },
        };

        let result = mock.get_image();
        assert!(result.is_ok());
        let image = result.expect("should succeed").expect("should have image");
        assert_eq!(image.width, 100);
        assert_eq!(image.height, 100);
    }

    #[test]
    fn test_mock_clipboard_empty() {
        let mock = MockClipboardEmpty;
        let result = mock.get_image();
        assert!(result.is_ok());
        assert!(result.expect("should succeed").is_none());
    }

    #[test]
    fn test_mock_clipboard_error() {
        let mock = MockClipboardError;
        let result = mock.get_image();
        assert!(result.is_err());
    }

    #[test]
    fn test_clipboard_image_clone() {
        let image = ClipboardImage {
            width: 50,
            height: 50,
            bytes: vec![1, 2, 3, 4],
            mime_type: "image/png".to_owned(),
            filename: "clone_test.png".to_owned(),
        };
        let cloned = image.clone();
        assert_eq!(image.width, cloned.width);
        assert_eq!(image.height, cloned.height);
        assert_eq!(image.bytes, cloned.bytes);
    }
}
