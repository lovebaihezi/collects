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
pub use compute::{Compute, ComputeDeps, assign_impl};
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

    impl State for DummyState {
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    #[derive(Default, Debug)]
    struct DummyComputeA {
        doubled: i32,
    }

    impl State for DummyComputeA {
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl Compute for DummyComputeA {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn deps(&self) -> ComputeDeps {
            const IDS: [TypeId; 1] = [TypeId::of::<DummyState>()];
            (&IDS, &[])
        }

        fn compute(&self, dep: Dep, updater: Updater) {
            let based = dep.get_state_ref::<DummyState>();
            updater.set(DummyComputeA {
                doubled: based.base_value * 2,
            });
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

        ctx.run_all_dirty();
        ctx.sync_computes();

        // Render the states, which, we here verify the states are correctly updated
        assert!(ctx.cached::<DummyComputeA>().is_some());
        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 2);
    }

    #[derive(Default, Debug)]
    struct DummyComputeB {
        doubled: i32,
    }

    impl State for DummyComputeB {
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl Compute for DummyComputeB {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn deps(&self) -> ComputeDeps {
            const IDS: [TypeId; 1] = [TypeId::of::<DummyState>()];
            (&IDS, &[])
        }

        fn compute(&self, dep: Dep, updater: Updater) {
            let based = dep.get_state_ref::<DummyState>();
            if based.base_value > 0 {
                updater.set(DummyComputeB {
                    doubled: based.base_value * 2,
                });
            }
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

        ctx.run_all_dirty();
        ctx.sync_computes();

        assert_eq!(ctx.cached::<DummyComputeB>().unwrap().doubled, 2);

        // Use update() which auto-propagates dirty to dependents
        ctx.update::<DummyState>(|state| {
            state.base_value = -1;
        });
        // DummyComputeB should now be automatically marked dirty
        ctx.run_all_dirty();
        ctx.sync_computes();
        // Since base_value is negative, compute doesn't update (keeps old value)
        assert_eq!(ctx.cached::<DummyComputeB>().unwrap().doubled, 2);
    }

    #[test]
    fn test_auto_dirty_propagation() {
        let mut ctx = StateCtx::new();

        ctx.add_state(DummyState { base_value: 1 });
        ctx.record_compute(DummyComputeA { doubled: 0 });

        // Initial run
        ctx.run_all_dirty();
        ctx.sync_computes();
        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 2);

        // Update state using update() - should auto-mark DummyComputeA as dirty
        ctx.update::<DummyState>(|state| {
            state.base_value = 5;
        });

        // Run all dirty computes
        ctx.run_all_dirty();
        ctx.sync_computes();

        // Verify compute was re-run with new value
        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 10);
    }

    #[test]
    fn test_run_specific_compute() {
        let mut ctx = StateCtx::new();

        ctx.add_state(DummyState { base_value: 3 });
        ctx.record_compute(DummyComputeA { doubled: 0 });

        // Use run::<T>() to run specific compute
        ctx.run::<DummyComputeA>();
        ctx.sync_computes();

        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 6);

        // Update and run specific compute again
        ctx.update::<DummyState>(|state| {
            state.base_value = 7;
        });
        ctx.run::<DummyComputeA>();
        ctx.sync_computes();

        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 14);
    }

    // Test for compute depending on another compute
    #[derive(Default, Debug)]
    struct DummyComputeC {
        quadrupled: i32,
    }

    impl State for DummyComputeC {
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl Compute for DummyComputeC {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn deps(&self) -> ComputeDeps {
            // Depends on DummyComputeA (which depends on DummyState)
            const STATE_IDS: [TypeId; 0] = [];
            const COMPUTE_IDS: [TypeId; 1] = [TypeId::of::<DummyComputeA>()];
            (&STATE_IDS, &COMPUTE_IDS)
        }

        fn compute(&self, dep: Dep, updater: Updater) {
            let compute_a = dep.get_compute_ref::<DummyComputeA>();
            updater.set(DummyComputeC {
                quadrupled: compute_a.doubled * 2,
            });
        }

        fn assign_box(&mut self, new_self: Box<dyn Any>) {
            assign_impl(self, new_self);
        }
    }

    #[test]
    fn test_run_with_dependencies() {
        let mut ctx = StateCtx::new();

        ctx.add_state(DummyState { base_value: 2 });
        ctx.record_compute(DummyComputeA { doubled: 0 });
        ctx.record_compute(DummyComputeC { quadrupled: 0 });

        // Run ComputeC - should automatically run ComputeA first (dependency)
        ctx.run::<DummyComputeC>();
        ctx.sync_computes();

        // ComputeA should have run: 2 * 2 = 4
        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 4);
        // ComputeC should have run: 4 * 2 = 8
        assert_eq!(ctx.cached::<DummyComputeC>().unwrap().quadrupled, 8);

        // Update state and run ComputeC again
        ctx.update::<DummyState>(|state| {
            state.base_value = 5;
        });
        ctx.run::<DummyComputeC>();
        ctx.sync_computes();

        // ComputeA: 5 * 2 = 10
        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 10);
        // ComputeC: 10 * 2 = 20
        assert_eq!(ctx.cached::<DummyComputeC>().unwrap().quadrupled, 20);
    }
}
