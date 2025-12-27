use collects_business::{ApiStatus, BusinessConfig};
use collects_states::{StateCtx, Time};
use serde::{Deserialize, Serialize};

use crate::widgets::InternalUsersState;

#[derive(Deserialize, Serialize)]
pub struct State {
    // We needs to store the presisent state
    #[serde(skip)]
    pub ctx: StateCtx,
    /// Internal users panel state (only used in test/internal builds).
    #[serde(skip)]
    pub internal_users: InternalUsersState,
}

impl Default for State {
    fn default() -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::default());
        ctx.record_compute(ApiStatus::default());

        Self {
            ctx,
            internal_users: InternalUsersState::new(),
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
            internal_users: InternalUsersState::new(),
        }
    }
}
