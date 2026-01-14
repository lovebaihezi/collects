//! Icon processing module for build-time icon generation.
//!
//! This module handles environment-based icon variants:
//! - Production: Original colored icon
//! - Non-prod (test, nightly, pr): Grayscale icon
//! - Internal (internal, test-internal): Inverted grayscale icon

use image::{ImageBuffer, Rgba};
use std::env;
use std::path::Path;

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
    /// Determines the icon variant based on environment features.
    pub fn from_env() -> Self {
        // Internal environments get inverted grayscale
        if env::var("CARGO_FEATURE_ENV_INTERNAL").is_ok()
            || env::var("CARGO_FEATURE_ENV_TEST_INTERNAL").is_ok()
        {
            return Self::Inverted;
        }

        // Non-prod environments (test, nightly, pr) get grayscale
        if env::var("CARGO_FEATURE_ENV_TEST").is_ok()
            || env::var("CARGO_FEATURE_ENV_NIGHTLY").is_ok()
            || env::var("CARGO_FEATURE_ENV_PR").is_ok()
        {
            return Self::Grayscale;
        }

        // Production (no env feature) gets original
        Self::Original
    }
}

/// Calculates luminance from RGB values using standard formula: Y = 0.299*R + 0.587*G + 0.114*B
fn rgb_to_luminance(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) as u8
}

/// Converts an RGBA image to grayscale while preserving alpha.
fn to_grayscale(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (width, height) = img.dimensions();
    let mut output = ImageBuffer::new(width, height);

    for (x, y, pixel) in img.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        let gray = rgb_to_luminance(r, g, b);
        output.put_pixel(x, y, Rgba([gray, gray, gray, a]));
    }

    output
}

/// Converts an RGBA image to inverted grayscale (255 - gray) while preserving alpha.
fn to_inverted_grayscale(img: &ImageBuffer<Rgba<u8>, Vec<u8>>) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let (width, height) = img.dimensions();
    let mut output = ImageBuffer::new(width, height);

    for (x, y, pixel) in img.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        let inverted = 255 - rgb_to_luminance(r, g, b);
        output.put_pixel(x, y, Rgba([inverted, inverted, inverted, a]));
    }

    output
}

/// Processes an icon image based on the given variant.
pub fn process_icon(
    img: ImageBuffer<Rgba<u8>, Vec<u8>>,
    variant: IconVariant,
) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    match variant {
        IconVariant::Grayscale => to_grayscale(&img),
        IconVariant::Inverted => to_inverted_grayscale(&img),
        IconVariant::Original => img,
    }
}

/// Generates the icon PNG file in the output directory.
pub fn generate_icon(png_path: &str, out_dir: &str) -> ImageBuffer<Rgba<u8>, Vec<u8>> {
    let out_png_path = Path::new(out_dir).join("icon.png");
    let variant = IconVariant::from_env();

    // Load the original PNG image
    let img = image::open(png_path).expect("Failed to open icon PNG");
    let rgba_img = img.to_rgba8();

    // Apply transformation based on variant
    let processed_img = process_icon(rgba_img, variant);

    // Save the processed image to OUT_DIR
    processed_img
        .save(&out_png_path)
        .expect("Failed to save processed icon PNG");

    processed_img
}

/// Generates the Windows ICO file from a processed image.
#[cfg(target_os = "windows")]
pub fn generate_windows_ico(
    processed_img: ImageBuffer<Rgba<u8>, Vec<u8>>,
    out_dir: &str,
) -> std::path::PathBuf {
    use std::fs::File;
    use std::io::BufWriter;

    let ico_path = Path::new(out_dir).join("icon.ico");

    let ico_file = File::create(&ico_path).expect("Failed to create ICO file");
    let mut ico_writer = BufWriter::new(ico_file);

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    let (width, height) = processed_img.dimensions();
    let icon_image = ico::IconImage::from_rgba_data(width, height, processed_img.into_raw());
    icon_dir.add_entry(ico::IconDirEntry::encode(&icon_image).expect("Failed to encode icon"));
    icon_dir
        .write(&mut ico_writer)
        .expect("Failed to write ICO file");

    ico_path
}

/// Compiles Windows resource file for the icon.
#[cfg(target_os = "windows")]
pub fn compile_windows_resource(ico_path: &std::path::Path, out_dir: &str) {
    let rc_content = format!("1 ICON \"{}\"", ico_path.display());
    let rc_path = Path::new(out_dir).join("windows-resources.rc");
    std::fs::write(&rc_path, rc_content).expect("Failed to write RC file");

    embed_resource::compile(&rc_path, embed_resource::NONE);
}

/// Prints cargo directives for build script rerun conditions.
pub fn print_rerun_directives(png_path: &str) {
    println!("cargo:rerun-if-changed={png_path}");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_INTERNAL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_TEST_INTERNAL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_TEST");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_NIGHTLY");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_PR");
}
