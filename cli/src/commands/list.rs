//! List collects command.

use anyhow::{Context as _, Result};
use collects_business::{
    BusinessConfig, GetGroupContentsCommand, GetGroupContentsCompute, GetGroupContentsInput,
    GetGroupContentsStatus, GroupItem, ListGroupsCommand, ListGroupsCompute, ListGroupsInput,
    ListGroupsStatus,
};
use collects_states::StateCtx;
use inquire::Select;
use tabled::settings::Style;
use tabled::{Table, Tabled};
use tracing::instrument;
use ustr::Ustr;

use crate::auth::ensure_authenticated;
use crate::commands::view::run_view;
use crate::context::{build_state_ctx, flush_and_await};
use crate::output::Output;

#[allow(clippy::too_many_lines)]
#[instrument(skip_all, name = "list", fields(limit, offset, status = status.as_deref().unwrap_or("all")))]
pub async fn run_list(
    mut ctx: StateCtx,
    limit: i32,
    offset: i32,
    status: Option<String>,
    interactive: bool,
) -> Result<()> {
    let out = Output::new();

    // Ensure authenticated (prompts for login if needed)
    ensure_authenticated(&mut ctx).await?;

    ctx.update::<ListGroupsInput>(|s| {
        s.limit = Some(limit);
        s.offset = Some(offset);
        s.status = status.map(|st| Ustr::from(&st));
    });

    ctx.enqueue_command::<ListGroupsCommand>();
    flush_and_await(&mut ctx).await;

    let compute = ctx.compute::<ListGroupsCompute>();
    let groups = match &compute.status {
        ListGroupsStatus::Success(groups) => groups.clone(),
        ListGroupsStatus::Error(e) => {
            out.error(format!("Error listing collects: {e}"));
            ctx.shutdown().await;
            std::process::exit(1);
        }
        _ => {
            out.error("List operation did not complete");
            ctx.shutdown().await;
            std::process::exit(1);
        }
    };

    if groups.is_empty() {
        out.dim("No collects found.");
        ctx.shutdown().await;
        return Ok(());
    }

    // Fetch file counts for each group (Option 2: N+1 calls)
    let mut group_file_counts: Vec<(GroupItem, usize)> = Vec::new();
    for group in &groups {
        ctx.update::<GetGroupContentsInput>(|s| {
            s.group_id = Some(group.id);
        });
        ctx.enqueue_command::<GetGroupContentsCommand>();
        flush_and_await(&mut ctx).await;

        let contents_compute = ctx.compute::<GetGroupContentsCompute>();
        let file_count = match &contents_compute.status {
            GetGroupContentsStatus::Success(items) => items.len(),
            _ => 0,
        };
        group_file_counts.push((group.clone(), file_count));
    }

    if interactive {
        // Interactive mode: let user select collect to view
        let options: Vec<String> = group_file_counts
            .iter()
            .map(|(group, file_count)| {
                format!("📁 {} ({} files) [{}]", group.name, file_count, group.id)
            })
            .collect();

        let selection = Select::new("Select collect to view:", options)
            .with_help_message("Use arrow keys to navigate, Enter to select")
            .prompt_skippable()
            .context("Failed to select collect")?;

        if let Some(selected) = selection {
            // Extract ID from the selected string
            if let Some(id_start) = selected.rfind('[') {
                let id = &selected[id_start + 1..selected.len() - 1];
                ctx.shutdown().await;
                // Re-create context for view
                let new_ctx = build_state_ctx(BusinessConfig::default());
                return run_view(new_ctx, Some(id.to_owned())).await;
            }
        }
    } else {
        // Non-interactive: just print the list using tabled
        #[derive(Tabled)]
        struct ListRow {
            #[tabled(rename = "ID")]
            id: String,
            #[tabled(rename = "Name")]
            name: String,
            #[tabled(rename = "Description")]
            description: String,
            #[tabled(rename = "Files")]
            file_count: usize,
            #[tabled(rename = "Status")]
            status: String,
        }

        fn truncate_str(s: &str, max_len: usize) -> String {
            if s.chars().count() > max_len {
                let truncated: String = s.chars().take(max_len - 3).collect();
                format!("{truncated}...")
            } else {
                s.to_owned()
            }
        }

        let rows: Vec<ListRow> = group_file_counts
            .iter()
            .map(|(group, file_count)| ListRow {
                id: group.id.to_string(),
                name: truncate_str(group.name.as_str(), 24),
                description: group
                    .description
                    .as_ref()
                    .map(|d| truncate_str(d.as_str(), 24))
                    .unwrap_or_default(),
                file_count: *file_count,
                status: group.status.to_string(),
            })
            .collect();

        let mut table = Table::new(&rows);
        table.with(Style::rounded());
        out.newline();
        out.print(table.to_string());
        out.total("Total", groups.len());
    }

    ctx.shutdown().await;
    Ok(())
}
