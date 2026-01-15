//! Icon processing module for build-time icon generation.
//!
//! This module handles environment-based icon variants:
//! - Production: Original colored icon
//! - Non-prod (test, nightly, pr): Grayscale icon
//! - Internal (internal, test-internal): Inverted grayscale icon
//!
//! All icon variants are generated at build time and embedded via `include_bytes!`.

/// Icon variant based on environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IconVariant {
    /// Original colored icon (production)
    Original,
    /// Grayscale icon (non-prod environments)
    Grayscale,
    /// Inverted grayscale icon (internal environments)
    Inverted,
}

impl IconVariant {
    /// Determines the icon variant based on compile-time feature flags.
    #[must_use]
    pub const fn from_features() -> Self {
        // Internal environments get inverted grayscale
        if cfg!(feature = "env_internal") || cfg!(feature = "env_test_internal") {
            return Self::Inverted;
        }

        // Non-prod environments (test, nightly, pr) get grayscale
        if cfg!(feature = "env_test") || cfg!(feature = "env_nightly") || cfg!(feature = "env_pr") {
            return Self::Grayscale;
        }

        // Production (no env feature) gets original
        Self::Original
    }

    /// Returns the filename suffix for this variant.
    #[must_use]
    pub const fn suffix(&self) -> &'static str {
        match self {
            Self::Original => "original",
            Self::Grayscale => "grayscale",
            Self::Inverted => "inverted",
        }
    }
}

/// Original icon PNG bytes (256x256).
pub const ICON_ORIGINAL: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon_original.png"));

/// Grayscale icon PNG bytes (256x256).
pub const ICON_GRAYSCALE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon_grayscale.png"));

/// Inverted grayscale icon PNG bytes (256x256).
pub const ICON_INVERTED: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/icon_inverted.png"));

/// Returns the appropriate icon bytes based on compile-time environment features.
///
/// - Production: Original colored icon
/// - Non-prod (test, nightly, pr): Grayscale icon
/// - Internal (internal, test-internal): Inverted grayscale icon
#[must_use]
pub const fn icon() -> &'static [u8] {
    match IconVariant::from_features() {
        IconVariant::Original => ICON_ORIGINAL,
        IconVariant::Grayscale => ICON_GRAYSCALE,
        IconVariant::Inverted => ICON_INVERTED,
    }
}

/// Returns the icon variant currently selected based on features.
#[must_use]
pub const fn current_variant() -> IconVariant {
    IconVariant::from_features()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_bytes_not_empty() {
        assert!(!ICON_ORIGINAL.is_empty());
        assert!(!ICON_GRAYSCALE.is_empty());
        assert!(!ICON_INVERTED.is_empty());
    }

    #[test]
    fn test_icon_function_returns_valid_bytes() {
        let bytes = icon();
        assert!(!bytes.is_empty());
        // PNG magic bytes
        assert_eq!(&bytes[0..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    }

    #[test]
    fn test_all_variants_are_valid_png() {
        // PNG magic bytes: 0x89 P N G \r \n 0x1A \n
        let png_magic = [137, 80, 78, 71, 13, 10, 26, 10];

        assert_eq!(&ICON_ORIGINAL[0..8], &png_magic);
        assert_eq!(&ICON_GRAYSCALE[0..8], &png_magic);
        assert_eq!(&ICON_INVERTED[0..8], &png_magic);
    }

    #[test]
    fn test_variant_suffix() {
        assert_eq!(IconVariant::Original.suffix(), "original");
        assert_eq!(IconVariant::Grayscale.suffix(), "grayscale");
        assert_eq!(IconVariant::Inverted.suffix(), "inverted");
    }
}
