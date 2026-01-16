//! Command for creating a new collect with content.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use collects_business::{
    AddGroupContentsCommand, AddGroupContentsCompute, AddGroupContentsInput,
    AddGroupContentsStatus, Attachment, CreateGroupCommand, CreateGroupCompute, CreateGroupInput,
    CreateGroupStatus,
};
use collects_clipboard::clear_clipboard_image;
use collects_states::StateCtx;
use tracing::instrument;
use ustr::Ustr;

use crate::auth::ensure_authenticated;
use crate::context::flush_and_await;
use crate::output::Output;
use crate::utils::{create_contents_for_inputs, read_clipboard_image_if_available};

#[instrument(skip_all, name = "new_collect", fields(title = %title, file_count = files.len(), stdin))]
pub async fn run_new(
    mut ctx: StateCtx,
    title: String,
    files: Vec<PathBuf>,
    stdin: bool,
) -> Result<()> {
    let out = Output::new();

    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

    let mut body = None;
    if stdin {
        out.info("Reading stdin... Press Ctrl+D to finish.");
        use std::io::Read as _;
        let mut buffer = Vec::new();
        std::io::stdin().read_to_end(&mut buffer)?;
        if !buffer.is_empty() {
            body = Some(String::from_utf8(buffer)?);
        }
    }

    let clipboard_image = read_clipboard_image_if_available();
    let had_clipboard = clipboard_image.is_some();

    if files.is_empty() && body.is_none() && clipboard_image.is_none() {
        out.error("No content to add (no files, stdin, or clipboard image)");
        ctx.shutdown().await;
        std::process::exit(1);
    }

    let mut attachments = Vec::new();
    for path in files {
        let filename = path
            .file_name()
            .context("Invalid filename")?
            .to_string_lossy()
            .to_string();

        let mime_type = mime_guess::from_path(&path)
            .first_or_octet_stream()
            .to_string();

        let data = std::fs::read(&path).context(format!("Failed to read file: {path:?}"))?;

        attachments.push(Attachment {
            filename,
            mime_type,
            data,
        });
    }

    if let Some(clip_attachment) = clipboard_image {
        out.clipboard(&clip_attachment.filename, &clip_attachment.mime_type);
        attachments.push(clip_attachment);
    }

    ctx.update::<CreateGroupInput>(|s| {
        s.name = Some(title.clone());
        s.description = None;
        s.visibility = None;
    });

    ctx.enqueue_command::<CreateGroupCommand>();
    flush_and_await(&mut ctx).await;

    let group = match &ctx.compute::<CreateGroupCompute>().status {
        CreateGroupStatus::Success(group) => group.clone(),
        CreateGroupStatus::Error(e) => {
            out.error(format!("Error creating collect: {e}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            out.error("Collect creation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    let content_ids = match create_contents_for_inputs(&mut ctx, None, body, attachments).await {
        Ok(ids) => ids,
        Err(e) => {
            out.error(format!("{e}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    ctx.update::<AddGroupContentsInput>(|s| {
        s.group_id = Some(group.id);
        s.content_ids = content_ids.iter().map(|id| Ustr::from(id)).collect();
    });

    ctx.enqueue_command::<AddGroupContentsCommand>();
    flush_and_await(&mut ctx).await;

    match &ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            out.success(format!("Collect created: {} ({added} item(s))", group.id));
        }
        AddGroupContentsStatus::Error(e) => {
            out.error(format!("Error adding content to collect: {e}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            out.error("Add-to-collect operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    }

    if had_clipboard && let Err(e) = clear_clipboard_image() {
        log::debug!("Failed to clear clipboard: {e}");
    }

    ctx.shutdown().await;
    Ok(())
}
