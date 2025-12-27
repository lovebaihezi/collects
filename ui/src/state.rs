use collects_business::{ApiStatus, AuthState, BusinessConfig};
use collects_states::{StateCtx, Time};

use crate::widgets::{LoginDialogState, LoginResultReceiver, LoginResultSender, create_login_channel};

/// The main application state.
///
/// Note: We manually implement Default because the login result channels
/// don't implement Default.
pub struct State {
    /// The state context for business logic.
    pub ctx: StateCtx,
    /// The current authentication state.
    pub auth_state: AuthState,
    /// The login dialog state.
    pub login_dialog_state: LoginDialogState,
    /// Sender for login result communication.
    pub login_result_sender: LoginResultSender,
    /// Receiver for login result communication.
    pub login_result_receiver: LoginResultReceiver,
}

impl Default for State {
    fn default() -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::default());
        ctx.record_compute(ApiStatus::default());

        let (login_result_sender, login_result_receiver) = create_login_channel();

        Self {
            ctx,
            auth_state: AuthState::default(),
            login_dialog_state: LoginDialogState::default(),
            login_result_sender,
            login_result_receiver,
        }
    }
}

impl State {
    pub fn test(base_url: String) -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::new(base_url));
        ctx.record_compute(ApiStatus::default());

        let (login_result_sender, login_result_receiver) = create_login_channel();

        Self {
            ctx,
            auth_state: AuthState::default(),
            login_dialog_state: LoginDialogState::default(),
            login_result_sender,
            login_result_receiver,
        }
    }
}
