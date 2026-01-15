//! Shared assets for the Collects project.
//!
//! This crate provides processed assets (icons, etc.) that are generated at build time
//! and can be used by multiple crates in the workspace (UI, services, etc.).
//!
//! # Icon Variants
//!
//! Icons are automatically transformed based on compile-time environment features:
//! - **Production** (no env feature): Original colored icon
//! - **Non-prod** (`env_test`, `env_nightly`, `env_pr`): Grayscale icon
//! - **Internal** (`env_internal`, `env_test_internal`): Inverted grayscale icon
//!
//! # Usage
//!
//! ```rust,ignore
//! use collects_assets::icon;
//!
//! // Get the appropriate icon for the current environment
//! let icon_bytes: &[u8] = icon::icon();
//!
//! // Or access specific variants directly
//! let original = icon::ICON_ORIGINAL;
//! let grayscale = icon::ICON_GRAYSCALE;
//! let inverted = icon::ICON_INVERTED;
//! ```

pub mod icon;

// Re-export commonly used items at crate root for convenience
#[rustfmt::skip]
pub use icon::{ICON_GRAYSCALE, ICON_INVERTED, ICON_ORIGINAL, IconVariant, icon};
