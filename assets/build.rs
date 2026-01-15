//! Build script for collects-assets.
//!
//! Generates all icon variants (original, grayscale, inverted) at build time.

use image::{ImageBuffer, Rgba};
use std::env;
use std::path::Path;

fn main() {
    let png_path = "res/icon-256.png";
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");

    // Print cargo rerun directives
    println!("cargo:rerun-if-changed={png_path}");

    // Load the original PNG image
    let img = image::open(png_path).expect("Failed to open icon PNG");
    let rgba_img = img.to_rgba8();

    // Generate all variants
    generate_variant(&rgba_img, IconVariant::Original, &out_dir);
    generate_variant(&rgba_img, IconVariant::Grayscale, &out_dir);
    generate_variant(&rgba_img, IconVariant::Inverted, &out_dir);
}

/// Icon variant for build-time processing.
#[derive(Debug, Clone, Copy)]
enum IconVariant {
    Original,
    Grayscale,
    Inverted,
}

impl IconVariant {
    fn filename(&self) -> &'static str {
        match self {
            Self::Original => "icon_original.png",
            Self::Grayscale => "icon_grayscale.png",
            Self::Inverted => "icon_inverted.png",
        }
    }
}

/// Generates an icon variant and saves it to the output directory.
fn generate_variant(img: &ImageBuffer<Rgba<u8>, Vec<u8>>, variant: IconVariant, out_dir: &str) {
    let out_path = Path::new(out_dir).join(variant.filename());

    let processed = match variant {
        IconVariant::Original => img.clone(),
        IconVariant::Grayscale => to_grayscale(img),
        IconVariant::Inverted => to_inverted_grayscale(img),
    };

    processed
        .save(&out_path)
        .unwrap_or_else(|e| panic!("Failed to save {:?}: {}", variant, e));
}

/// Calculates luminance from RGB values using standard formula.
///
/// Formula: Y = 0.299*R + 0.587*G + 0.114*B
#[inline]
fn rgb_to_luminance(r: u8, g: u8, b: u8) -> u8 {
    (0.299 * f32::from(r) + 0.587 * f32::from(g) + 0.114 * f32::from(b)) as u8
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
