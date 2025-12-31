//! QR code generation utilities.

use egui::{Color32, ColorImage};

/// Generate a QR code image from data.
///
/// Returns a `ColorImage` that can be loaded as a texture in egui.
pub fn generate_qr_image(data: &str, size: usize) -> Option<ColorImage> {
    let code = qrcode::QrCode::new(data.as_bytes()).ok()?;
    let qr_width = code.width();

    // Calculate scale factor to fit the desired size (minimum scale of 1)
    let scale = (size / qr_width).max(1);
    let actual_size = qr_width * scale;

    // Create pixel buffer
    let mut pixels = vec![Color32::WHITE; actual_size * actual_size];

    for (y, row) in code.to_colors().chunks(qr_width).enumerate() {
        for (x, color) in row.iter().enumerate() {
            let pixel_color = match color {
                qrcode::Color::Dark => Color32::BLACK,
                qrcode::Color::Light => Color32::WHITE,
            };

            // Fill scaled pixels
            for dy in 0..scale {
                for dx in 0..scale {
                    let px = x * scale + dx;
                    let py = y * scale + dy;
                    if px < actual_size && py < actual_size {
                        pixels[py * actual_size + px] = pixel_color;
                    }
                }
            }
        }
    }

    Some(ColorImage::new([actual_size, actual_size], pixels))
}
