mod basic_state;
mod compute;
mod ctx;
mod enum_states;
mod graph;
mod register_state;
mod runtime;
mod state;
mod state_sync_status;

pub use basic_state::Time;
pub use compute::Compute;
pub use ctx::StateCtx;
pub use enum_states::BasicStates;
pub use graph::{DepRoute, Graph, TopologyError};
pub use register_state::Reg;
pub use runtime::StateRuntime;
pub use state::{State, StateReader, StateUpdater};
pub use state_sync_status::StateSyncStatus;

#[cfg(test)]
mod state_runtime_test {
    use super::*;

    #[derive(Default)]
    struct DummyState;

    impl State for DummyState {
        const ID: Reg = Reg::TestStateA;
    }

    #[derive(Default)]
    struct DummyComputeA;

    impl State for DummyComputeA {
        const ID: Reg = Reg::TestComputeA;
    }

    impl Compute for DummyComputeA {
        const DEPS: &'static [Reg] = &[Reg::TestStateA, Reg::Time];

        fn compute(&self, _ctx: &StateCtx) -> Option<Self> {
            Some(DummyComputeA)
        }
    }

    #[test]
    fn state_runtime_baisic() {
        let mut ctx = StateCtx::new();
        // Register the states and computes, which, the state manually init
        ctx.add_state(DummyState);
        ctx.add_state(Time::default());
        ctx.record_compute(DummyComputeA);

        // run init compute
        todo!();

        // Render the states, which, we here verify the states are correctly updated
    }
}
