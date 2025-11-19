mod basic_states;
mod compute;
mod ctx;
mod graph;
mod register_state;
mod runtime;
mod state;
mod state_sync_status;

pub use basic_states::BasicStates;
pub use compute::Compute;
pub use ctx::StateCtx;
pub use graph::{DepRoute, Graph, TopologyError};
pub use register_state::register_state;
pub use runtime::StateRuntime;
pub use state::{State, StateReader, StateUpdater};
pub use state_sync_status::StateSyncStatus;

#[cfg(test)]
mod state_runtime_test {
    use super::*;

    #[test]
    fn simple_state() {
        let mut runtime = StateRuntime::new();
        let sender = runtime.sender();
        let receiver = runtime.receiver();

        struct TestState {
            value: i32,
        }

        impl Default for TestState {
            fn default() -> Self {
                Self { value: 0 }
            }
        }

        impl State for TestState {
            const TYPE: &'static str = "test_state";
            const ID: Reg = Reg::BasicStates; // Just an example, use appropriate Reg
        }

        let test_state = TestState { value: 42 };
        updater.set(test_state);

        let boxed = receiver.recv().unwrap();
        let state = boxed.downcast::<TestState>().unwrap();
        assert_eq!(state.value, 42);
    }
}
