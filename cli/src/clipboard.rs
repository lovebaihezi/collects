//! Clipboard image handling for the CLI.
//!
//! Reads image data from the system clipboard and displays it using
//! the appropriate terminal graphics protocol.

use anyhow::{Context, Result, bail};
use arboard::Clipboard;
use image::{ImageBuffer, Rgba};
use std::path::Path;

use crate::terminal::{self, Protocol};

/// Read image from clipboard and display it.
pub fn show_clipboard_image(format: &str, output_path: Option<&str>) -> Result<()> {
    // Read image from clipboard
    let mut clipboard = Clipboard::new().context("Failed to access clipboard")?;

    let image_data = match clipboard.get_image() {
        Ok(img) => img,
        Err(arboard::Error::ContentNotAvailable) => {
            // Try to get text and check for file:// URI
            if let Ok(text) = clipboard.get_text() {
                if let Some(img) = try_load_image_from_file_uri(&text)? {
                    img
                } else {
                    bail!("No image found in clipboard. Copy an image first.");
                }
            } else {
                bail!("No image found in clipboard. Copy an image first.");
            }
        }
        Err(e) => {
            bail!("Failed to read clipboard: {}", e);
        }
    };

    let width = image_data.width as u32;
    let height = image_data.height as u32;
    let bytes = image_data.bytes.into_owned();

    println!("Found image: {}x{} ({} bytes)", width, height, bytes.len());

    // Handle file output separately
    if format.to_lowercase() == "file" {
        let path = output_path.unwrap_or("clipboard_image.png");
        save_image_to_file(width, height, &bytes, path)?;
        println!("✓ Image saved to: {}", path);
        return Ok(());
    }

    // Determine output protocol
    let protocol = terminal::parse_format(format)?;

    match protocol {
        Protocol::Kitty | Protocol::ITerm => {
            // Convert to PNG for these protocols
            let png_data = encode_as_png(width, height, &bytes)?;

            match protocol {
                Protocol::Kitty => {
                    terminal::display_kitty(&png_data)?;
                }
                Protocol::ITerm => {
                    terminal::display_iterm(&png_data)?;
                }
                _ => unreachable!(),
            }
        }
        Protocol::Sixel => {
            terminal::display_sixel(width, height, &bytes)?;
        }
        Protocol::None => {
            terminal::print_info(width, height, "RGBA")?;

            // If an output path was provided, save the image
            if let Some(path) = output_path {
                save_image_to_file(width, height, &bytes, path)?;
                println!("✓ Image saved to: {}", path);
            }
        }
    }

    Ok(())
}

/// Encode RGBA image data as PNG.
fn encode_as_png(width: u32, height: u32, rgba_data: &[u8]) -> Result<Vec<u8>> {
    // Create image buffer from raw RGBA data
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgba_data.to_vec())
            .context("Failed to create image buffer")?;

    // Encode as PNG using the write_image trait method
    let mut png_data = Vec::new();
    {
        use image::ImageEncoder;
        let encoder = image::codecs::png::PngEncoder::new(&mut png_data);
        encoder
            .write_image(img.as_raw(), width, height, image::ExtendedColorType::Rgba8)
            .context("Failed to encode image as PNG")?;
    }

    Ok(png_data)
}

/// Save RGBA image data to a file.
fn save_image_to_file(width: u32, height: u32, rgba_data: &[u8], path: &str) -> Result<()> {
    let img: ImageBuffer<Rgba<u8>, Vec<u8>> =
        ImageBuffer::from_raw(width, height, rgba_data.to_vec())
            .context("Failed to create image buffer")?;

    img.save(path)
        .with_context(|| format!("Failed to save image to: {}", path))?;

    Ok(())
}

/// Attempt to load an image from a file:// URI in clipboard text.
fn try_load_image_from_file_uri(text: &str) -> Result<Option<arboard::ImageData<'static>>> {
    for line in text.lines() {
        let line = line.trim();

        // Check for file:// prefix
        if !line.to_lowercase().starts_with("file://") {
            continue;
        }

        // Extract path
        let path_str = &line[7..];

        // URL-decode the path
        let decoded = urlencoding::decode(path_str).context("Failed to decode file URI")?;

        let path = Path::new(decoded.as_ref());

        if !path.is_file() {
            continue;
        }

        // Try to load as image
        match image::open(path) {
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (width, height) = rgba.dimensions();

                return Ok(Some(arboard::ImageData {
                    width: width as usize,
                    height: height as usize,
                    bytes: rgba.into_raw().into(),
                }));
            }
            Err(_) => continue,
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_as_png() {
        // Create a small test image (2x2 red pixels)
        let width = 2;
        let height = 2;
        let rgba_data = vec![
            255, 0, 0, 255, // Red
            255, 0, 0, 255, // Red
            255, 0, 0, 255, // Red
            255, 0, 0, 255, // Red
        ];

        let png_data = encode_as_png(width, height, &rgba_data).expect("Should encode");

        // PNG magic bytes
        assert!(png_data.starts_with(&[0x89, 0x50, 0x4E, 0x47]));
    }

    #[test]
    fn test_save_image_to_file() {
        let temp_dir = tempfile::tempdir().expect("Should create temp dir");
        let path = temp_dir.path().join("test.png");

        // Create a small test image
        let width = 2;
        let height = 2;
        let rgba_data = vec![
            0, 255, 0, 255, // Green
            0, 255, 0, 255, // Green
            0, 255, 0, 255, // Green
            0, 255, 0, 255, // Green
        ];

        save_image_to_file(width, height, &rgba_data, path.to_str().unwrap()).expect("Should save");

        assert!(path.exists());
    }
}
