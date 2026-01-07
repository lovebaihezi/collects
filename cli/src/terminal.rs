//! Terminal image display protocols.
//!
//! This module supports displaying images in terminals that have graphics capabilities:
//! - Kitty graphics protocol (kitty, WezTerm with kitty support)
//! - iTerm2 inline images protocol
//! - Sixel graphics (supported by many terminals)

use anyhow::{Result, bail};
use base64::Engine;
use std::io::{self, Write};

/// Terminal graphics protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Kitty graphics protocol
    Kitty,
    /// iTerm2 inline images protocol
    ITerm,
    /// Sixel graphics protocol
    Sixel,
    /// No graphics support, print info only
    None,
}

/// Detect the graphics protocol supported by the current terminal.
///
/// Detection is based on environment variables:
/// - `TERM_PROGRAM` for iTerm2
/// - `TERM` containing "kitty" for Kitty
/// - `KITTY_WINDOW_ID` for Kitty
/// - `WEZTERM_EXECUTABLE` for WezTerm (supports kitty protocol)
pub fn detect_protocol() -> Protocol {
    // Check for iTerm2
    if let Ok(term_program) = std::env::var("TERM_PROGRAM")
        && term_program.contains("iTerm")
    {
        return Protocol::ITerm;
    }

    // Check for Kitty
    if std::env::var("KITTY_WINDOW_ID").is_ok() {
        return Protocol::Kitty;
    }

    // Check for TERM=*kitty*
    if let Ok(term) = std::env::var("TERM")
        && term.contains("kitty")
    {
        return Protocol::Kitty;
    }

    // Check for WezTerm (supports kitty graphics protocol)
    if std::env::var("WEZTERM_EXECUTABLE").is_ok() {
        return Protocol::Kitty;
    }

    // No supported graphics protocol detected
    Protocol::None
}

/// Display image using Kitty graphics protocol.
///
/// The Kitty graphics protocol transmits images as base64-encoded PNG data
/// using a special escape sequence.
pub fn display_kitty(png_data: &[u8]) -> Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);

    let mut stdout = io::stdout().lock();

    // Kitty graphics protocol format:
    // ESC _ G <control data> ; <payload> ESC \
    // a=T: direct transmission
    // f=100: PNG format
    // m=0 or m=1: more data follows (0 = last chunk, 1 = more chunks)

    // For large images, we need to chunk the data (max 4096 bytes per chunk)
    const CHUNK_SIZE: usize = 4096;

    let chunks: Vec<&str> = encoded
        .as_bytes()
        .chunks(CHUNK_SIZE)
        .map(|chunk| std::str::from_utf8(chunk).expect("base64 is always valid UTF-8"))
        .collect();

    for (i, chunk) in chunks.iter().enumerate() {
        let is_last = i == chunks.len() - 1;
        let m = if is_last { 0 } else { 1 };

        if i == 0 {
            // First chunk includes all the parameters
            write!(stdout, "\x1b_Ga=T,f=100,m={};{}\x1b\\", m, chunk)?;
        } else {
            // Subsequent chunks only need the m parameter
            write!(stdout, "\x1b_Gm={};{}\x1b\\", m, chunk)?;
        }
    }

    // Print a newline after the image
    writeln!(stdout)?;

    stdout.flush()?;

    Ok(())
}

/// Display image using iTerm2 inline images protocol.
///
/// The iTerm2 protocol uses OSC 1337 escape sequences with base64-encoded data.
pub fn display_iterm(png_data: &[u8]) -> Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(png_data);

    let mut stdout = io::stdout().lock();

    // iTerm2 inline images protocol:
    // ESC ] 1337 ; File = [arguments] : <base64 data> BEL
    // Arguments: name=<base64 filename>;size=<bytes>;inline=1
    write!(
        stdout,
        "\x1b]1337;File=inline=1;size={}:{}\x07",
        png_data.len(),
        encoded
    )?;

    // Print a newline after the image
    writeln!(stdout)?;

    stdout.flush()?;

    Ok(())
}

/// Display image using Sixel graphics.
///
/// Sixel is an older bitmap graphics format supported by many terminals.
/// This is a simplified implementation - full sixel support would require
/// proper color quantization.
pub fn display_sixel(width: u32, height: u32, rgba_data: &[u8]) -> Result<()> {
    // Sixel encoding is complex - for now, we'll use a basic implementation
    // that may not work perfectly with all images

    let mut stdout = io::stdout().lock();

    // Start sixel sequence
    // DCS P1 ; P2 ; P3 q
    // P1=0: pixel aspect ratio 2:1
    // P2=1: background color option
    // P3=0: horizontal grid size
    write!(stdout, "\x1bPq")?;

    // Set colors (simplified - using 16 color palette)
    // #n;2;r;g;b sets color n to RGB values (0-100 scale)
    for i in 0..16 {
        let r = ((i >> 2) & 1) * 100;
        let g = ((i >> 1) & 1) * 100;
        let b = (i & 1) * 100;
        write!(stdout, "#{};2;{};{};{}", i, r, g, b)?;
    }

    // Encode pixels in sixel format
    // Each sixel character represents 6 vertical pixels
    // Character value = 63 + bitmap (where bitmap is 6 bits)

    for row_group in 0..height.div_ceil(6) {
        for x in 0..width {
            let mut sixel_value = 0u8;

            for y_offset in 0..6 {
                let y = row_group * 6 + y_offset;
                if y < height {
                    let pixel_idx = ((y * width + x) * 4) as usize;
                    if pixel_idx + 3 < rgba_data.len() {
                        let r = rgba_data[pixel_idx];
                        let g = rgba_data[pixel_idx + 1];
                        let b = rgba_data[pixel_idx + 2];
                        let a = rgba_data[pixel_idx + 3];

                        // Simple threshold: if pixel is not transparent and not too dark
                        if a > 128 && (r > 64 || g > 64 || b > 64) {
                            sixel_value |= 1 << y_offset;
                        }
                    }
                }
            }

            // Use color 15 (white) for set pixels
            write!(stdout, "#15{}", (63 + sixel_value) as char)?;
        }

        // Graphics newline (move to next row of sixels)
        write!(stdout, "-")?;
    }

    // End sixel sequence
    write!(stdout, "\x1b\\")?;
    writeln!(stdout)?;

    stdout.flush()?;

    Ok(())
}

/// Print image information without displaying it.
pub fn print_info(width: u32, height: u32, format: &str) -> Result<()> {
    println!("Image from clipboard:");
    println!("  Size: {}x{} pixels", width, height);
    println!("  Format: {}", format);
    println!("\nNote: Your terminal does not support inline images.");
    println!("Use --format=file --output=<path> to save the image to a file.");

    Ok(())
}

/// Parse the format string into a Protocol.
pub fn parse_format(format: &str) -> Result<Protocol> {
    match format.to_lowercase().as_str() {
        "auto" => Ok(detect_protocol()),
        "kitty" => Ok(Protocol::Kitty),
        "iterm" | "iterm2" => Ok(Protocol::ITerm),
        "sixel" => Ok(Protocol::Sixel),
        "none" | "info" => Ok(Protocol::None),
        "file" => Ok(Protocol::None), // File output is handled separately
        _ => bail!(
            "Unknown format: {}. Valid options: auto, kitty, iterm, sixel, none, file",
            format
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_format() {
        assert_eq!(parse_format("kitty").unwrap(), Protocol::Kitty);
        assert_eq!(parse_format("KITTY").unwrap(), Protocol::Kitty);
        assert_eq!(parse_format("iterm").unwrap(), Protocol::ITerm);
        assert_eq!(parse_format("iterm2").unwrap(), Protocol::ITerm);
        assert_eq!(parse_format("sixel").unwrap(), Protocol::Sixel);
        assert_eq!(parse_format("none").unwrap(), Protocol::None);
        assert_eq!(parse_format("info").unwrap(), Protocol::None);
        assert_eq!(parse_format("file").unwrap(), Protocol::None);
    }

    #[test]
    fn test_parse_format_invalid() {
        assert!(parse_format("invalid").is_err());
    }
}
