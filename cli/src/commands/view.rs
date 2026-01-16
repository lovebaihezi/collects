//! View a collect and its contents command.

use anyhow::{Context as _, Result};
use collects_business::{
    GetContentCommand, GetContentCompute, GetContentInput, GetContentStatus,
    GetGroupContentsCommand, GetGroupContentsCompute, GetGroupContentsInput,
    GetGroupContentsStatus,
};
use collects_states::StateCtx;
use inquire::Text;
use tracing::instrument;
use ustr::Ustr;

use crate::auth::ensure_authenticated;
use crate::context::flush_and_await;
use crate::output::Output;
use crate::utils::{format_size, truncate_preview};

#[instrument(skip_all, name = "view", fields(collect_id = id.as_deref().unwrap_or("interactive")))]
pub async fn run_view(mut ctx: StateCtx, id: Option<String>) -> Result<()> {
    let out = Output::new();

    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

    // Get ID interactively if not provided
    let group_id = match id {
        Some(id) => id,
        None => Text::new("Collect ID:")
            .with_help_message("Enter the UUID of the collect to view")
            .prompt()
            .context("Failed to read collect ID")?,
    };

    // Get contents in the group
    ctx.update::<GetGroupContentsInput>(|s| {
        s.group_id = Some(Ustr::from(&group_id));
    });

    ctx.enqueue_command::<GetGroupContentsCommand>();
    flush_and_await(&mut ctx).await;

    let contents_compute = ctx.compute::<GetGroupContentsCompute>();
    let items = match &contents_compute.status {
        GetGroupContentsStatus::Success(items) => items.clone(),
        GetGroupContentsStatus::NotFound => {
            out.error(format!("Collect not found: {group_id}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
        GetGroupContentsStatus::Error(e) => {
            out.error(format!("Error getting collect: {e}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            out.error("Get collect operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    out.collect_header(&group_id);
    out.divider(50);

    if items.is_empty() {
        out.dim("No files in this collect.");
        ctx.shutdown().await;
        return Ok(());
    }

    out.count("Files", items.len());
    out.newline();

    // Fetch details for each content item
    for group_content in &items {
        ctx.update::<GetContentInput>(|s| {
            s.id = group_content.content_id;
        });

        ctx.enqueue_command::<GetContentCommand>();
        flush_and_await(&mut ctx).await;

        let content_compute = ctx.compute::<GetContentCompute>();
        match &content_compute.status {
            GetContentStatus::Success(item) => {
                let size = format_size(item.file_size);

                if item.is_file() {
                    out.file_item(item.title, item.content_type, &size);
                } else {
                    out.text_item(item.title, item.content_type, &size);
                }

                out.labeled_indent("ID", item.id, 5);

                if let Some(desc) = &item.description {
                    out.labeled_indent("Description", desc, 5);
                }

                if item.is_text()
                    && let Some(body) = &item.body
                {
                    let preview = truncate_preview(body, 120);
                    if !preview.is_empty() {
                        out.labeled_indent("Preview", &preview, 5);
                    }
                }
            }
            GetContentStatus::NotFound => {
                out.warning(format!(
                    "  {} (content not found)",
                    group_content.content_id
                ));
            }
            GetContentStatus::Error(e) => {
                out.warning(format!("  {} (error: {})", group_content.content_id, e));
            }
            _ => {}
        }
    }

    ctx.shutdown().await;
    Ok(())
}
