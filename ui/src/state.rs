use collects_business::{ApiStatus, BusinessConfig};
#[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
use collects_business::{InternalApiStatus, InternalUsers};
use collects_states::{StateCtx, Time};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct State {
    // We needs to store the presisent state
    #[serde(skip)]
    pub ctx: StateCtx,
}

impl Default for State {
    fn default() -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::default());
        ctx.record_compute(ApiStatus::default());

        // Register internal states for internal builds
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        {
            ctx.record_compute(InternalApiStatus::default());
            ctx.record_compute(InternalUsers::default());
        }

        Self { ctx }
    }
}

impl State {
    pub fn test(base_url: String) -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::new(base_url));
        ctx.record_compute(ApiStatus::default());

        // Register internal states for internal builds
        #[cfg(any(feature = "env_internal", feature = "env_test_internal"))]
        {
            ctx.record_compute(InternalApiStatus::default());
            ctx.record_compute(InternalUsers::default());
        }

        Self { ctx }
    }
}
