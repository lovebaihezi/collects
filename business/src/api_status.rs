use collects_states::{State, StateID};

pub struct APIStatus;

impl State for APIStatus {
    const ID: StateID = StateID::ApiStatus;
    const DEPS: &'static [StateID] = &[];

    fn compute(&mut self, _ctx: &collects_states::StateCtx) {}
}
