mod basic_state;
mod compute;
mod ctx;
mod dep;
mod enum_states;
mod graph;
mod register_state;
mod runtime;
mod state;
mod state_sync_status;

pub use basic_state::Time;
pub use compute::Compute;
pub use ctx::StateCtx;
pub use dep::Dep;
pub use enum_states::BasicStates;
pub use graph::{DepRoute, Graph, TopologyError};
pub use register_state::Reg;
pub use runtime::StateRuntime;
pub use state::{State, StateReader, StateUpdater};
pub use state_sync_status::StateSyncStatus;

#[cfg(test)]
mod state_runtime_test {
    use super::*;

    #[derive(Default, Debug)]
    struct DummyState {
        base_value: i32,
    }

    impl State for DummyState {
        fn id(&self) -> Reg {
            Reg::TestStateA
        }
    }

    #[derive(Default, Debug)]
    struct DummyComputeA {
        doubled: i32,
    }

    impl State for DummyComputeA {
        fn id(&self) -> Reg {
            Reg::TestComputeA
        }
    }

    impl Compute for DummyComputeA {
        fn deps(&self) -> &'static [Reg] {
            &[Reg::TestStateA, Reg::Time]
        }

        fn compute(&self, dep: Dep, updater: StateUpdater) {
            let based = dep.get_ref::<DummyState>(Reg::TestStateA);
            updater.set(DummyComputeA {
                doubled: based.base_value * 2,
            });
        }
    }

    #[test]
    fn state_runtime_baisic() {
        let mut ctx = StateCtx::new();
        // Register the states and computes, which, the state manually init
        ctx.add_state(DummyState { base_value: 1 });
        ctx.add_state(Time::default());
        ctx.record_compute(DummyComputeA { doubled: 0 });

        // run init compute
        ctx.run_computed();

        // Render the states, which, we here verify the states are correctly updated
    }
}
