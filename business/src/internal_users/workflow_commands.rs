//! Workflow commands for internal users state management.
//!
//! These commands encapsulate state mutations that were previously done directly
//! from UI code via `state_ctx.state_mut::<InternalUsersState>()`.
//!
//! By using commands, UI code follows the pattern:
//! - UI sets input state via `ctx.update::<WorkflowInput>(...)`
//! - UI dispatches commands via `ctx.dispatch::<Command>()`
//! - Commands read input and update state/computes
//! - UI reads via `ctx.cached::<Compute>()` or `ctx.state::<State>()`

use collects_states::{Command, Dep, State, Updater};
use std::any::Any;
use ustr::Ustr;

use super::state::{InternalUsersState, UserAction};

/// Input state for workflow commands.
///
/// UI sets these fields before dispatching the corresponding command.
#[derive(Debug, Clone, Default)]
pub struct WorkflowInput {
    /// Action to open (for `OpenInternalUsersActionCommand`).
    pub action: Option<UserAction>,

    /// Username for toggle OTP visibility.
    pub toggle_otp_username: Option<Ustr>,
}

impl State for WorkflowInput {
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Command to open/start an internal users action (modal or inline).
///
/// This replaces direct calls to `state_ctx.state_mut::<InternalUsersState>().start_action(action)`.
///
/// Before dispatching, set `WorkflowInput.action` to the desired action.
#[derive(Debug, Default)]
pub struct OpenInternalUsersActionCommand;

impl Command for OpenInternalUsersActionCommand {
    fn run(&self, deps: Dep, _updater: Updater) {
        let input = deps.get_state_ref::<WorkflowInput>();
        let Some(action) = input.action.clone() else {
            return;
        };

        let state = deps.state_mut::<InternalUsersState>();
        state.start_action(action);
    }
}

/// Command to close the current internal users action.
///
/// This replaces direct calls to `state_ctx.state_mut::<InternalUsersState>().close_action()`.
#[derive(Debug, Default)]
pub struct CloseInternalUsersActionCommand;

impl Command for CloseInternalUsersActionCommand {
    fn run(&self, deps: Dep, _updater: Updater) {
        let state = deps.state_mut::<InternalUsersState>();
        state.close_action();
    }
}

/// Command to toggle OTP visibility for a specific user.
///
/// This replaces direct calls to `state_ctx.state_mut::<InternalUsersState>().toggle_otp_visibility(username)`.
///
/// Before dispatching, set `WorkflowInput.toggle_otp_username` to the target username.
#[derive(Debug, Default)]
pub struct ToggleOtpVisibilityCommand;

impl Command for ToggleOtpVisibilityCommand {
    fn run(&self, deps: Dep, _updater: Updater) {
        let input = deps.get_state_ref::<WorkflowInput>();
        let Some(username) = input.toggle_otp_username else {
            return;
        };

        let state = deps.state_mut::<InternalUsersState>();
        state.toggle_otp_visibility(username);
    }
}

/// Command to open the create user modal.
///
/// This replaces direct calls to `state_ctx.state_mut::<InternalUsersState>().open_create_modal()`.
#[derive(Debug, Default)]
pub struct OpenCreateUserModalCommand;

impl Command for OpenCreateUserModalCommand {
    fn run(&self, deps: Dep, _updater: Updater) {
        let state = deps.state_mut::<InternalUsersState>();
        state.open_create_modal();
    }
}

/// Command to close the create user modal.
///
/// This replaces direct calls to `state_ctx.state_mut::<InternalUsersState>().close_create_modal()`.
#[derive(Debug, Default)]
pub struct CloseCreateUserModalCommand;

impl Command for CloseCreateUserModalCommand {
    fn run(&self, deps: Dep, _updater: Updater) {
        let state = deps.state_mut::<InternalUsersState>();
        state.close_create_modal();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use collects_states::StateCtx;

    /// Helper to create a StateCtx with all necessary states and commands registered.
    fn setup_ctx() -> StateCtx {
        let mut ctx = StateCtx::new();
        ctx.add_state(InternalUsersState::default());
        ctx.add_state(WorkflowInput::default());
        ctx.record_command(OpenInternalUsersActionCommand);
        ctx.record_command(CloseInternalUsersActionCommand);
        ctx.record_command(ToggleOtpVisibilityCommand);
        ctx.record_command(OpenCreateUserModalCommand);
        ctx.record_command(CloseCreateUserModalCommand);
        ctx
    }

    #[test]
    fn test_open_action_command() {
        let mut ctx = setup_ctx();

        // Initially no action
        let state = ctx.state_mut::<InternalUsersState>();
        assert!(matches!(state.current_action, UserAction::None));

        // Set input and dispatch
        ctx.update::<WorkflowInput>(|input| {
            input.action = Some(UserAction::EditUsername(Ustr::from("alice")));
        });
        ctx.dispatch::<OpenInternalUsersActionCommand>();

        let state = ctx.state_mut::<InternalUsersState>();
        assert!(matches!(
            state.current_action,
            UserAction::EditUsername(u) if u == Ustr::from("alice")
        ));
    }

    #[test]
    fn test_close_action_command() {
        let mut ctx = setup_ctx();

        // Set up an active action
        let state = ctx.state_mut::<InternalUsersState>();
        state.start_action(UserAction::DeleteUser(Ustr::from("bob")));
        assert!(matches!(state.current_action, UserAction::DeleteUser(_)));

        // Dispatch close action command
        ctx.dispatch::<CloseInternalUsersActionCommand>();

        let state = ctx.state_mut::<InternalUsersState>();
        assert!(matches!(state.current_action, UserAction::None));
    }

    #[test]
    fn test_toggle_otp_visibility_command() {
        let mut ctx = setup_ctx();
        let username = Ustr::from("charlie");

        // Initially not revealed
        let state = ctx.state_mut::<InternalUsersState>();
        assert!(!state.is_otp_revealed(username.as_str()));

        // Toggle on
        ctx.update::<WorkflowInput>(|input| {
            input.toggle_otp_username = Some(username);
        });
        ctx.dispatch::<ToggleOtpVisibilityCommand>();

        let state = ctx.state_mut::<InternalUsersState>();
        assert!(state.is_otp_revealed(username.as_str()));

        // Toggle off (input is still set from previous update)
        ctx.dispatch::<ToggleOtpVisibilityCommand>();

        let state = ctx.state_mut::<InternalUsersState>();
        assert!(!state.is_otp_revealed(username.as_str()));
    }

    #[test]
    fn test_open_create_user_modal_command() {
        let mut ctx = setup_ctx();

        // Initially closed
        let state = ctx.state_mut::<InternalUsersState>();
        assert!(!state.create_modal_open);

        // Open modal
        ctx.dispatch::<OpenCreateUserModalCommand>();

        let state = ctx.state_mut::<InternalUsersState>();
        assert!(state.create_modal_open);
    }

    #[test]
    fn test_close_create_user_modal_command() {
        let mut ctx = setup_ctx();

        // Open modal first
        let state = ctx.state_mut::<InternalUsersState>();
        state.open_create_modal();
        state.new_username = "test_user".to_string();
        assert!(state.create_modal_open);

        // Close modal
        ctx.dispatch::<CloseCreateUserModalCommand>();

        let state = ctx.state_mut::<InternalUsersState>();
        assert!(!state.create_modal_open);
        assert!(state.new_username.is_empty()); // Should be cleared
    }
}
