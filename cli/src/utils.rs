use anyhow::Result;
use collects_business::{
    Attachment, ContentCreationStatus, CreateContentCommand, CreateContentCompute,
    CreateContentInput,
};
use collects_input::{ClipboardProvider as _, SystemClipboard};
use collects_states::StateCtx;
use tracing::{info, instrument};

use crate::context::flush_and_await;

/// Attempts to read an image from the clipboard, returning None on failure.
pub fn read_clipboard_image_if_available() -> Option<Attachment> {
    let clipboard = SystemClipboard;
    match clipboard.get_image() {
        Ok(Some(image)) => {
            info!(
                "Clipboard image found: {}x{} ({})",
                image.width, image.height, image.filename
            );
            Some(Attachment {
                filename: image.filename,
                mime_type: image.mime_type,
                data: image.bytes,
            })
        }
        Ok(None) => {
            log::debug!("No image in clipboard");
            None
        }
        Err(e) => {
            log::debug!("Failed to read clipboard: {e}");
            None
        }
    }
}

#[instrument(skip_all, name = "create_contents", fields(has_body = body.is_some(), attachment_count = attachments.len()))]
pub async fn create_contents_for_inputs(
    ctx: &mut StateCtx,
    title: Option<String>,
    body: Option<String>,
    attachments: Vec<Attachment>,
) -> Result<Vec<String>> {
    ctx.update::<CreateContentInput>(|s| {
        s.title = title;
        s.description = None;
        s.body = body;
        s.attachments = attachments;
    });

    ctx.enqueue_command::<CreateContentCommand>();
    flush_and_await(ctx).await;

    let compute = ctx.compute::<CreateContentCompute>();
    match &compute.status {
        ContentCreationStatus::Success(ids) => Ok(ids.clone()),
        ContentCreationStatus::Error(e) => {
            Err(anyhow::anyhow!(format!("Error creating content: {e}")))
        }
        _ => Err(anyhow::anyhow!("Content creation did not complete")),
    }
}

pub fn truncate_preview(text: &str, max_len: usize) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    if trimmed.chars().count() <= max_len {
        return trimmed.to_owned();
    }
    let truncated: String = trimmed.chars().take(max_len - 1).collect();
    format!("{truncated}…")
}

pub fn format_size(bytes: i64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}
