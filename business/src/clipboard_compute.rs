//! Clipboard compute for managing clipboard image data.
//!
//! This module provides a Compute for storing clipboard image data,
//! allowing async operations to update the clipboard state via Updater.

use std::any::Any;

use collects_states::{Compute, ComputeDeps, Dep, Updater, assign_impl};

/// Clipboard image data stored in a Compute.
#[derive(Debug, Clone)]
pub struct ClipboardImageData {
    /// Width of the image in pixels
    pub width: usize,
    /// Height of the image in pixels
    pub height: usize,
    /// Raw image bytes (RGBA format)
    pub bytes: Vec<u8>,
}

/// Compute for clipboard image state.
///
/// This Compute stores the most recent clipboard image read.
/// It's updated via Updater from async clipboard operations.
#[derive(Debug, Clone, Default)]
pub struct ClipboardCompute {
    /// The current clipboard image, if any
    pub image: Option<ClipboardImageData>,
}

impl ClipboardCompute {
    /// Create a new empty clipboard compute
    pub fn new() -> Self {
        Self { image: None }
    }

    /// Create a clipboard compute with an image
    pub fn with_image(width: usize, height: usize, bytes: Vec<u8>) -> Self {
        Self {
            image: Some(ClipboardImageData {
                width,
                height,
                bytes,
            }),
        }
    }

    /// Get the current clipboard image
    pub fn get_image(&self) -> Option<&ClipboardImageData> {
        self.image.as_ref()
    }

    /// Take the clipboard image, leaving None in its place
    pub fn take_image(&mut self) -> Option<ClipboardImageData> {
        self.image.take()
    }

    /// Clear the clipboard image
    pub fn clear(&mut self) {
        self.image = None;
    }
}

impl Compute for ClipboardCompute {
    fn deps(&self) -> ComputeDeps {
        // No dependencies - this is a pure state holder
        (&[], &[])
    }

    fn compute(&self, _deps: Dep, _updater: Updater) {
        // No-op: This compute is only updated via updater.set() from async operations
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
        assign_impl(self, new_self);
    }
}
