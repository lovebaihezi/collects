//! Clipboard handling utilities for paste operations.
//!
//! This module provides functionality to handle Ctrl+V (Cmd+V on macOS) paste events
//! and read image data from the clipboard on native platforms.

use egui::Context;

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
        read_and_log_clipboard_image();
    }
}

/// Reads image from clipboard and logs its information.
///
/// This function attempts to read an image from the system clipboard.
/// If successful, it logs the image dimensions and byte size.
/// If no image is found or an error occurs, appropriate messages are logged.
#[cfg(not(target_arch = "wasm32"))]
fn read_and_log_clipboard_image() {
    use arboard::Clipboard;

    match Clipboard::new() {
        Ok(mut clipboard) => match clipboard.get_image() {
            Ok(image_data) => {
                let width = image_data.width;
                let height = image_data.height;
                let bytes_len = image_data.bytes.len();

                // Calculate expected size for RGBA format (4 bytes per pixel)
                let expected_bytes = width * height * 4;
                let format_info = if bytes_len == expected_bytes {
                    "RGBA"
                } else {
                    "unknown"
                };

                log::info!(
                    "Clipboard image pasted: width={width}, height={height}, \
                     bytes={bytes_len}, format={format_info}"
                );
            }
            Err(arboard::Error::ContentNotAvailable) => {
                log::debug!(
                    "No image in clipboard - paste shortcut pressed but clipboard contains other content"
                );
            }
            Err(e) => {
                log::warn!("Failed to read clipboard image: {e}");
            }
        },
        Err(e) => {
            log::warn!("Failed to access clipboard: {e}");
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

    #[test]
    fn test_handle_paste_shortcut_no_panic() {
        // This test verifies that the function doesn't panic when called
        // with a fresh egui context. It won't actually trigger paste since
        // there are no input events, but ensures the code path is valid.
        let ctx = Context::default();
        handle_paste_shortcut(&ctx);
    }
}
