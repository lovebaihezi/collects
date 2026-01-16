//! Add content to an existing collect command.

use std::path::PathBuf;

use anyhow::{Context as _, Result};
use collects_business::{
    AddGroupContentsCommand, AddGroupContentsCompute, AddGroupContentsInput,
    AddGroupContentsStatus, Attachment,
};
use collects_input::{RealStdinReader, StdinReader, clear_clipboard_image};
use collects_states::StateCtx;
use tracing::instrument;
use ustr::Ustr;

use crate::auth::ensure_authenticated;
use crate::context::flush_and_await;
use crate::output::Output;
use crate::utils::{create_contents_for_inputs, read_clipboard_image_if_available};

/// Run the add content command with the default stdin reader.
#[instrument(skip_all, name = "add_collect", fields(collect_id = id.as_str(), file_count = files.len(), stdin))]
pub async fn run_add(ctx: StateCtx, id: String, files: Vec<PathBuf>, stdin: bool) -> Result<()> {
    let reader = RealStdinReader::new();
    run_add_with_reader(ctx, id, files, stdin, reader).await
}

/// Run the add content command with a custom stdin reader.
///
/// This function accepts a generic `StdinReader` implementation, making it
/// testable with mock readers.
#[instrument(skip_all, name = "add_collect_impl", fields(collect_id = id.as_str(), file_count = files.len(), stdin))]
pub async fn run_add_with_reader<R: StdinReader>(
    mut ctx: StateCtx,
    id: String,
    files: Vec<PathBuf>,
    stdin: bool,
    mut reader: R,
) -> Result<()> {
    let out = Output::new();

    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

    let mut body = None;
    if stdin {
        #[cfg(windows)]
        out.info("Reading stdin... Press Ctrl+Z then Enter to finish.");
        #[cfg(not(windows))]
        out.info("Reading stdin... Press Ctrl+D to finish.");

        body = reader.read_body()?;
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

    let content_ids = match create_contents_for_inputs(&mut ctx, None, body, attachments).await {
        Ok(ids) => ids,
        Err(e) => {
            out.error(format!("{e}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    ctx.update::<AddGroupContentsInput>(|s| {
        s.group_id = Some(Ustr::from(&id));
        s.content_ids = content_ids.iter().map(|cid| Ustr::from(cid)).collect();
    });

    ctx.enqueue_command::<AddGroupContentsCommand>();
    flush_and_await(&mut ctx).await;

    match &ctx.compute::<AddGroupContentsCompute>().status {
        AddGroupContentsStatus::Success { added } => {
            out.success(format!("Added {added} item(s) to collect {id}"));
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
