#![feature(box_as_ptr)]

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
pub use compute::{Compute, ComputeDeps, ComputeStage, assign_impl};
pub use ctx::StateCtx;
pub use dep::Dep;
pub use enum_states::BasicStates;
pub use graph::{DepRoute, Graph, TopologyError};
pub use runtime::StateRuntime;
pub use state::{Reader, State, Updater};
pub use state_sync_status::Stage;

#[cfg(test)]
mod state_runtime_test {
    use std::any::{Any, TypeId};

    use crate::compute::ComputeDeps;

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
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn deps(&self) -> ComputeDeps {
            const IDS: [TypeId; 1] = [TypeId::of::<DummyState>()];
            (&IDS, &[])
        }

        fn compute(&self, dep: Dep, updater: Updater) -> ComputeStage {
            let based = dep.get_state_ref::<DummyState>();
            updater.set(DummyComputeA {
                doubled: based.base_value * 2,
            });
            ComputeStage::Pending
        }

        fn assign_box(&mut self, new_self: Box<dyn Any>) {
            assign_impl(self, new_self);
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

    #[derive(Default, Debug)]
    struct DummyComputeB {
        doubled: i32,
    }

    impl State for DummyComputeB {}

    impl Compute for DummyComputeB {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn deps(&self) -> ComputeDeps {
            const IDS: [TypeId; 1] = [TypeId::of::<DummyState>()];
            (&IDS, &[])
        }

        fn compute(&self, dep: Dep, updater: Updater) -> ComputeStage {
            let based = dep.get_state_ref::<DummyState>();
            if based.base_value > 0 {
                updater.set(DummyComputeB {
                    doubled: based.base_value * 2,
                });
                return ComputeStage::Pending;
            }
            ComputeStage::Finished
        }

        fn assign_box(&mut self, new_self: Box<dyn Any>) {
            assign_impl(self, new_self);
        }
    }

    #[test]
    fn state_runtime_pending() {
        let mut ctx = StateCtx::new();

        ctx.add_state(DummyState { base_value: 1 });
        ctx.record_compute(DummyComputeB { doubled: 0 });

        ctx.run_computed();
        ctx.sync_computes();

        assert_eq!(ctx.cached::<DummyComputeB>().unwrap().doubled, 2);

        *ctx.states.get_mut(&TypeId::of::<DummyState>()).unwrap().0.get_mut() = Box::new(DummyState { base_value: -1 });
        ctx.mark_dirty(&TypeId::of::<DummyState>());
        ctx.mark_dirty(&TypeId::of::<DummyComputeB>());
        ctx.run_computed();
        ctx.sync_computes();
        assert_eq!(ctx.cached::<DummyComputeB>().unwrap().doubled, 2);
    }
}
