//! Paste handler abstractions for clipboard image support.
//!
//! This module provides trait-based abstractions for handling paste operations,
//! enabling mock implementations for testing without relying on system clipboard.

use super::clipboard::ClipboardImagePayload;
#[cfg(not(target_arch = "wasm32"))]
use super::clipboard::{self};

/// Trait for handling paste operations, enabling mock implementations for testing.
///
/// This trait abstracts the paste shortcut detection and clipboard access,
/// allowing tests to inject mock clipboard providers.
pub trait PasteHandler {
    /// Handle paste shortcut and return clipboard image payload if available.
    fn handle_paste(&self, ctx: &egui::Context) -> Option<ClipboardImagePayload>;
}

/// Default paste handler using the system clipboard.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct SystemPasteHandler;

#[cfg(not(target_arch = "wasm32"))]
impl PasteHandler for SystemPasteHandler {
    fn handle_paste(&self, ctx: &egui::Context) -> Option<ClipboardImagePayload> {
        clipboard::handle_paste_shortcut_payload(ctx)
    }
}

/// Generic paste handler that wraps any ClipboardProvider for testing.
#[cfg(not(target_arch = "wasm32"))]
pub struct GenericPasteHandler<C: clipboard::ClipboardProvider> {
    clipboard: C,
}

#[cfg(not(target_arch = "wasm32"))]
impl<C: clipboard::ClipboardProvider> GenericPasteHandler<C> {
    /// Create a new paste handler with the given clipboard provider.
    pub fn new(clipboard: C) -> Self {
        Self { clipboard }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<C: clipboard::ClipboardProvider> PasteHandler for GenericPasteHandler<C> {
    fn handle_paste(&self, ctx: &egui::Context) -> Option<ClipboardImagePayload> {
        clipboard::handle_paste_shortcut_payload_with_clipboard(ctx, &self.clipboard)
    }
}

/// Stub paste handler for WASM target.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct SystemPasteHandler;

#[cfg(target_arch = "wasm32")]
impl PasteHandler for SystemPasteHandler {
    fn handle_paste(&self, _ctx: &egui::Context) -> Option<ClipboardImagePayload> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::clipboard::{ClipboardError, ClipboardImage, ClipboardProvider};

    /// Mock paste handler that always returns None (no image in clipboard)
    struct MockPasteHandlerEmpty;

    impl PasteHandler for MockPasteHandlerEmpty {
        fn handle_paste(&self, _ctx: &egui::Context) -> Option<ClipboardImagePayload> {
            None
        }
    }

    /// Mock paste handler that returns a predefined payload
    struct MockPasteHandlerWithImage {
        image: ClipboardImagePayload,
    }

    impl PasteHandler for MockPasteHandlerWithImage {
        fn handle_paste(&self, _ctx: &egui::Context) -> Option<ClipboardImagePayload> {
            Some(self.image.clone())
        }
    }

    #[test]
    fn test_mock_paste_handler_empty() {
        let handler = MockPasteHandlerEmpty;
        let ctx = egui::Context::default();
        assert!(handler.handle_paste(&ctx).is_none());
    }

    #[test]
    fn test_mock_paste_handler_with_image() {
        let handler = MockPasteHandlerWithImage {
            image: ClipboardImagePayload {
                bytes: vec![1, 2, 3, 4],
                mime_type: "image/png".to_owned(),
                filename: "test.png".to_owned(),
                synthesized: true,
            },
        };
        let ctx = egui::Context::default();
        let result = handler.handle_paste(&ctx);
        assert!(result.is_some());
        let payload = result.unwrap();
        assert_eq!(payload.mime_type, "image/png");
        assert_eq!(payload.filename, "test.png");
        assert_eq!(payload.bytes.len(), 4);
    }

    #[test]
    fn test_generic_paste_handler_with_mock_clipboard() {
        struct MockClipboard {
            payload: Option<ClipboardImagePayload>,
        }

        impl ClipboardProvider for MockClipboard {
            fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
                Ok(None)
            }

            fn get_image_rgba(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
                Ok(None)
            }

            fn get_image_payload(&self) -> Result<Option<ClipboardImagePayload>, ClipboardError> {
                Ok(self.payload.clone())
            }
        }

        let mock_clipboard = MockClipboard {
            payload: Some(ClipboardImagePayload {
                bytes: vec![137, 80, 78, 71], // "PNG" signature prefix (partial)
                mime_type: "image/png".to_owned(),
                filename: "test.png".to_owned(),
                synthesized: true,
            }),
        };

        let paste_handler = GenericPasteHandler::new(mock_clipboard);
        let ctx = egui::Context::default();

        // The handler won't return a payload because no paste key event occurred,
        // but this verifies the generic type composition works correctly
        let result = paste_handler.handle_paste(&ctx);
        assert!(result.is_none()); // No key event, so no paste triggered
    }
}
