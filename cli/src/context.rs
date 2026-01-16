//! State context initialization and task management utilities.

use collects_business::{
    AddGroupContentsCommand, AddGroupContentsCompute, AddGroupContentsInput, AuthCompute,
    BusinessConfig, CFTokenCompute, CreateContentCommand, CreateContentCompute, CreateContentInput,
    CreateGroupCommand, CreateGroupCompute, CreateGroupInput, GetContentCommand, GetContentCompute,
    GetContentInput, GetGroupContentsCommand, GetGroupContentsCompute, GetGroupContentsInput,
    ListGroupsCommand, ListGroupsCompute, ListGroupsInput, LoginCommand, LoginInput,
    PendingTokenValidation, ValidateTokenCommand,
};
use collects_states::StateCtx;
use tracing::instrument;

/// Initialize `StateCtx` with CLI-relevant states, computes, and commands.
pub fn build_state_ctx(config: BusinessConfig) -> StateCtx {
    let mut ctx = StateCtx::new();

    // Business config
    ctx.add_state(config);

    // Login states and computes
    ctx.add_state(LoginInput::default());
    ctx.add_state(PendingTokenValidation::default());
    ctx.record_compute(CFTokenCompute::default());
    ctx.record_compute(AuthCompute::default());

    // Content creation states and computes
    ctx.add_state(CreateContentInput::default());
    ctx.record_compute(CreateContentCompute::default());

    // Group creation states and computes
    ctx.add_state(CreateGroupInput::default());
    ctx.record_compute(CreateGroupCompute::default());

    // Add-to-group states and computes
    ctx.add_state(AddGroupContentsInput::default());
    ctx.record_compute(AddGroupContentsCompute::default());

    // List groups (collects) states and computes
    ctx.add_state(ListGroupsInput::default());
    ctx.record_compute(ListGroupsCompute::default());

    // Get content states and computes
    ctx.add_state(GetContentInput::default());
    ctx.record_compute(GetContentCompute::default());

    // Get group contents states and computes
    ctx.add_state(GetGroupContentsInput::default());
    ctx.record_compute(GetGroupContentsCompute::default());

    // Commands
    ctx.record_command(LoginCommand);
    ctx.record_command(ValidateTokenCommand);
    ctx.record_command(CreateContentCommand);
    ctx.record_command(CreateGroupCommand);
    ctx.record_command(AddGroupContentsCommand);
    ctx.record_command(ListGroupsCommand);
    ctx.record_command(GetGroupContentsCommand);
    ctx.record_command(GetContentCommand);

    ctx
}

/// Await all pending tasks in the `JoinSet` and sync computes.
#[instrument(skip_all, name = "await_tasks")]
pub async fn await_pending_tasks(ctx: &mut StateCtx) {
    while ctx.task_count() > 0 {
        if ctx.task_set_mut().join_next().await.is_some() {
            ctx.sync_computes();
        }
    }
}

/// Flush commands and await all spawned tasks.
#[instrument(skip_all, name = "flush")]
pub async fn flush_and_await(ctx: &mut StateCtx) {
    ctx.sync_computes();
    ctx.flush_commands();
    await_pending_tasks(ctx).await;
    ctx.sync_computes();
}
