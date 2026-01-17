//! Clipboard image state for storing original image payload + UI preview cache.
//!
//! Goals:
//! - Store the original encoded bytes from clipboard (or a synthesized fallback) without
//!   forcing a decode/re-encode on the state boundary.
//! - Provide a separate RGBA8 preview cache that the UI can populate (decoded/downconverted)
//!   for egui texture upload.
//!
//! Non-goals:
//! - Actually reading the system clipboard (belongs in `collects-input`).
//! - Decoding image formats (UI can use `image` crate or platform APIs).
//!
//! This module is placed in `collects-states` so the *data model* is shared and can be
//! stored/updated via `StateCtx`.
//!
//! Note: The preview cache is plain bytes (RGBA8) rather than an `egui::TextureHandle`
//! because `collects-states` must not depend on UI crates.

use std::any::Any;

use crate::{SnapshotClone, State};

/// Encoded clipboard image payload (ideal for storage/export).
///
/// This is intended to carry (best-effort) the original encoded bytes
/// (e.g. PNG/JPEG/TIFF/WebP/...) plus metadata that helps the UI and persistence layers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClipboardImagePayload {
    /// Encoded bytes (original when possible).
    pub bytes: Vec<u8>,
    /// MIME type for `bytes` (best-effort).
    ///
    /// Examples: `image/png`, `image/jpeg`, `image/webp`, `image/tiff`.
    pub mime_type: String,
    /// Suggested filename.
    pub filename: String,
    /// True if the payload was synthesized (e.g. bitmap -> encoded PNG) because the platform
    /// didn't provide original encoded bytes.
    pub synthesized: bool,
}

/// Raw RGBA8 preview for UI rendering.
///
/// The UI can generate/downconvert this from [`ClipboardImagePayload`] and cache it here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardImagePreviewRgba8 {
    pub width: usize,
    pub height: usize,
    /// RGBA8, row-major, tightly packed: `width * height * 4`.
    pub bytes: Vec<u8>,
}

impl ClipboardImagePreviewRgba8 {
    /// Validate byte length matches `width * height * 4`.
    pub fn is_valid(&self) -> bool {
        self.width
            .checked_mul(self.height)
            .and_then(|px| px.checked_mul(4))
            .is_some_and(|expected| expected == self.bytes.len())
    }
}

/// State: clipboard image payload + preview cache.
///
/// This exists as a shared state type so UI can:
/// - set payload on paste
/// - compute a preview later (sync or async) and set it
/// - clear both when needed
#[derive(Debug, Default)]
pub struct ClipboardImageState {
    payload: Option<ClipboardImagePayload>,
    preview_rgba8: Option<ClipboardImagePreviewRgba8>,

    /// A monotonically increasing value that changes whenever the payload changes.
    ///
    /// UI can use this to decide if its derived preview/texture is stale.
    generation: u64,
}

impl ClipboardImageState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Current payload generation. Incremented whenever payload changes.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Returns the current encoded payload (if any).
    pub fn payload(&self) -> Option<&ClipboardImagePayload> {
        self.payload.as_ref()
    }

    /// Returns the cached RGBA8 preview (if any).
    pub fn preview_rgba8(&self) -> Option<&ClipboardImagePreviewRgba8> {
        self.preview_rgba8.as_ref()
    }

    /// Replace payload and clear preview cache.
    ///
    /// This should be called when a new image is pasted.
    pub fn set_payload(&mut self, payload: ClipboardImagePayload) {
        self.payload = Some(payload);
        self.preview_rgba8 = None;
        self.generation = self.generation.saturating_add(1);
    }

    /// Clear payload and preview cache.
    pub fn clear(&mut self) {
        self.payload = None;
        self.preview_rgba8 = None;
        self.generation = self.generation.saturating_add(1);
    }

    /// Set the preview cache if it is consistent and valid.
    ///
    /// If there is no payload, this does nothing.
    /// If `payload_generation` doesn't match current generation, this does nothing.
    ///
    /// This lets you safely publish previews from async tasks: they can check/retain the
    /// generation they started with and only write if still current.
    pub fn set_preview_rgba8_if_current(
        &mut self,
        payload_generation: u64,
        preview: ClipboardImagePreviewRgba8,
    ) -> bool {
        if self.payload.is_none() {
            return false;
        }
        if self.generation != payload_generation {
            return false;
        }
        if !preview.is_valid() {
            return false;
        }

        self.preview_rgba8 = Some(preview);
        true
    }

    /// Returns whether a payload exists.
    pub fn has_payload(&self) -> bool {
        self.payload.is_some()
    }

    /// Returns whether a preview exists.
    pub fn has_preview(&self) -> bool {
        self.preview_rgba8.is_some()
    }
}

// This is a plain state containing bytes; it is safe to snapshot clone.
// (The derive isn't used because `State` is object-safe and SnapshotClone is a trait.)
impl SnapshotClone for ClipboardImageState {}

impl State for ClipboardImageState {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_set_payload_increments_generation_and_clears_preview() {
        let mut s = ClipboardImageState::new();
        assert_eq!(s.generation(), 0);
        assert!(!s.has_payload());
        assert!(!s.has_preview());

        s.preview_rgba8 = Some(ClipboardImagePreviewRgba8 {
            width: 1,
            height: 1,
            bytes: vec![0, 0, 0, 255],
        });

        s.set_payload(ClipboardImagePayload {
            bytes: vec![1, 2, 3],
            mime_type: "image/png".to_owned(),
            filename: "x.png".to_owned(),
            synthesized: false,
        });

        assert!(s.has_payload());
        assert!(!s.has_preview());
        assert_eq!(s.generation(), 1);
    }

    #[test]
    fn test_set_preview_requires_current_generation() {
        let mut s = ClipboardImageState::new();
        s.set_payload(ClipboardImagePayload {
            bytes: vec![1, 2, 3],
            mime_type: "image/png".to_owned(),
            filename: "x.png".to_owned(),
            synthesized: true,
        });

        let generation = s.generation();

        // Wrong generation => rejected
        let ok = s.set_preview_rgba8_if_current(
            generation + 1,
            ClipboardImagePreviewRgba8 {
                width: 1,
                height: 1,
                bytes: vec![0, 0, 0, 255],
            },
        );
        assert!(!ok);
        assert!(!s.has_preview());

        // Correct generation + valid bytes => accepted
        let ok = s.set_preview_rgba8_if_current(
            generation,
            ClipboardImagePreviewRgba8 {
                width: 2,
                height: 1,
                bytes: vec![0, 0, 0, 255, 10, 20, 30, 255],
            },
        );
        assert!(ok);
        assert!(s.has_preview());
    }

    #[test]
    fn test_set_preview_rejects_invalid_byte_len() {
        let mut s = ClipboardImageState::new();
        s.set_payload(ClipboardImagePayload {
            bytes: vec![1, 2, 3],
            mime_type: "image/png".to_owned(),
            filename: "x.png".to_owned(),
            synthesized: false,
        });

        let generation = s.generation();
        let ok = s.set_preview_rgba8_if_current(
            generation,
            ClipboardImagePreviewRgba8 {
                width: 2,
                height: 2,
                bytes: vec![0; 3], // invalid
            },
        );
        assert!(!ok);
        assert!(!s.has_preview());
    }

    #[test]
    fn test_clear_increments_generation_and_removes_all() {
        let mut s = ClipboardImageState::new();
        s.set_payload(ClipboardImagePayload {
            bytes: vec![1],
            mime_type: "image/png".to_owned(),
            filename: "x.png".to_owned(),
            synthesized: true,
        });

        let gen1 = s.generation();
        assert!(s.has_payload());

        s.clear();
        assert!(!s.has_payload());
        assert!(!s.has_preview());
        assert!(s.generation() > gen1);
    }
}
