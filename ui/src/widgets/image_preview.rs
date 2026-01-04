//! Image preview widget for displaying dragged/pasted images in a grid.
//!
//! This module provides:
//! - A ring buffer-based state for storing image textures without network requests
//! - A grid widget for displaying image previews
//! - A maximized view modal for full-size image display
//!
//! # Architecture
//!
//! Images are stored as `egui::TextureHandle` in a lock-free ring buffer.
//! The ring buffer has a fixed capacity and overwrites oldest entries when full.
//! Each image is identified by a unique ID for rendering.
//!
//! # Usage
//!
//! 1. When an image is pasted/dropped, call `ImagePreviewState::add_image()`
//! 2. Call `image_preview_grid()` to render the grid of images
//! 3. Click an image to maximize it in a modal

use collects_states::State;
use egui::{Color32, ColorImage, Context, Response, TextureHandle, TextureOptions, Ui, Window};
use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Maximum number of images to store in the ring buffer.
const MAX_IMAGES: usize = 16;

/// Size of image thumbnails in the grid (pixels).
const THUMBNAIL_SIZE: f32 = 120.0;

/// Spacing between grid items (pixels).
const GRID_SPACING: f32 = 8.0;

/// Global counter for generating unique image IDs.
static IMAGE_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Generate a unique ID for a new image.
fn next_image_id() -> usize {
    IMAGE_ID_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// A single image entry in the preview state.
pub struct ImageEntry {
    /// Unique identifier for this image.
    pub id: usize,
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
            .field("id", &self.id)
            .field("width", &self.width)
            .field("height", &self.height)
            .finish_non_exhaustive()
    }
}

/// State for storing and managing image previews.
///
/// Uses a ring buffer to store a fixed number of images, automatically
/// overwriting the oldest entries when capacity is exceeded.
///
/// This state is designed for zero-copy operations where possible,
/// storing egui texture handles directly.
pub struct ImagePreviewState {
    /// Ring buffer of images. None entries are empty slots.
    images: [Option<ImageEntry>; MAX_IMAGES],
    /// Index of the next slot to write to.
    write_index: usize,
    /// Number of valid images in the buffer.
    count: usize,
    /// Currently maximized image ID (if any).
    maximized_id: Option<usize>,
}

impl Default for ImagePreviewState {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ImagePreviewState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ImagePreviewState")
            .field("count", &self.count)
            .field("write_index", &self.write_index)
            .field("maximized_id", &self.maximized_id)
            .field("image_ids", &self.image_ids())
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
        Self {
            images: std::array::from_fn(|_| None),
            write_index: 0,
            count: 0,
            maximized_id: None,
        }
    }

    /// Add an image to the preview state.
    ///
    /// If the ring buffer is full, the oldest image is replaced.
    ///
    /// # Arguments
    ///
    /// * `ctx` - The egui context for creating textures
    /// * `image` - The color image data to add
    ///
    /// # Returns
    ///
    /// The unique ID assigned to this image.
    pub fn add_image(&mut self, ctx: &Context, image: ColorImage) -> usize {
        let id = next_image_id();
        let width = image.width();
        let height = image.height();

        // Create texture with linear filtering for better quality when scaled
        let texture =
            ctx.load_texture(format!("image_preview_{id}"), image, TextureOptions::LINEAR);

        let entry = ImageEntry {
            id,
            texture,
            width,
            height,
        };

        // Store in ring buffer
        self.images[self.write_index] = Some(entry);
        self.write_index = (self.write_index + 1) % MAX_IMAGES;
        if self.count < MAX_IMAGES {
            self.count += 1;
        }

        id
    }

    /// Add an image from raw RGBA bytes.
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
    /// The unique ID assigned to this image, or None if the data is invalid.
    pub fn add_image_rgba(
        &mut self,
        ctx: &Context,
        width: usize,
        height: usize,
        rgba_bytes: Vec<u8>,
    ) -> Option<usize> {
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
            return None;
        }

        // Convert bytes to Color32 pixels
        let pixels: Vec<Color32> = rgba_bytes
            .chunks_exact(4)
            .map(|chunk| Color32::from_rgba_unmultiplied(chunk[0], chunk[1], chunk[2], chunk[3]))
            .collect();

        let image = ColorImage::new([width, height], pixels);

        Some(self.add_image(ctx, image))
    }

    /// Get the number of images currently stored.
    pub fn len(&self) -> usize {
        self.count
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Get all valid images in order (oldest to newest).
    pub fn images(&self) -> impl Iterator<Item = &ImageEntry> {
        // Calculate the start index (oldest image)
        let start = if self.count < MAX_IMAGES {
            0
        } else {
            self.write_index
        };

        (0..self.count).filter_map(move |i| {
            let idx = (start + i) % MAX_IMAGES;
            self.images[idx].as_ref()
        })
    }

    /// Get all image IDs (for debugging).
    fn image_ids(&self) -> Vec<usize> {
        self.images().map(|e| e.id).collect()
    }

    /// Get an image by its ID.
    pub fn get_image(&self, id: usize) -> Option<&ImageEntry> {
        self.images().find(|e| e.id == id)
    }

    /// Set the maximized image ID.
    pub fn maximize(&mut self, id: usize) {
        self.maximized_id = Some(id);
    }

    /// Clear the maximized state.
    pub fn unmaximize(&mut self) {
        self.maximized_id = None;
    }

    /// Get the currently maximized image ID.
    pub fn maximized_id(&self) -> Option<usize> {
        self.maximized_id
    }

    /// Clear all images.
    pub fn clear(&mut self) {
        for slot in &mut self.images {
            *slot = None;
        }
        self.write_index = 0;
        self.count = 0;
        self.maximized_id = None;
    }
}

/// Renders the image preview grid widget.
///
/// Displays all stored images in a responsive grid layout.
/// Clicking an image maximizes it in a modal overlay.
///
/// # Arguments
///
/// * `state` - Mutable reference to the image preview state
/// * `ui` - The egui UI to render into
///
/// # Returns
///
/// The egui Response from the grid container.
pub fn image_preview_grid(state: &mut ImagePreviewState, ui: &mut Ui) -> Response {
    let available_width = ui.available_width();
    let columns = ((available_width + GRID_SPACING) / (THUMBNAIL_SIZE + GRID_SPACING))
        .floor()
        .max(1.0) as usize;

    // Collect image IDs to avoid borrow issues
    let image_data: Vec<(usize, TextureHandle, usize, usize)> = state
        .images()
        .map(|e| (e.id, e.texture.clone(), e.width, e.height))
        .collect();

    let clicked_id = ui
        .scope(|ui| {
            let mut clicked_id = None;

            if image_data.is_empty() {
                ui.label("No images. Paste (Ctrl+V) or drop images here.");
                return clicked_id;
            }

            egui::Grid::new("image_preview_grid")
                .spacing([GRID_SPACING, GRID_SPACING])
                .show(ui, |ui| {
                    for (i, (id, texture, width, height)) in image_data.iter().enumerate() {
                        // Calculate aspect ratio and thumbnail dimensions
                        let aspect_ratio = *width as f32 / *height as f32;
                        let (thumb_w, thumb_h) = if aspect_ratio > 1.0 {
                            (THUMBNAIL_SIZE, THUMBNAIL_SIZE / aspect_ratio)
                        } else {
                            (THUMBNAIL_SIZE * aspect_ratio, THUMBNAIL_SIZE)
                        };

                        // Create clickable image button using Button::image
                        let sized_texture =
                            egui::load::SizedTexture::new(texture.id(), [thumb_w, thumb_h]);
                        let response = ui.add(
                            egui::Button::image(egui::Image::from_texture(sized_texture))
                                .frame(true)
                                .sense(egui::Sense::click()),
                        );

                        if response.clicked() {
                            clicked_id = Some(*id);
                        }

                        response.on_hover_text(format!("{}×{}", width, height));

                        // End row after `columns` items
                        if (i + 1) % columns == 0 {
                            ui.end_row();
                        }
                    }
                });

            clicked_id
        })
        .inner;

    // Handle click to maximize (after the borrow ends)
    if let Some(id) = clicked_id {
        state.maximize(id);
    }

    // Show maximized modal if needed
    if let Some(maximized_id) = state.maximized_id {
        show_maximized_image_modal(state, maximized_id, ui);
    }

    ui.response()
}

/// Shows the maximized image modal.
fn show_maximized_image_modal(state: &mut ImagePreviewState, image_id: usize, ui: &mut Ui) {
    // Get image data before modal (to avoid borrow issues)
    let image_info = state
        .get_image(image_id)
        .map(|e| (e.texture.clone(), e.width, e.height, e.texture.id()));

    let Some((texture, width, height, _texture_id)) = image_info else {
        // Image was removed, close modal
        state.unmaximize();
        return;
    };

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
        state.unmaximize();
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
        assert_eq!(state.len(), 0);
        assert!(state.is_empty());
        assert!(state.maximized_id().is_none());
    }

    #[test]
    fn test_add_image_increases_count() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let image = create_test_image(100, 100);
        let id = state.add_image(&ctx, image);

        assert_eq!(state.len(), 1);
        assert!(!state.is_empty());
        assert!(state.get_image(id).is_some());
    }

    #[test]
    fn test_add_multiple_images() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let ids: Vec<usize> = (0..5)
            .map(|i| {
                let image = create_test_image(100 + i * 10, 100 + i * 10);
                state.add_image(&ctx, image)
            })
            .collect();

        assert_eq!(state.len(), 5);

        for id in ids {
            assert!(state.get_image(id).is_some());
        }
    }

    #[test]
    fn test_ring_buffer_overflow() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        // Add more images than the buffer can hold
        let mut ids = Vec::new();
        for i in 0..(MAX_IMAGES + 5) {
            let image = create_test_image(50, 50);
            ids.push(state.add_image(&ctx, image));
            assert!(i < MAX_IMAGES || state.len() == MAX_IMAGES);
        }

        // Should have exactly MAX_IMAGES
        assert_eq!(state.len(), MAX_IMAGES);

        // First 5 images should be gone
        for id in &ids[..5] {
            assert!(state.get_image(*id).is_none());
        }

        // Last MAX_IMAGES images should still be present
        for id in &ids[5..] {
            assert!(state.get_image(*id).is_some());
        }
    }

    #[test]
    fn test_add_image_rgba_valid() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let width = 10;
        let height = 10;
        let rgba_bytes = vec![255u8; width * height * 4];

        let id = state.add_image_rgba(&ctx, width, height, rgba_bytes);
        assert!(id.is_some());
        assert_eq!(state.len(), 1);
    }

    #[test]
    fn test_add_image_rgba_invalid_size() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let width = 10;
        let height = 10;
        // Wrong number of bytes
        let rgba_bytes = vec![255u8; width * height * 3];

        let id = state.add_image_rgba(&ctx, width, height, rgba_bytes);
        assert!(id.is_none());
        assert_eq!(state.len(), 0);
    }

    #[test]
    fn test_maximize_unmaximize() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let image = create_test_image(100, 100);
        let id = state.add_image(&ctx, image);

        assert!(state.maximized_id().is_none());

        state.maximize(id);
        assert_eq!(state.maximized_id(), Some(id));

        state.unmaximize();
        assert!(state.maximized_id().is_none());
    }

    #[test]
    fn test_clear() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let mut first_id = 0;
        for i in 0..5 {
            let image = create_test_image(50, 50);
            let id = state.add_image(&ctx, image);
            if i == 0 {
                first_id = id;
            }
        }

        state.maximize(first_id);
        assert_eq!(state.len(), 5);

        state.clear();
        assert_eq!(state.len(), 0);
        assert!(state.is_empty());
        assert!(state.maximized_id().is_none());
    }

    #[test]
    fn test_images_iterator_order() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let ids: Vec<usize> = (0..3)
            .map(|_| {
                let image = create_test_image(50, 50);
                state.add_image(&ctx, image)
            })
            .collect();

        let retrieved_ids: Vec<usize> = state.images().map(|e| e.id).collect();
        assert_eq!(retrieved_ids, ids);
    }

    #[test]
    fn test_unique_ids() {
        let ctx = Context::default();
        let mut state = ImagePreviewState::new();

        let ids: Vec<usize> = (0..10)
            .map(|_| {
                let image = create_test_image(50, 50);
                state.add_image(&ctx, image)
            })
            .collect();

        // All IDs should be unique
        let mut sorted_ids = ids.clone();
        sorted_ids.sort();
        sorted_ids.dedup();
        assert_eq!(sorted_ids.len(), ids.len());
    }
}
