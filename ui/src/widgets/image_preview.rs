//! Image preview widget for displaying pasted images.
//!
//! This module provides:
//! - A simple state for storing a single image texture
//! - A widget for displaying the current image preview
//! - A maximized view modal for full-size image display
//!
//! # Architecture
//!
//! Only one image is stored at a time. Each paste replaces the current image.
//! The image is stored as an `egui::TextureHandle` for efficient rendering.
//!
//! # Usage
//!
//! 1. When an image is pasted/dropped, call `ImagePreviewState::set_image()`
//! 2. Call `image_preview()` to render the current image
//! 3. Click the image to maximize it in a modal

use collects_states::State;
use egui::{Color32, ColorImage, Context, Response, TextureHandle, TextureOptions, Ui, Window};
use std::any::Any;

/// The current image entry in the preview state.
pub struct ImageEntry {
    /// The texture handle for rendering.
    pub texture: TextureHandle,
    /// Original width of the image.
    pub width: usize,
    /// Original height of the image.
    pub height: usize,
}

impl std::fmt::Debug for ImageEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImageEntry")
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// State for storing and displaying a single image preview.
///
/// Each paste operation replaces the current image.
/// The image is not persisted - it only exists for display during the session.
#[derive(Default)]
pub struct ImagePreviewState {
    /// The current image (if any).
    current_image: Option<ImageEntry>,
    /// Whether the image is currently maximized.
    is_maximized: bool,
}

impl std::fmt::Debug for ImagePreviewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImagePreviewState")
            .field("has_image", &self.current_image.is_some())
            .field("is_maximized", &self.is_maximized)
            .finish()
    }
}

impl State for ImagePreviewState {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl ImagePreviewState {
    /// Create a new empty image preview state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the current image, replacing any existing image.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context for creating textures
    /// * `image` - The color image data to display
    pub fn set_image(&mut self, ctx: &Context, image: ColorImage) {
        let width = image.width();
        let height = image.height();

        // Create texture with linear filtering for better quality when scaled
        let texture = ctx.load_texture("image_preview", image, TextureOptions::LINEAR);

        self.current_image = Some(ImageEntry {
            texture,
            width,
            height,
        });
        self.is_maximized = false;
    }

    /// Set image from raw RGBA bytes, replacing any existing image.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context for creating textures
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    /// * `rgba_bytes` - Raw RGBA pixel data (4 bytes per pixel)
    ///
    /// # Returns
    ///
    /// `true` if the image was set successfully, `false` if the data was invalid.
    pub fn set_image_rgba(
        &mut self,
        ctx: &Context,
        width: usize,
        height: usize,
        rgba_bytes: Vec<u8>,
    ) -> bool {
        // Validate that we have exactly the right number of bytes
        let expected_bytes = width * height * 4;
        if rgba_bytes.len() != expected_bytes {
            log::warn!(
                "Invalid image data: expected {} bytes ({}x{}x4), got {} bytes",
                expected_bytes,
                width,
                height,
                rgba_bytes.len()
            );
            return false;
        }

        // Convert bytes to Color32 pixels
        let pixels: Vec<Color32> = rgba_bytes
            .chunks_exact(4)
            .map(|chunk| Color32::from_rgba_unmultiplied(chunk[0], chunk[1], chunk[2], chunk[3]))
            .collect();

        let image = ColorImage::new([width, height], pixels);
        self.set_image(ctx, image);
        true
    }

    /// Check if there is an image to display.
    pub fn has_image(&self) -> bool {
        self.current_image.is_some()
    }

    /// Get the current image (if any).
    pub fn current_image(&self) -> Option<&ImageEntry> {
        self.current_image.as_ref()
    }

    /// Set the maximized state.
    pub fn set_maximized(&mut self, maximized: bool) {
        self.is_maximized = maximized;
    }

    /// Check if the image is maximized.
    pub fn is_maximized(&self) -> bool {
        self.is_maximized
    }

    /// Clear the current image.
    pub fn clear(&mut self) {
        self.current_image = None;
        self.is_maximized = false;
    }
}

/// Maximum display size for the preview image (pixels).
const MAX_PREVIEW_SIZE: f32 = 400.0;

/// Renders the image in fullscreen mode.
///
/// Displays the current pasted image filling the available space while preserving aspect ratio.
/// Shows a close button to clear the image and return to normal view.
///
/// # Arguments
///
/// * `state` - Mutable reference to the image preview state
/// * `ui` - The egui UI to render into
///
/// # Returns
///
/// The egui Response from the widget.
pub fn image_preview_fullscreen(state: &mut ImagePreviewState, ui: &mut Ui) -> Response {
    let Some(entry) = state.current_image() else {
        // Should not happen, but handle gracefully
        ui.label("No image");
        return ui.response();
    };

    let width = entry.width;
    let height = entry.height;
    let texture = entry.texture.clone();

    // Calculate display size to fill available space while preserving aspect ratio
    let available_size = ui.available_size();
    let aspect_ratio = width as f32 / height as f32;

    let (display_w, display_h) = {
        let max_w = available_size.x;
        let max_h = available_size.y - 40.0; // Leave space for close button

        if max_w / max_h > aspect_ratio {
            // Height-constrained
            (max_h * aspect_ratio, max_h)
        } else {
            // Width-constrained
            (max_w, max_w / aspect_ratio)
        }
    };

    ui.vertical_centered(|ui| {
        // Close button at the top
        if ui.button("✕ Close Image").clicked() {
            state.clear();
        }

        ui.add_space(8.0);

        // Display image dimensions with display indicator for testing
        ui.label(format!("Image displayed: {}×{}", width, height));

        ui.add_space(8.0);

        // Display the image
        ui.image(egui::load::SizedTexture::new(
            texture.id(),
            [display_w, display_h],
        ));
    });

    ui.response()
}

/// Renders the image preview widget.
///
/// Displays the current pasted image. Each paste replaces the previous image.
/// Clicking the image maximizes it in a modal overlay.
///
/// # Arguments
///
/// * `state` - Mutable reference to the image preview state
/// * `ui` - The egui UI to render into
///
/// # Returns
///
/// The egui Response from the widget.
pub fn image_preview(state: &mut ImagePreviewState, ui: &mut Ui) -> Response {
    let response = ui
        .scope(|ui| {
            let Some(entry) = state.current_image() else {
                ui.label("No image. Paste (Ctrl+V) an image to display it here.");
                return false;
            };

            let width = entry.width;
            let height = entry.height;
            let texture = entry.texture.clone();

            // Calculate display size preserving aspect ratio
            let aspect_ratio = width as f32 / height as f32;
            let (display_w, display_h) = if aspect_ratio > 1.0 {
                let w = MAX_PREVIEW_SIZE.min(width as f32);
                (w, w / aspect_ratio)
            } else {
                let h = MAX_PREVIEW_SIZE.min(height as f32);
                (h * aspect_ratio, h)
            };

            // Create clickable image button
            let sized_texture = egui::load::SizedTexture::new(texture.id(), [display_w, display_h]);
            let response = ui.add(
                egui::Button::image(egui::Image::from_texture(sized_texture))
                    .frame(true)
                    .sense(egui::Sense::click()),
            );

            let clicked = response.clicked();
            response.on_hover_text(format!("{}×{} - Click to maximize", width, height));

            clicked
        })
        .inner;

    // Handle click to maximize
    if response {
        state.set_maximized(true);
    }

    // Show maximized modal if needed
    if state.is_maximized() {
        show_maximized_image_modal(state, ui);
    }

    ui.response()
}

/// Shows the maximized image modal.
fn show_maximized_image_modal(state: &mut ImagePreviewState, ui: &mut Ui) {
    let Some(entry) = state.current_image() else {
        state.set_maximized(false);
        return;
    };

    let width = entry.width;
    let height = entry.height;
    let texture = entry.texture.clone();

    let mut open = true;

    Window::new(format!("Image Preview - {}×{}", width, height))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .default_size([width as f32, height as f32])
        .show(ui.ctx(), |ui| {
            // Calculate maximum display size based on available space
            let available_size = ui.available_size();
            let aspect_ratio = width as f32 / height as f32;

            let (display_w, display_h) = {
                let max_w = available_size.x.min(width as f32);
                let max_h = available_size.y.min(height as f32);

                if max_w / max_h > aspect_ratio {
                    // Height-constrained
                    (max_h * aspect_ratio, max_h)
                } else {
                    // Width-constrained
                    (max_w, max_w / aspect_ratio)
                }
            };

            ui.image(egui::load::SizedTexture::new(
                texture.id(),
                [display_w, display_h],
            ));
        });

    if !open {
        state.set_maximized(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a simple test ColorImage with the given dimensions.
    fn create_test_image(width: usize, height: usize) -> ColorImage {
        let pixels = vec![Color32::RED; width * height];
        ColorImage::new([width, height], pixels)
    }

    #[test]
    fn test_image_preview_state_new() {
        let state = ImagePreviewState::new();
        assert!(!state.has_image());
        assert!(!state.is_maximized());
    }

    #[test]
    fn test_set_image() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let image = create_test_image(100, 100);
        state.set_image(&ctx, image);

        assert!(state.has_image());
        let entry = state.current_image().unwrap();
        assert_eq!(entry.width, 100);
        assert_eq!(entry.height, 100);
    }

    #[test]
    fn test_set_image_replaces_previous() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        // Set first image
        let image1 = create_test_image(100, 100);
        state.set_image(&ctx, image1);
        assert_eq!(state.current_image().unwrap().width, 100);

        // Set second image - should replace
        let image2 = create_test_image(200, 150);
        state.set_image(&ctx, image2);
        assert_eq!(state.current_image().unwrap().width, 200);
        assert_eq!(state.current_image().unwrap().height, 150);
    }

    #[test]
    fn test_set_image_rgba_valid() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let width = 10;
        let height = 10;
        let rgba_bytes = vec![255u8; width * height * 4];

        let success = state.set_image_rgba(&ctx, width, height, rgba_bytes);
        assert!(success);
        assert!(state.has_image());
    }

    #[test]
    fn test_set_image_rgba_invalid_size() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let width = 10;
        let height = 10;
        // Wrong number of bytes
        let rgba_bytes = vec![255u8; width * height * 3];

        let success = state.set_image_rgba(&ctx, width, height, rgba_bytes);
        assert!(!success);
        assert!(!state.has_image());
    }

    #[test]
    fn test_maximize() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let image = create_test_image(100, 100);
        state.set_image(&ctx, image);

        assert!(!state.is_maximized());

        state.set_maximized(true);
        assert!(state.is_maximized());

        state.set_maximized(false);
        assert!(!state.is_maximized());
    }

    #[test]
    fn test_clear() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let image = create_test_image(100, 100);
        state.set_image(&ctx, image);
        state.set_maximized(true);

        assert!(state.has_image());
        assert!(state.is_maximized());

        state.clear();
        assert!(!state.has_image());
        assert!(!state.is_maximized());
    }
}

/// Widget tests for image_preview rendering and interactions.
#[cfg(test)]
mod image_preview_widget_tests {
    use super::*;
    use egui_kittest::Harness;
    use kittest::Queryable;

    /// Creates a simple test ColorImage with the given dimensions.
    fn create_test_image(width: usize, height: usize) -> ColorImage {
        let pixels = vec![Color32::RED; width * height];
        ColorImage::new([width, height], pixels)
    }

    #[test]
    fn test_image_preview_widget_renders_empty_state() {
        let state = ImagePreviewState::new();

        let harness = Harness::new_ui_state(
            |ui, state: &mut ImagePreviewState| {
                image_preview(state, ui);
            },
            state,
        );

        // Should show the "No image" placeholder text
        assert!(
            harness.query_by_label_contains("No image").is_some(),
            "Empty state should show 'No image' placeholder"
        );
    }

    #[test]
    fn test_image_preview_widget_renders_with_image() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        // Set up an image
        let image = create_test_image(100, 100);
        state.set_image(&ctx, image);

        let mut harness = Harness::new_ui_state(
            |ui, state: &mut ImagePreviewState| {
                image_preview(state, ui);
            },
            state,
        );

        harness.step();

        // Should NOT show the "No image" placeholder when image is present
        assert!(
            harness.query_by_label_contains("No image").is_none(),
            "Should not show 'No image' when an image is set"
        );
    }

    #[test]
    fn test_image_preview_widget_maximized_state() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        // Set up an image and maximize it
        let image = create_test_image(100, 100);
        state.set_image(&ctx, image);
        state.set_maximized(true);

        let mut harness = Harness::new_ui_state(
            |ui, state: &mut ImagePreviewState| {
                image_preview(state, ui);
            },
            state,
        );

        harness.step();

        // When maximized, should show the modal window
        assert!(
            harness.query_by_label_contains("Image Preview").is_some(),
            "Maximized state should show Image Preview window"
        );
    }

    #[test]
    fn test_image_preview_dimensions_stored_correctly() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        // Test various image dimensions
        let test_cases = [(100, 100), (200, 150), (50, 200), (1, 1)];

        for (width, height) in test_cases {
            let image = create_test_image(width, height);
            state.set_image(&ctx, image);

            let entry = state.current_image().unwrap();
            assert_eq!(
                entry.width, width,
                "Width should be stored correctly for {}x{}",
                width, height
            );
            assert_eq!(
                entry.height, height,
                "Height should be stored correctly for {}x{}",
                width, height
            );
        }
    }

    #[test]
    fn test_image_preview_rgba_conversion() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        // Create RGBA bytes for a 2x2 image
        // Red, Green, Blue, White pixels
        let rgba_bytes = vec![
            255, 0, 0, 255, // Red
            0, 255, 0, 255, // Green
            0, 0, 255, 255, // Blue
            255, 255, 255, 255, // White
        ];

        let success = state.set_image_rgba(&ctx, 2, 2, rgba_bytes);
        assert!(success, "Should successfully convert RGBA bytes to image");
        assert!(state.has_image(), "Image should be stored after conversion");

        let entry = state.current_image().unwrap();
        assert_eq!(entry.width, 2);
        assert_eq!(entry.height, 2);
    }
}
