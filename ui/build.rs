//! Build script for collects-ui.
//!
//! This script handles platform-specific icon processing:
//! - On all platforms: Copies the environment-appropriate icon from collects-assets
//! - On Windows: Additionally generates ICO file and compiles Windows resource

use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");

    // Determine which icon variant to use based on environment features
    let variant = get_icon_variant();

    // Copy the appropriate pre-generated icon from collects-assets to our OUT_DIR
    // The assets crate generates all variants, we just pick the right one
    copy_icon_from_assets(&out_dir, variant);

    // Windows-specific: generate ICO and compile resource
    #[cfg(target_os = "windows")]
    {
        generate_windows_ico(&out_dir);
        compile_windows_resource(&out_dir);
    }
}

/// Icon variant based on environment features.
#[derive(Debug, Clone, Copy)]
enum IconVariant {
    Original,
    Grayscale,
    Inverted,
}

impl IconVariant {
    fn source_filename(&self) -> &'static str {
        match self {
            Self::Original => "icon_original.png",
            Self::Grayscale => "icon_grayscale.png",
            Self::Inverted => "icon_inverted.png",
        }
    }
}

/// Determines the icon variant based on Cargo feature flags.
fn get_icon_variant() -> IconVariant {
    // Print rerun directives for feature changes
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_INTERNAL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_TEST_INTERNAL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_TEST");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_NIGHTLY");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_PR");

    // Internal environments get inverted grayscale
    if env::var("CARGO_FEATURE_ENV_INTERNAL").is_ok()
        || env::var("CARGO_FEATURE_ENV_TEST_INTERNAL").is_ok()
    {
        return IconVariant::Inverted;
    }

    // Non-prod environments (test, nightly, pr) get grayscale
    if env::var("CARGO_FEATURE_ENV_TEST").is_ok()
        || env::var("CARGO_FEATURE_ENV_NIGHTLY").is_ok()
        || env::var("CARGO_FEATURE_ENV_PR").is_ok()
    {
        return IconVariant::Grayscale;
    }

    // Production (no env feature) gets original
    IconVariant::Original
}

/// Copies the appropriate icon variant from collects-assets OUT_DIR to ui OUT_DIR.
fn copy_icon_from_assets(out_dir: &str, variant: IconVariant) {
    // The collects-assets crate's OUT_DIR contains the generated icons
    // We need to find it via the DEP_ environment variable set by cargo
    // However, since we're a build dependency consumer, we need a different approach

    // Since collects-assets is a dependency, we can find its OUT_DIR through cargo
    // For now, we'll regenerate the icon here using the same logic
    // This is a bit redundant but ensures the icon is always available

    let assets_icon_path = find_assets_icon(variant);
    let out_icon_path = Path::new(out_dir).join("icon.png");

    std::fs::copy(&assets_icon_path, &out_icon_path).unwrap_or_else(|e| {
        panic!(
            "Failed to copy icon from {:?} to {:?}: {}",
            assets_icon_path, out_icon_path, e
        )
    });
}

/// Finds the collects-assets icon path.
/// Falls back to regenerating if the assets OUT_DIR isn't accessible.
fn find_assets_icon(variant: IconVariant) -> std::path::PathBuf {
    // Try to use the icon from collects-assets via cargo's DEP_ mechanism
    // The assets crate would need to emit cargo:root= for this to work
    // For simplicity, we'll check if the assets crate's output exists in a known location

    // First, try the manifest dir approach (assets crate is a sibling)
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    let assets_res = Path::new(&manifest_dir)
        .parent()
        .expect("No parent dir")
        .join("assets")
        .join("res")
        .join("icon-256.png");

    // Rerun if source icon changes
    println!("cargo:rerun-if-changed={}", assets_res.display());

    // Since we can't easily access collects-assets' OUT_DIR from here,
    // we'll process the icon directly (same logic as assets crate)
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_path = Path::new(&out_dir).join(variant.source_filename());

    // Generate the icon variant
    let img = image::open(&assets_res).expect("Failed to open icon PNG from assets");
    let rgba_img = img.to_rgba8();

    let processed = match variant {
        IconVariant::Original => rgba_img,
        IconVariant::Grayscale => to_grayscale(&rgba_img),
        IconVariant::Inverted => to_inverted_grayscale(&rgba_img),
    };

    processed
        .save(&out_path)
        .expect("Failed to save processed icon");

    out_path
}

/// Calculates luminance from RGB values.
#[inline]
fn rgb_to_luminance(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)) as u8
}

/// Converts an RGBA image to grayscale while preserving alpha.
fn to_grayscale(
    img: &image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
) -> image::ImageBuffer<image::Rgba<u8>, Vec<u8>> {
    use image::{ImageBuffer, Rgba};

    let (width, height) = img.dimensions();
    let mut output = ImageBuffer::new(width, height);

    for (x, y, pixel) in img.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        let gray = rgb_to_luminance(r, g, b);
        output.put_pixel(x, y, Rgba([gray, gray, gray, a]));
    }

    output
}

/// Converts an RGBA image to inverted grayscale while preserving alpha.
fn to_inverted_grayscale(
    img: &image::ImageBuffer<image::Rgba<u8>, Vec<u8>>,
) -> image::ImageBuffer<image::Rgba<u8>, Vec<u8>> {
    use image::{ImageBuffer, Rgba};

    let (width, height) = img.dimensions();
    let mut output = ImageBuffer::new(width, height);

    for (x, y, pixel) in img.enumerate_pixels() {
        let [r, g, b, a] = pixel.0;
        let inverted = 255 - rgb_to_luminance(r, g, b);
        output.put_pixel(x, y, Rgba([inverted, inverted, inverted, a]));
    }

    output
}

/// Generates the Windows ICO file from the processed PNG.
#[cfg(target_os = "windows")]
fn generate_windows_ico(out_dir: &str) {
    use std::fs::File;
    use std::io::BufWriter;

    let png_path = Path::new(out_dir).join("icon.png");
    let ico_path = Path::new(out_dir).join("icon.ico");

    let img = image::open(&png_path).expect("Failed to open processed icon PNG");
    let rgba_img = img.to_rgba8();

    let ico_file = File::create(&ico_path).expect("Failed to create ICO file");
    let mut ico_writer = BufWriter::new(ico_file);

    let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
    let (width, height) = rgba_img.dimensions();
    let icon_image = ico::IconImage::from_rgba_data(width, height, rgba_img.into_raw());
    icon_dir.add_entry(ico::IconDirEntry::encode(&icon_image).expect("Failed to encode icon"));
    icon_dir
        .write(&mut ico_writer)
        .expect("Failed to write ICO file");
}

/// Compiles Windows resource file for the icon.
#[cfg(target_os = "windows")]
fn compile_windows_resource(out_dir: &str) {
    let ico_path = Path::new(out_dir).join("icon.ico");
    let rc_content = format!("1 ICON \"{}\"", ico_path.display());
    let rc_path = Path::new(out_dir).join("windows-resources.rc");
    std::fs::write(&rc_path, rc_content).expect("Failed to write RC file");

    embed_resource::compile(&rc_path, embed_resource::NONE);
}
