mod basic_state;
mod compute;
mod ctx;
mod dep;
mod enum_states;
mod graph;
mod runtime;
mod state;
mod state_sync_status;

pub use basic_state::Time;
pub use compute::Compute;
pub use ctx::StateCtx;
pub use dep::Dep;
pub use enum_states::BasicStates;
pub use graph::{DepRoute, Graph, TopologyError};
pub use runtime::StateRuntime;
pub use state::{State, StateReader, StateUpdater};
pub use state_sync_status::StateSyncStatus;

#[cfg(test)]
mod state_runtime_test {
    use std::any::TypeId;

    use super::*;

    #[derive(Default, Debug)]
    struct DummyState {
        base_value: i32,
    }

    impl State for DummyState {}

    #[derive(Default, Debug)]
    struct DummyComputeA {
        doubled: i32,
    }

    impl State for DummyComputeA {}

    impl Compute for DummyComputeA {
        fn deps(&self) -> &'static [TypeId] {
            const IDS: [TypeId; 1] = [TypeId::of::<DummyState>()];
            &IDS
        }

        fn compute(&self, dep: Dep, updater: StateUpdater) {
            let based = dep.get_ref::<DummyState>();
            updater.set(DummyComputeA {
                doubled: based.base_value * 2,
            });
        }
    }

    #[test]
    fn state_runtime_basic() {
        let mut ctx = StateCtx::new();
        // Register the states and computes, which, the state manually init
        ctx.add_state(DummyState { base_value: 1 });
        ctx.add_state(Time::default());
        ctx.record_compute(DummyComputeA { doubled: 0 });

        ctx.run_computed();
        ctx.sync_computes();

        // Render the states, which, we here verify the states are correctly updated
        assert!(ctx.cached::<DummyComputeA>().is_some());
        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 2);
    }
}
