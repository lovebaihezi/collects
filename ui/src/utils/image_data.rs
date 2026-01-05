//! Generic image data structures for image handling operations.
//!
//! This module provides a common image data structure that can be used across
//! different image sources (clipboard, drag-and-drop, file picker, etc.).

/// Generic image data structure for storing raw image bytes.
///
/// This struct represents decoded image data in a format ready for display.
/// It is source-agnostic and can be used for images from:
/// - Clipboard paste operations
/// - Drag-and-drop file operations
/// - File picker operations
/// - Network downloads
///
/// # Format
///
/// The bytes are expected to be in RGBA format (4 bytes per pixel) or
/// RGB format (3 bytes per pixel). Use `bytes_per_pixel()` to determine
/// the format.
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Width of the image in pixels
    pub width: usize,
    /// Height of the image in pixels
    pub height: usize,
    /// Raw image bytes (typically RGBA or RGB format)
    pub bytes: Vec<u8>,
}

impl ImageData {
    /// Create a new ImageData with the given dimensions and bytes.
    pub fn new(width: usize, height: usize, bytes: Vec<u8>) -> Self {
        Self {
            width,
            height,
            bytes,
        }
    }

    /// Returns the number of bytes per pixel based on the image dimensions.
    ///
    /// Returns `Some(4)` for RGBA, `Some(3)` for RGB, or `None` if the
    /// byte count doesn't match expected formats.
    pub fn bytes_per_pixel(&self) -> Option<usize> {
        let pixels = self.width.checked_mul(self.height)?;
        if pixels == 0 {
            return None;
        }

        // Check for RGBA (4 bytes per pixel)
        if pixels.checked_mul(4) == Some(self.bytes.len()) {
            return Some(4);
        }

        // Check for RGB (3 bytes per pixel)
        if pixels.checked_mul(3) == Some(self.bytes.len()) {
            return Some(3);
        }

        None
    }

    /// Returns true if the image is in RGBA format.
    pub fn is_rgba(&self) -> bool {
        self.bytes_per_pixel() == Some(4)
    }

    /// Returns true if the image is in RGB format.
    pub fn is_rgb(&self) -> bool {
        self.bytes_per_pixel() == Some(3)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_data_new() {
        let data = ImageData::new(100, 100, vec![0u8; 100 * 100 * 4]);
        assert_eq!(data.width, 100);
        assert_eq!(data.height, 100);
        assert_eq!(data.bytes.len(), 100 * 100 * 4);
    }

    #[test]
    fn test_bytes_per_pixel_rgba() {
        let data = ImageData::new(100, 100, vec![0u8; 100 * 100 * 4]);
        assert_eq!(data.bytes_per_pixel(), Some(4));
        assert!(data.is_rgba());
        assert!(!data.is_rgb());
    }

    #[test]
    fn test_bytes_per_pixel_rgb() {
        let data = ImageData::new(100, 100, vec![0u8; 100 * 100 * 3]);
        assert_eq!(data.bytes_per_pixel(), Some(3));
        assert!(!data.is_rgba());
        assert!(data.is_rgb());
    }

    #[test]
    fn test_bytes_per_pixel_invalid() {
        let data = ImageData::new(100, 100, vec![0u8; 100]); // Invalid size
        assert_eq!(data.bytes_per_pixel(), None);
        assert!(!data.is_rgba());
        assert!(!data.is_rgb());
    }

    #[test]
    fn test_bytes_per_pixel_zero_dimensions() {
        // Width is 0
        let data = ImageData::new(0, 100, vec![]);
        assert_eq!(data.bytes_per_pixel(), None);

        // Height is 0
        let data = ImageData::new(100, 0, vec![]);
        assert_eq!(data.bytes_per_pixel(), None);

        // Both are 0
        let data = ImageData::new(0, 0, vec![]);
        assert_eq!(data.bytes_per_pixel(), None);
    }
}
