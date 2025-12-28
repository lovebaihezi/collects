use collects_business::ApiStatus;
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{CreateUserCompute, CreateUserInput, InternalApiStatus};
use collects_business::BusinessConfig;
use collects_states::{StateCtx, Time};
use serde::{Deserialize, Serialize};

#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use crate::widgets::InternalUsersState;

#[derive(Deserialize, Serialize)]
pub struct State {
    // We need to store the persistent state
    #[serde(skip)]
    pub ctx: StateCtx,
    /// Internal users state (only for internal builds)
    #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
    #[serde(skip)]
    pub internal_users: InternalUsersState,
}

impl Default for State {
    fn default() -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::default());
        ctx.record_compute(ApiStatus::default());

        // Add internal states and computes for internal builds
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        {
            ctx.add_state(CreateUserInput::default());
            ctx.record_compute(InternalApiStatus::default());
            ctx.record_compute(CreateUserCompute::default());
        }

        Self {
            ctx,
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
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

        // Add internal states and computes for internal builds
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        {
            ctx.add_state(CreateUserInput::default());
            ctx.record_compute(InternalApiStatus::default());
            ctx.record_compute(CreateUserCompute::default());
        }

        Self {
            ctx,
            #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
            internal_users: InternalUsersState::new(),
        }
    }
}
