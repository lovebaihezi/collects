//! File picker handler for selecting image files via native file dialogs.
//!
//! This module provides trait-based abstractions for file picker operations,
//! enabling mock implementations for testing without relying on system dialogs.
//!
//! # Platform Support
//!
//! - **Native (Windows, macOS, Linux)**: Full support via `rfd` crate using native dialogs.
//! - **Web (WASM)**: Not supported (stub implementation).
//!
//! # Usage
//!
//! The file picker is triggered by a keyboard shortcut (Ctrl+O / Cmd+O) or UI button.
//! When a user selects an image file, it is loaded and returned as `ImageData`.

use super::image_data::ImageData;

/// Trait for handling file picker operations, enabling mock implementations for testing.
///
/// This trait abstracts the file picker shortcut detection and dialog display,
/// allowing tests to inject mock file providers.
pub trait FilePickerHandler {
    /// Handle file picker shortcut and return image data if a file was selected.
    fn handle_file_pick(&self, ctx: &egui::Context) -> Option<ImageData>;
}

/// Default file picker handler using the system file dialog.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct SystemFilePickerHandler;

#[cfg(not(target_arch = "wasm32"))]
impl FilePickerHandler for SystemFilePickerHandler {
    fn handle_file_pick(&self, ctx: &egui::Context) -> Option<ImageData> {
        handle_file_pick_shortcut(ctx)
    }
}

/// Handles file picker shortcut (Ctrl+O / Cmd+O) and opens a file dialog.
///
/// # Arguments
///
/// * `ctx` - The egui context to check for keyboard shortcuts
///
/// # Returns
///
/// The selected `ImageData` if an image file was successfully loaded.
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_file_pick_shortcut(ctx: &egui::Context) -> Option<ImageData> {
    // Check for Ctrl+O (or Cmd+O on macOS)
    let open_shortcut_pressed =
        ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.command_only());

    if !open_shortcut_pressed {
        return None;
    }

    log::debug!("File picker shortcut detected (Ctrl+O / Cmd+O)");

    // Open file dialog
    pick_image_file()
}

/// Opens a native file dialog to pick an image file.
///
/// # Returns
///
/// The selected `ImageData` if an image file was successfully loaded.
#[cfg(not(target_arch = "wasm32"))]
pub fn pick_image_file() -> Option<ImageData> {
    use rfd::FileDialog;

    let file_path = FileDialog::new()
        .add_filter(
            "Image",
            &[
                "png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tiff", "tif",
            ],
        )
        .set_title("Select an image")
        .pick_file()?;

    log::info!("User selected file: {:?}", file_path);

    load_image_from_path(&file_path)
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
    use image::GenericImageView;
    use std::fs;

    log::debug!("Loading image from path: {:?}", path);

    // Read file contents
    let bytes = match fs::read(path) {
        Ok(b) => {
            log::debug!("Read {} bytes from file", b.len());
            b
        }
        Err(e) => {
            log::warn!("Failed to read file {:?}: {}", path, e);
            return None;
        }
    };

    // Try to decode the image
    let img = match image::load_from_memory(&bytes) {
        Ok(img) => img,
        Err(e) => {
            log::warn!("Failed to decode image from file {:?}: {}", path, e);
            return None;
        }
    };

    let (width, height) = img.dimensions();
    let rgba = img.to_rgba8();
    let rgba_bytes = rgba.into_raw();

    log::info!(
        "Loaded image from file picker: {}x{}, {} bytes",
        width,
        height,
        rgba_bytes.len()
    );

    Some(ImageData::new(width as usize, height as usize, rgba_bytes))
}

/// Stub file picker handler for WASM target.
#[cfg(target_arch = "wasm32")]
#[derive(Default)]
pub struct SystemFilePickerHandler;

#[cfg(target_arch = "wasm32")]
impl FilePickerHandler for SystemFilePickerHandler {
    fn handle_file_pick(&self, _ctx: &egui::Context) -> Option<ImageData> {
        // File picker not supported on WASM
        // Users can use drag-and-drop instead
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Mock file picker handler that always returns None (no file selected)
    struct MockFilePickerHandlerEmpty;

    impl FilePickerHandler for MockFilePickerHandlerEmpty {
        fn handle_file_pick(&self, _ctx: &egui::Context) -> Option<ImageData> {
            None
        }
    }

    /// Mock file picker handler that returns a predefined image
    struct MockFilePickerHandlerWithImage {
        image: ImageData,
    }

    impl FilePickerHandler for MockFilePickerHandlerWithImage {
        fn handle_file_pick(&self, _ctx: &egui::Context) -> Option<ImageData> {
            Some(self.image.clone())
        }
    }

    #[test]
    fn test_mock_file_picker_handler_empty() {
        let handler = MockFilePickerHandlerEmpty;
        let ctx = egui::Context::default();
        assert!(handler.handle_file_pick(&ctx).is_none());
    }

    #[test]
    fn test_mock_file_picker_handler_with_image() {
        let handler = MockFilePickerHandlerWithImage {
            image: ImageData::new(100, 100, vec![255u8; 100 * 100 * 4]),
        };
        let ctx = egui::Context::default();
        let result = handler.handle_file_pick(&ctx);
        assert!(result.is_some());
        let img = result.unwrap();
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 100);
    }

    #[test]
    fn test_file_picker_handler_trait_is_object_safe() {
        // Verify that FilePickerHandler can be used as a trait object
        fn _accept_file_picker_handler(_handler: &dyn FilePickerHandler) {}
        let handler = MockFilePickerHandlerEmpty;
        _accept_file_picker_handler(&handler);
    }

    #[cfg(not(target_arch = "wasm32"))]
    mod native_tests {
        use super::*;

        #[test]
        fn test_load_image_from_path_invalid() {
            // Test with a non-existent path
            let invalid_path = std::path::Path::new("/non/existent/path/image.png");
            let result = load_image_from_path(invalid_path);
            assert!(result.is_none());
        }

        #[test]
        fn test_load_image_from_path_valid_png() {
            // Create a temporary PNG file for testing
            use ::image::ImageEncoder;
            use ::image::codecs::png::PngEncoder;
            use std::io::Write;
            use tempfile::NamedTempFile;

            let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");

            // Create a minimal 1x1 red PNG image
            let mut png_data = Vec::new();
            let encoder = PngEncoder::new(&mut png_data);
            let pixel: [u8; 4] = [255, 0, 0, 255];
            encoder
                .write_image(&pixel, 1, 1, ::image::ColorType::Rgba8.into())
                .expect("Failed to encode test PNG");

            temp_file
                .write_all(&png_data)
                .expect("Failed to write to temp file");

            let result = load_image_from_path(temp_file.path());
            assert!(result.is_some(), "Should decode valid PNG file");
            let img = result.unwrap();
            assert_eq!(img.width, 1);
            assert_eq!(img.height, 1);
            assert_eq!(img.bytes.len(), 4); // RGBA has 4 bytes per pixel
        }
    }
}
