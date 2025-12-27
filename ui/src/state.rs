use collects_business::{ApiStatus, AuthState, BusinessConfig};
use collects_states::{StateCtx, Time};
use serde::{Deserialize, Serialize};

use crate::widgets::LoginDialogState;

#[derive(Deserialize, Serialize)]
pub struct State {
    // We needs to store the presisent state
    #[serde(skip)]
    pub ctx: StateCtx,
    #[serde(skip)]
    pub auth_state: AuthState,
    #[serde(skip)]
    pub login_dialog_state: LoginDialogState,
}

impl Default for State {
    fn default() -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::default());
        ctx.record_compute(ApiStatus::default());

        Self {
            ctx,
            auth_state: AuthState::default(),
            login_dialog_state: LoginDialogState::default(),
        }
    }
}

impl State {
    pub fn test(base_url: String) -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::new(base_url));
        ctx.record_compute(ApiStatus::default());

        Self {
            ctx,
            auth_state: AuthState::default(),
            login_dialog_state: LoginDialogState::default(),
        }
    }
}
