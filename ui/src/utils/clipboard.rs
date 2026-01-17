//! Clipboard + paste shortcut handling for the UI.
//!
//! This module is intentionally thin:
//! - Clipboard IO is implemented in `collects-input` (`collects_input::clipboard`)
//! - UI keeps only egui/eframe-specific paste shortcut handling and adapts types
//!
//! Why:
//! - Avoid duplicated clipboard logic across crates
//! - Centralize platform quirks (file:// URI, etc.) in one place (`collects-input`)
//! - UI can downconvert as needed for preview / texture upload

use egui::Context;

pub use collects_input::clipboard::{
    ClipboardError, ClipboardImage, ClipboardImagePayload, ClipboardProvider, SystemClipboard,
};

/// Convert an encoded clipboard payload into the UI's `ClipboardImage`.
///
/// Today the UI pipeline expects a `ClipboardImage`. We keep this adapter here so call sites
/// can be migrated later to use `ClipboardImagePayload` directly.
///
/// Note: width/height are not available from the payload without decoding; set to 0 for now.
/// UI preview can decode `payload.bytes` (using `image` or another decoder) to obtain dimensions.
///
/// This function is available on both native and wasm builds (conversion is pure).
fn payload_to_clipboard_image(payload: ClipboardImagePayload) -> ClipboardImage {
    ClipboardImage {
        width: 0,
        height: 0,
        bytes: payload.bytes,
        mime_type: payload.mime_type,
        filename: payload.filename,
    }
}

/// Handles paste keyboard shortcut (Ctrl+V or Cmd+V) and returns pasted image (legacy type).
///
/// Prefer [`handle_paste_shortcut_payload`] if you want to store the original encoded bytes.
/// This legacy helper exists because the current UI pipeline still expects `ClipboardImage`.
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_paste_shortcut(ctx: &Context) -> Option<ClipboardImage> {
    handle_paste_shortcut_with_clipboard(ctx, &SystemClipboard)
}

/// Same as [`handle_paste_shortcut`], but with an injected clipboard provider (tests).
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_paste_shortcut_with_clipboard<C: ClipboardProvider>(
    ctx: &Context,
    clipboard: &C,
) -> Option<ClipboardImage> {
    handle_paste_shortcut_payload_with_clipboard(ctx, clipboard).map(payload_to_clipboard_image)
}

/// Handles paste keyboard shortcut (Ctrl+V or Cmd+V) and returns the image payload.
///
/// This is the preferred entrypoint when you want to store the original clipboard bytes.
/// The payload preserves the best-effort original encoding (e.g. from `file://`), with a
/// synthesized fallback (e.g. bitmap -> PNG) when necessary.
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_paste_shortcut_payload(ctx: &Context) -> Option<ClipboardImagePayload> {
    handle_paste_shortcut_payload_with_clipboard(ctx, &SystemClipboard)
}

/// Same as [`handle_paste_shortcut_payload`], but with an injected clipboard provider (tests).
#[cfg(not(target_arch = "wasm32"))]
pub fn handle_paste_shortcut_payload_with_clipboard<C: ClipboardProvider>(
    ctx: &Context,
    clipboard: &C,
) -> Option<ClipboardImagePayload> {
    // Work around egui issue where Ctrl+V press may be missing on some Wayland setups:
    // react to the release event for Ctrl+V.
    let paste_triggered = ctx.input_mut(consume_paste_shortcut);

    if !paste_triggered {
        return None;
    }

    // Prefer encoded payload for storage/export.
    // For UI preview, you can decode/downconvert later as needed (egui textures are typically 8-bit).
    match clipboard.get_image_payload() {
        Ok(Some(payload)) => {
            log::trace!(
                target: "collects_ui::paste",
                "clipboard_image_payload_pasted mime_type={} bytes={} filename={} synthesized={}",
                payload.mime_type,
                payload.bytes.len(),
                payload.filename,
                payload.synthesized
            );
            Some(payload)
        }
        Ok(None) => {
            log::trace!(
                target: "collects_ui::paste",
                "clipboard_no_image: clipboard accessible but contains no image data"
            );
            None
        }
        Err(e) => {
            log::warn!(target: "collects_ui::paste", "clipboard_error: {e}");
            None
        }
    }
}

/// Consume Ctrl+V / Cmd+V from egui's event queue.
///
/// - `Ctrl+V`: trigger on key release (Wayland-safe)
/// - `Cmd+V`: trigger on key press
#[cfg(not(target_arch = "wasm32"))]
fn consume_paste_shortcut(input: &mut egui::InputState) -> bool {
    consume_key(
        input,
        egui::Modifiers::CTRL,
        egui::Key::V,
        /*trigger_on_release=*/ true,
    ) || consume_key(
        input,
        egui::Modifiers::COMMAND,
        egui::Key::V,
        /*trigger_on_release=*/ false,
    )
}

/// Consume a specific key event from egui's event queue.
///
/// When `trigger_on_release` is true, we match `pressed: false`.
/// When false, we match `pressed: true`.
#[cfg(not(target_arch = "wasm32"))]
fn consume_key(
    input: &mut egui::InputState,
    mods: egui::Modifiers,
    key: egui::Key,
    trigger_on_release: bool,
) -> bool {
    let mut found = false;

    input.events.retain(|event| {
        let is_match = matches!(
            event,
            egui::Event::Key { key: k, modifiers: m, pressed, .. }
                if *k == key
                    && m.matches_exact(mods)
                    && (*pressed != trigger_on_release)
        );

        found |= is_match;
        !is_match
    });

    found
}

/// WASM stub: web clipboard image access is async and requires secure context + permissions.
///
/// The UI crate currently uses `collects-input` for native clipboard behavior; web support
/// should be implemented as a separate async path (likely using `navigator.clipboard`).
#[cfg(target_arch = "wasm32")]
pub fn handle_paste_shortcut(_ctx: &Context) -> Option<ClipboardImage> {
    None
}

/// WASM stub payload variant.
///
/// Web clipboard image reads require async Clipboard API + permissions; not yet implemented here.
#[cfg(target_arch = "wasm32")]
pub fn handle_paste_shortcut_payload(_ctx: &Context) -> Option<ClipboardImagePayload> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handle_paste_shortcut_no_panic() {
        // Ensures the function can be called without panicking even when no input events exist.
        let ctx = Context::default();
        let _ = handle_paste_shortcut(&ctx);
    }

    // Note: deeper clipboard behaviors (file:// URI handling, image load/encode) are
    // tested in `collects-input`. This UI module only tests shortcut handling glue.
}
