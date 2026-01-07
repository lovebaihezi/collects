//! Paste handler abstractions for clipboard image support.
//!
//! This module provides trait-based abstractions for handling paste operations,
//! enabling mock implementations for testing without relying on system clipboard.

use super::clipboard::{self, ClipboardImage};

/// Trait for handling paste operations, enabling mock implementations for testing.
///
/// This trait abstracts the paste shortcut detection and clipboard access,
/// allowing tests to inject mock clipboard providers.
pub trait PasteHandler {
    /// Handle paste shortcut and return clipboard image if available.
    fn handle_paste(&self, ctx: &egui::Context) -> Option<ClipboardImage>;
}

/// Default paste handler using the system clipboard.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct SystemPasteHandler;

#[cfg(not(target_arch = "wasm32"))]
impl PasteHandler for SystemPasteHandler {
    fn handle_paste(&self, ctx: &egui::Context) -> Option<ClipboardImage> {
        clipboard::handle_paste_shortcut(ctx)
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
    fn handle_paste(&self, ctx: &egui::Context) -> Option<ClipboardImage> {
        clipboard::handle_paste_shortcut_with_clipboard(ctx, &self.clipboard)
    }
}

/// System paste handler for WASM target using web_sys.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct SystemPasteHandler;

#[cfg(target_arch = "wasm32")]
impl PasteHandler for SystemPasteHandler {
    fn handle_paste(&self, ctx: &egui::Context) -> Option<ClipboardImage> {
        clipboard::handle_paste_shortcut(ctx)
    }
}

/// Generic paste handler that wraps any ClipboardProvider for testing (WASM).
#[cfg(target_arch = "wasm32")]
pub struct GenericPasteHandler<C: clipboard::ClipboardProvider> {
    clipboard: C,
}

#[cfg(target_arch = "wasm32")]
impl<C: clipboard::ClipboardProvider> GenericPasteHandler<C> {
    /// Create a new paste handler with the given clipboard provider.
    pub fn new(clipboard: C) -> Self {
        Self { clipboard }
    }
}

#[cfg(target_arch = "wasm32")]
impl<C: clipboard::ClipboardProvider> PasteHandler for GenericPasteHandler<C> {
    fn handle_paste(&self, ctx: &egui::Context) -> Option<ClipboardImage> {
        clipboard::handle_paste_shortcut_with_clipboard(ctx, &self.clipboard)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::clipboard::{ClipboardError, ClipboardProvider};

    /// Mock paste handler that always returns None (no image in clipboard)
    struct MockPasteHandlerEmpty;

    impl PasteHandler for MockPasteHandlerEmpty {
        fn handle_paste(&self, _ctx: &egui::Context) -> Option<ClipboardImage> {
            None
        }
    }

    /// Mock paste handler that returns a predefined image
    struct MockPasteHandlerWithImage {
        image: ClipboardImage,
    }

    impl PasteHandler for MockPasteHandlerWithImage {
        fn handle_paste(&self, _ctx: &egui::Context) -> Option<ClipboardImage> {
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
            image: ClipboardImage {
                width: 100,
                height: 100,
                bytes: vec![255u8; 100 * 100 * 4],
            },
        };
        let ctx = egui::Context::default();
        let result = handler.handle_paste(&ctx);
        assert!(result.is_some());
        let img = result.unwrap();
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);
    }

    #[test]
    fn test_generic_paste_handler_with_mock_clipboard() {
        struct MockClipboard {
            image: Option<ClipboardImage>,
        }

        impl ClipboardProvider for MockClipboard {
            fn get_image(&self) -> Result<Option<ClipboardImage>, ClipboardError> {
                Ok(self.image.clone())
            }
        }

        let mock_clipboard = MockClipboard {
            image: Some(ClipboardImage {
                width: 50,
                height: 50,
                bytes: vec![128u8; 50 * 50 * 4],
            }),
        };

        let paste_handler = GenericPasteHandler::new(mock_clipboard);
        let ctx = egui::Context::default();

        // The handler won't return an image because no paste key event occurred,
        // but this verifies the generic type composition works correctly
        let result = paste_handler.handle_paste(&ctx);
        assert!(result.is_none()); // No key event, so no paste triggered
    }
}
