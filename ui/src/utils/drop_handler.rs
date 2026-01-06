//! Drop handler abstractions for drag-and-drop file support.
//!
//! This module provides trait-based abstractions for handling dropped files,
//! enabling mock implementations for testing without relying on system events.
//!
//! # Platform Support
//!
//! - **Native (Windows, macOS, Linux)**: Full support via winit/egui integration.
//!   On Windows, drag-and-drop must be explicitly enabled in viewport options.
//! - **Web (WASM)**: Partial support via browser drag-and-drop API.
//!
//! # Architecture
//!
//! The module uses a trait-based design for testability:
//! - `DropHandler` trait: Generic interface for handling dropped files
//! - `SystemDropHandler`: Production implementation using egui's input events

use super::image_data::ImageData;

/// Trait for handling dropped files, enabling mock implementations for testing.
///
/// This trait abstracts the drag-and-drop event handling,
/// allowing tests to inject mock file providers.
pub trait DropHandler {
    /// Handle dropped files and return image data if an image was dropped.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context to check for dropped files
    ///
    /// # Returns
    ///
    /// The dropped `ImageData` if an image file was found.
    fn handle_drop(&self, ctx: &egui::Context) -> Option<ImageData>;
}

/// Default drop handler using system drag-and-drop events.
#[derive(Default)]
pub struct SystemDropHandler;

impl DropHandler for SystemDropHandler {
    fn handle_drop(&self, ctx: &egui::Context) -> Option<ImageData> {
        handle_dropped_files(ctx)
    }
}

/// Handles dropped files from the system and returns image data if found.
///
/// This function checks for dropped files in the current frame and attempts
/// to load image data from them.
///
/// # Arguments
///
/// * `ctx` - The egui context to check for dropped files
///
/// # Returns
///
/// The dropped `ImageData` if an image file was successfully loaded.
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_dropped_files(ctx: &egui::Context) -> Option<ImageData> {
    let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());

    if dropped_files.is_empty() {
        return None;
    }

    log::trace!(
        target: "collects_ui::drop",
        "dropped_files={}",
        dropped_files.len()
    );

    // Process only the first dropped file (replace behavior like paste)
    for file in &dropped_files {
        let has_path = file.path.is_some();
        let has_bytes = file.bytes.is_some();

        log::trace!(
            target: "collects_ui::drop",
            "dropped_file name={} has_path={} has_bytes={}",
            file.name,
            has_path,
            has_bytes
        );

        if let Some(image) = load_image_from_dropped_file(file) {
            log::trace!(
                target: "collects_ui::drop",
                "loaded_image {}x{} bytes={}",
                image.width,
                image.height,
                image.bytes.len()
            );
            return Some(image);
        }

        log::warn!(
            target: "collects_ui::drop",
            "dropped_file_not_loadable name={} has_path={} has_bytes={}",
            file.name,
            has_path,
            has_bytes
        );
    }

    log::warn!(
        target: "collects_ui::drop",
        "no_valid_image_in_drop dropped_files={}",
        dropped_files.len()
    );
    None
}

/// Loads image data from a dropped file.
///
/// # Arguments
///
/// * `file` - The dropped file from egui
///
/// # Returns
///
/// The `ImageData` if the file is a valid image.
#[cfg(not(target_arch = "wasm32"))]
fn load_image_from_dropped_file(file: &egui::DroppedFile) -> Option<ImageData> {
    // Try to load from file path (native platforms)
    if let Some(path) = &file.path {
        return load_image_from_path(path);
    }

    // Try to load from bytes (web or if bytes are provided)
    if let Some(bytes) = &file.bytes {
        return load_image_from_bytes(bytes);
    }

    // This is the common failure mode when the backend reports a drop but doesn't
    // provide a filesystem path or file contents.
    log::warn!(
        target: "collects_ui::drop",
        "dropped_file_missing_path_and_bytes name={}",
        file.name
    );
    None
}

/// Loads an image from a file path.
///
/// # Arguments
///
/// * `path` - Path to the image file
///
/// # Returns
///
/// The `ImageData` if the file is a valid image.
#[cfg(not(target_arch = "wasm32"))]
fn load_image_from_path(path: &std::path::Path) -> Option<ImageData> {
    use std::fs;

    log::debug!("Loading image from path: {:?}", path);

    // Read file contents
    let bytes = match fs::read(path) {
        Ok(b) => {
            log::debug!("Read {} bytes from file", b.len());
            b
        }
        Err(e) => {
            log::warn!("Failed to read dropped file {:?}: {}", path, e);
            return None;
        }
    };

    load_image_from_bytes(&bytes)
}

/// Loads an image from raw bytes using the image crate.
///
/// # Arguments
///
/// * `bytes` - Raw bytes of the image file
///
/// # Returns
///
/// The `ImageData` if the bytes represent a valid image.
#[cfg(not(target_arch = "wasm32"))]
fn load_image_from_bytes(bytes: &[u8]) -> Option<ImageData> {
    use image::GenericImageView;

    // Try to decode the image
    let img = match image::load_from_memory(bytes) {
        Ok(img) => img,
        Err(e) => {
            log::debug!("Failed to decode image from dropped file: {}", e);
            return None;
        }
    };

    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();
    let rgba_bytes = rgba.into_raw();

    log::info!(
        "Loaded dropped image: {}x{}, {} bytes",
        width,
        height,
        rgba_bytes.len()
    );

    Some(ImageData::new(width as usize, height as usize, rgba_bytes))
}

/// Stub implementation for WASM target.
///
/// Web drag-and-drop support may work via the browser's drag-and-drop API,
/// but image decoding requires additional handling.
#[cfg(target_arch = "wasm32")]
pub fn handle_dropped_files(_ctx: &egui::Context) -> Option<ImageData> {
    // Web drag-and-drop image support requires:
    // 1. Browser drag-and-drop API integration
    // 2. Image decoding via web-sys or a WASM-compatible image library
    // This is left as a placeholder for future implementation.
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock drop handler that always returns None (no dropped files)
    struct MockDropHandlerEmpty;

    impl DropHandler for MockDropHandlerEmpty {
        fn handle_drop(&self, _ctx: &egui::Context) -> Option<ImageData> {
            None
        }
    }

    /// Mock drop handler that returns a predefined image
    struct MockDropHandlerWithImage {
        image: ImageData,
    }

    impl DropHandler for MockDropHandlerWithImage {
        fn handle_drop(&self, _ctx: &egui::Context) -> Option<ImageData> {
            Some(self.image.clone())
        }
    }

    #[test]
    fn test_mock_drop_handler_empty() {
        let handler = MockDropHandlerEmpty;
        let ctx = egui::Context::default();
        assert!(handler.handle_drop(&ctx).is_none());
    }

    #[test]
    fn test_mock_drop_handler_with_image() {
        let handler = MockDropHandlerWithImage {
            image: ImageData::new(100, 100, vec![255u8; 100 * 100 * 4]),
        };
        let ctx = egui::Context::default();
        let result = handler.handle_drop(&ctx);
        assert!(result.is_some());
        let img = result.unwrap();
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);
    }

    #[test]
    fn test_system_drop_handler_no_panic() {
        // This test verifies that the function doesn't panic when called
        // with a fresh egui context. It won't actually process drops since
        // there are no dropped files, but ensures the code path is valid.
        let handler = SystemDropHandler;
        let ctx = egui::Context::default();
        assert!(handler.handle_drop(&ctx).is_none());
    }

    #[test]
    fn test_drop_handler_trait_is_object_safe() {
        // Verify that DropHandler can be used as a trait object
        fn _accept_drop_handler(_handler: &dyn DropHandler) {}
        let handler = SystemDropHandler;
        _accept_drop_handler(&handler);
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native_tests {
        use super::*;

        #[test]
        fn test_load_image_from_bytes_invalid() {
            // Test with invalid image data
            let invalid_bytes = b"not an image";
            let result = load_image_from_bytes(invalid_bytes);
            assert!(result.is_none());
        }

        #[test]
        fn test_load_image_from_bytes_valid_png() {
            // Create a minimal 1x1 red PNG image programmatically
            use ::image::ImageEncoder;
            use ::image::codecs::png::PngEncoder;

            let mut png_data = Vec::new();
            let encoder = PngEncoder::new(&mut png_data);

            // Create a 1x1 red pixel (RGBA)
            let pixel: [u8; 4] = [255, 0, 0, 255];
            encoder
                .write_image(&pixel, 1, 1, ::image::ColorType::Rgba8.into())
                .expect("Failed to encode test PNG");

            let result = load_image_from_bytes(&png_data);
            assert!(result.is_some(), "Should decode valid PNG");
            let img = result.unwrap();
            assert_eq!(img.width, 1);
            assert_eq!(img.height, 1);
            // RGBA has 4 bytes per pixel
            assert_eq!(img.bytes.len(), 4);
        }
    }
}
