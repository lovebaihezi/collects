use collects_business::{ApiStatus, BusinessConfig};
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

        Self { ctx }
    }
}

impl State {
    pub fn test(base_url: String) -> Self {
        let mut ctx = StateCtx::new();

        ctx.add_state(Time::default());
        ctx.add_state(BusinessConfig::new(base_url));
        ctx.record_compute(ApiStatus::default());

        Self { ctx }
    }
}
