use image::{ImageBuffer, Rgba};
use std::env;
use std::path::Path;

/// Determines the icon variant based on environment features.
/// Returns: "original" for prod, "grayscale" for non-prod, "inverted" for internal envs.
fn get_icon_variant() -> &'static str {
    // Internal environments get inverted grayscale
    if env::var("CARGO_FEATURE_ENV_INTERNAL").is_ok()
        || env::var("CARGO_FEATURE_ENV_TEST_INTERNAL").is_ok()
    {
        return "inverted";
    }

    // Non-prod environments (test, nightly, pr) get grayscale
    if env::var("CARGO_FEATURE_ENV_TEST").is_ok()
        || env::var("CARGO_FEATURE_ENV_NIGHTLY").is_ok()
        || env::var("CARGO_FEATURE_ENV_PR").is_ok()
    {
        return "grayscale";
    }

    // Production (no env feature) gets original
    "original"
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

fn main() {
    let png_path = "assets/icon-256.png";
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR not set");
    let out_png_path = Path::new(&out_dir).join("icon.png");

    println!("cargo:rerun-if-changed={}", png_path);
    // Rerun if any environment feature changes
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_INTERNAL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_TEST_INTERNAL");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_TEST");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_NIGHTLY");
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_ENV_PR");

    let variant = get_icon_variant();

    // Load the original PNG image
    let img = image::open(png_path).expect("Failed to open icon PNG");
    let rgba_img = img.to_rgba8();

    // Apply transformation based on variant
    let processed_img = match variant {
        "grayscale" => to_grayscale(&rgba_img),
        "inverted" => to_inverted_grayscale(&rgba_img),
        _ => rgba_img, // "original" - no transformation
    };

    // Save the processed image to OUT_DIR
    processed_img
        .save(&out_png_path)
        .expect("Failed to save processed icon PNG");

    #[cfg(target_os = "windows")]
    {
        use std::fs::File;
        use std::io::BufWriter;

        let ico_path = Path::new(&out_dir).join("icon.ico");

        // Create ICO file from processed image
        let ico_file = File::create(&ico_path).expect("Failed to create ICO file");
        let mut ico_writer = BufWriter::new(ico_file);

        let mut icon_dir = ico::IconDir::new(ico::ResourceType::Icon);
        // Capture dimensions before consuming the image
        let (width, height) = processed_img.dimensions();
        let icon_image = ico::IconImage::from_rgba_data(width, height, processed_img.into_raw());
        icon_dir
            .add_entry(ico::IconDirEntry::encode(&icon_image).expect("Failed to encode icon"));
        icon_dir
            .write(&mut ico_writer)
            .expect("Failed to write ICO file");

        // Create Windows resource file pointing to OUT_DIR ico
        let rc_content = format!("1 ICON \"{}\"", ico_path.display());
        let rc_path = Path::new(&out_dir).join("windows-resources.rc");
        std::fs::write(&rc_path, rc_content).expect("Failed to write RC file");

        // Compile the resource file
        embed_resource::compile(&rc_path, embed_resource::NONE);
    }
}
