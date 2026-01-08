#![feature(box_as_ptr)]

mod basic_state;
mod compute;
mod ctx;
mod dep;
mod enum_states;
mod graph;
mod runtime;
mod snapshot;
mod state;
mod state_sync_status;

pub use basic_state::Time;
pub use compute::{Compute, ComputeDeps, assign_impl};
pub use ctx::StateCtx;
pub use dep::Dep;
pub use enum_states::BasicStates;
pub use graph::{DepRoute, Graph, TopologyError};
pub use runtime::StateRuntime;
pub use snapshot::{CommandSnapshot, ComputeSnapshot, SnapshotClone, StateSnapshot};
pub use state::{Reader, State, Updater, state_assign_impl};
pub use state_sync_status::Stage;

/// Manual-only side effects / commands.
///
/// Commands intentionally **do not** participate in the compute dependency graph.
/// They must be invoked explicitly (e.g. from UI events, app init, or a scheduler).
///
/// Best practice:
/// - Keep `Compute` pure/derived.
/// - Put IO / async work / heavy CPU into `Command`.
///
/// ## Snapshot-based command reads + queued writes
///
/// Commands:
/// - **Read only from snapshots (owned clones)** of State/Compute values via `CommandSnapshot`.
/// - **Never borrow or mutate live state/compute references** during execution.
/// - **Write only via queued updates** (e.g. `Updater::set(...)` / `Updater::set_state(...)`).
///
/// Rationale:
/// - Supports concurrent async work safely (including `wasm32` where threads are limited).
/// - Prevents commands from observing or racing against mid-frame mutations.
///
/// ### Concurrent async and out-of-order completion
///
/// If commands can start multiple in-flight requests for the same compute type, `TypeId` alone
/// is not sufficient to identify a request. Use `(TypeId, generation)` where `generation: u64`
/// increments per compute type, and carry `generation` through async completion so stale results
/// can be ignored.
///
/// ### UI-affine state boundary
///
/// States containing non-Send UI types (e.g. `egui::TextureHandle`) must be mutated only on the
/// UI thread and must not be updated from async completion via `Updater::set_state()`.
/// Keep that state in UI code and update it via `StateCtx::update()` / `StateCtx::state_mut()`.
pub trait Command: std::fmt::Debug + Send + Sync + 'static {
    /// Runs the command with snapshot-based access to states and computes.
    ///
    /// Commands read from `CommandSnapshot` (owned clones) and write updates via `Updater`.
    /// This ensures commands never hold mutable references to live state during execution.
    fn run(&self, snap: CommandSnapshot, updater: Updater);
}

#[cfg(test)]
mod state_runtime_test {
    use std::{
        any::{Any, TypeId},
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
    };

    use crate::compute::ComputeDeps;

    use super::*;

    #[derive(Default, Debug, Clone)]
    struct DummyState {
        base_value: i32,
    }

    impl SnapshotClone for DummyState {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for DummyState {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
        }
    }

    #[derive(Default, Debug, Clone)]
    struct DummyComputeA {
        doubled: i32,
    }

    impl SnapshotClone for DummyComputeA {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for DummyComputeA {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
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

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
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

    #[derive(Default, Debug, Clone)]
    struct DummyComputeB {
        doubled: i32,
    }

    impl SnapshotClone for DummyComputeB {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for DummyComputeB {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
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

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
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
    #[derive(Default, Debug, Clone)]
    struct DummyComputeC {
        quadrupled: i32,
    }

    impl SnapshotClone for DummyComputeC {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for DummyComputeC {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
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

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
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

    #[allow(dead_code)]
    #[derive(Default, Debug, Clone)]
    struct SideEffectCountState {
        count: usize,
    }

    impl SnapshotClone for SideEffectCountState {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for SideEffectCountState {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
        }
    }

    #[derive(Debug)]
    struct IncrementCountCommand {
        shared: Arc<AtomicUsize>,
    }

    impl Command for IncrementCountCommand {
        fn run(&self, _snap: CommandSnapshot, _updater: Updater) {
            self.shared.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_command_dispatch_is_manual_only() {
        let shared = Arc::new(AtomicUsize::new(0));

        let mut ctx = StateCtx::new();
        ctx.add_state(SideEffectCountState::default());

        // Store the command in ctx, but it must NOT run during `run_all_dirty()`.
        ctx.record_command(IncrementCountCommand {
            shared: Arc::clone(&shared),
        });

        ctx.run_all_dirty();
        ctx.sync_computes();

        assert_eq!(shared.load(Ordering::SeqCst), 0);

        // Only runs when explicitly invoked via enqueue + flush.
        ctx.enqueue_command::<IncrementCountCommand>();
        ctx.flush_commands();

        assert_eq!(shared.load(Ordering::SeqCst), 1);
    }

    #[derive(Default, Debug, Clone)]
    struct DummyComputeFromCommand {
        value: i32,
    }

    impl SnapshotClone for DummyComputeFromCommand {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for DummyComputeFromCommand {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
        }
    }

    impl Compute for DummyComputeFromCommand {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn deps(&self) -> ComputeDeps {
            const STATE_IDS: [TypeId; 0] = [];
            const COMPUTE_IDS: [TypeId; 0] = [];
            (&STATE_IDS, &COMPUTE_IDS)
        }

        fn compute(&self, _dep: Dep, _updater: Updater) {
            // Intentionally no-op: this compute is updated by a Command via Updater.
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            assign_impl(self, new_self);
        }
    }

    #[derive(Debug)]
    struct SetComputeValueCommand {
        value: i32,
    }

    impl Command for SetComputeValueCommand {
        fn run(&self, _snap: CommandSnapshot, updater: Updater) {
            updater.set(DummyComputeFromCommand { value: self.value });
        }
    }

    #[test]
    fn test_command_can_update_compute_via_updater_and_sync() {
        let mut ctx = StateCtx::new();

        // Register the compute so it can receive updates via `Updater`.
        ctx.record_compute(DummyComputeFromCommand { value: 0 });

        // Register the command and execute it via enqueue + flush.
        ctx.record_command(SetComputeValueCommand { value: 123 });
        ctx.enqueue_command::<SetComputeValueCommand>();
        ctx.flush_commands();

        // Command updates are delivered via the same runtime channel as computes.
        ctx.sync_computes();

        assert_eq!(ctx.cached::<DummyComputeFromCommand>().unwrap().value, 123);
    }

    #[allow(dead_code)]
    #[derive(Debug, Clone)]
    struct UnregisteredCompute {
        value: i32,
    }

    impl SnapshotClone for UnregisteredCompute {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for UnregisteredCompute {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
        }
    }

    impl Compute for UnregisteredCompute {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn deps(&self) -> ComputeDeps {
            const STATE_IDS: [TypeId; 0] = [];
            const COMPUTE_IDS: [TypeId; 0] = [];
            (&STATE_IDS, &COMPUTE_IDS)
        }

        fn compute(&self, _dep: Dep, _updater: Updater) {
            // Intentionally no-op: this compute is only used to validate strict syncing.
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            assign_impl(self, new_self);
        }
    }

    #[derive(Debug)]
    struct SetUnregisteredComputeCommand {
        value: i32,
    }

    impl Command for SetUnregisteredComputeCommand {
        fn run(&self, _snap: CommandSnapshot, updater: Updater) {
            // Intentionally send an update for a compute type that was never registered.
            updater.set(UnregisteredCompute { value: self.value });
        }
    }

    #[test]
    #[should_panic]
    fn test_updater_set_on_unregistered_compute_panics_strictly() {
        let mut ctx = StateCtx::new();

        // Register the command (but NOT the compute type `UnregisteredCompute`).
        ctx.record_command(SetUnregisteredComputeCommand { value: 1 });

        // Enqueue + flush queues an update; syncing must panic strictly because the compute
        // receiving the update was never registered with `record_compute`.
        ctx.enqueue_command::<SetUnregisteredComputeCommand>();
        ctx.flush_commands();
        ctx.sync_computes();
    }

    /// A compute that tracks how many times it has been executed.
    #[derive(Debug, Clone)]
    struct ExecutionCountingCompute {
        value: i32,
        execution_count: Arc<AtomicUsize>,
    }

    impl SnapshotClone for ExecutionCountingCompute {
        fn clone_boxed(&self) -> Option<Box<dyn Any + Send>> {
            Some(Box::new(self.clone()))
        }
    }

    impl State for ExecutionCountingCompute {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            state_assign_impl(self, new_self);
        }
    }

    impl Compute for ExecutionCountingCompute {
        fn as_any(&self) -> &dyn Any {
            self
        }

        fn deps(&self) -> ComputeDeps {
            // Depends on DummyState (simulating ApiStatus depending on Time)
            const IDS: [TypeId; 1] = [TypeId::of::<DummyState>()];
            (&IDS, &[])
        }

        fn compute(&self, dep: Dep, updater: Updater) {
            // Track execution count
            self.execution_count.fetch_add(1, Ordering::SeqCst);

            let state = dep.get_state_ref::<DummyState>();
            updater.set(ExecutionCountingCompute {
                value: state.base_value * 10,
                execution_count: Arc::clone(&self.execution_count),
            });
        }

        fn assign_box(&mut self, new_self: Box<dyn Any + Send>) {
            assign_impl(self, new_self);
        }
    }

    /// Test that verifies compute execution behavior when state changes rapidly.
    ///
    /// This test simulates the scenario where Time state updates frequently (e.g., every second)
    /// and a compute (like ApiStatus) depends on it. The compute should:
    /// 1. Be marked dirty when dependency changes
    /// 2. Only execute once per `run_all_dirty()` call
    /// 3. Not spam logs at INFO level (tested by verifying execution count)
    #[test]
    fn test_compute_execution_count_with_rapid_state_changes() {
        let execution_count = Arc::new(AtomicUsize::new(0));

        let mut ctx = StateCtx::new();
        ctx.add_state(DummyState { base_value: 1 });
        ctx.record_compute(ExecutionCountingCompute {
            value: 0,
            execution_count: Arc::clone(&execution_count),
        });

        // Initial run - should execute once
        ctx.run_all_dirty();
        ctx.sync_computes();
        assert_eq!(execution_count.load(Ordering::SeqCst), 1);
        assert_eq!(ctx.cached::<ExecutionCountingCompute>().unwrap().value, 10);

        // Simulate rapid state changes (like Time updating every second)
        // Each update should mark the compute dirty
        for i in 2..=5 {
            ctx.update::<DummyState>(|state| {
                state.base_value = i;
            });
        }

        // Even though state changed 4 times, run_all_dirty() should only execute compute ONCE
        // because it processes all dirty computes in a single pass
        ctx.run_all_dirty();
        ctx.sync_computes();

        // Compute should have executed exactly 2 times total (1 initial + 1 after multiple updates)
        assert_eq!(execution_count.load(Ordering::SeqCst), 2);
        // Value should be based on the last state value (5 * 10 = 50)
        assert_eq!(ctx.cached::<ExecutionCountingCompute>().unwrap().value, 50);
    }

    /// Test that verifies command receives snapshot with correct state/compute values.
    ///
    /// This test ensures that:
    /// 1. Commands receive a CommandSnapshot instead of Dep
    /// 2. CommandSnapshot provides access to state via snap.state::<T>()
    /// 3. CommandSnapshot provides access to compute via snap.compute::<T>()
    /// 4. Commands can update compute values via Updater::set()
    #[derive(Debug)]
    struct SnapshotReadingCommand {
        expected_state_value: i32,
        expected_compute_value: i32,
        shared_success: Arc<AtomicUsize>,
    }

    impl Command for SnapshotReadingCommand {
        fn run(&self, snap: CommandSnapshot, updater: Updater) {
            // Read state from snapshot
            let state: &DummyState = snap.state();
            assert_eq!(state.base_value, self.expected_state_value);

            // Read compute from snapshot
            let compute: &DummyComputeA = snap.compute();
            assert_eq!(compute.doubled, self.expected_compute_value);

            // Signal success
            self.shared_success.fetch_add(1, Ordering::SeqCst);

            // Update another compute via updater
            updater.set(DummyComputeFromCommand {
                value: state.base_value * 100,
            });
        }
    }

    #[test]
    fn test_command_reads_from_snapshot() {
        let success = Arc::new(AtomicUsize::new(0));

        let mut ctx = StateCtx::new();

        // Add state with initial value 5
        ctx.add_state(DummyState { base_value: 5 });

        // Add compute (will compute doubled = 10)
        ctx.record_compute(DummyComputeA { doubled: 0 });

        // Add target compute for command to update
        ctx.record_compute(DummyComputeFromCommand { value: 0 });

        // Run initial compute
        ctx.run_all_dirty();
        ctx.sync_computes();

        // Verify compute ran correctly
        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 10);

        // Register command expecting state.base_value=5, compute.doubled=10
        ctx.record_command(SnapshotReadingCommand {
            expected_state_value: 5,
            expected_compute_value: 10,
            shared_success: Arc::clone(&success),
        });

        // Execute command via enqueue + flush
        ctx.enqueue_command::<SnapshotReadingCommand>();
        ctx.flush_commands();
        ctx.sync_computes();

        // Verify command ran successfully and assertions passed
        assert_eq!(success.load(Ordering::SeqCst), 1);

        // Verify command updated the compute via updater
        assert_eq!(ctx.cached::<DummyComputeFromCommand>().unwrap().value, 500);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // COMMAND QUEUE TESTS
    // ═══════════════════════════════════════════════════════════════════════

    /// Tests that enqueue_command adds commands to the queue without executing them.
    #[test]
    fn test_enqueue_command_does_not_execute_immediately() {
        let shared = Arc::new(AtomicUsize::new(0));

        let mut ctx = StateCtx::new();
        ctx.record_command(IncrementCountCommand {
            shared: Arc::clone(&shared),
        });

        // Enqueue the command
        ctx.enqueue_command::<IncrementCountCommand>();

        // Command should NOT have executed yet
        assert_eq!(shared.load(Ordering::SeqCst), 0);
        assert_eq!(ctx.command_queue_len(), 1);
    }

    /// Tests that flush_commands executes all queued commands.
    #[test]
    fn test_flush_commands_executes_queued_commands() {
        let shared = Arc::new(AtomicUsize::new(0));

        let mut ctx = StateCtx::new();
        ctx.record_command(IncrementCountCommand {
            shared: Arc::clone(&shared),
        });

        // Enqueue multiple instances of the same command
        ctx.enqueue_command::<IncrementCountCommand>();
        ctx.enqueue_command::<IncrementCountCommand>();
        ctx.enqueue_command::<IncrementCountCommand>();

        assert_eq!(ctx.command_queue_len(), 3);

        // Flush should execute all commands
        ctx.flush_commands();

        assert_eq!(shared.load(Ordering::SeqCst), 3);
        assert_eq!(ctx.command_queue_len(), 0);
    }

    /// Tests that flush_commands with empty queue does nothing.
    #[test]
    fn test_flush_commands_empty_queue() {
        let mut ctx = StateCtx::new();

        // Flushing empty queue should not panic
        ctx.flush_commands();

        assert_eq!(ctx.command_queue_len(), 0);
    }

    /// Tests that commands enqueued during flush are not executed until next flush.
    #[derive(Debug)]
    struct EnqueueAnotherCommand {
        counter: Arc<AtomicUsize>,
    }

    impl Command for EnqueueAnotherCommand {
        fn run(&self, _snap: CommandSnapshot, _updater: Updater) {
            self.counter.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn test_flush_commands_order() {
        let counter1 = Arc::new(AtomicUsize::new(0));
        let counter2 = Arc::new(AtomicUsize::new(0));

        let mut ctx = StateCtx::new();

        ctx.record_command(IncrementCountCommand {
            shared: Arc::clone(&counter1),
        });
        ctx.record_command(EnqueueAnotherCommand {
            counter: Arc::clone(&counter2),
        });

        // Enqueue in specific order
        ctx.enqueue_command::<IncrementCountCommand>();
        ctx.enqueue_command::<EnqueueAnotherCommand>();
        ctx.enqueue_command::<IncrementCountCommand>();

        ctx.flush_commands();

        // All three should have executed
        assert_eq!(counter1.load(Ordering::SeqCst), 2);
        assert_eq!(counter2.load(Ordering::SeqCst), 1);
    }

    /// Tests that enqueue_command panics for unregistered commands.
    #[test]
    #[should_panic(expected = "No command found")]
    fn test_enqueue_unregistered_command_panics() {
        let mut ctx = StateCtx::new();
        // IncrementCountCommand was not registered
        ctx.enqueue_command::<IncrementCountCommand>();
    }

    /// Tests that command queue works with snapshot reading.
    #[test]
    fn test_enqueue_command_reads_from_snapshot() {
        let success = Arc::new(AtomicUsize::new(0));

        let mut ctx = StateCtx::new();

        // Add state with initial value 5
        ctx.add_state(DummyState { base_value: 5 });

        // Add compute (will compute doubled = 10)
        ctx.record_compute(DummyComputeA { doubled: 0 });

        // Add target compute for command to update
        ctx.record_compute(DummyComputeFromCommand { value: 0 });

        // Run initial compute
        ctx.run_all_dirty();
        ctx.sync_computes();

        // Verify compute ran correctly
        assert_eq!(ctx.cached::<DummyComputeA>().unwrap().doubled, 10);

        // Register command expecting state.base_value=5, compute.doubled=10
        ctx.record_command(SnapshotReadingCommand {
            expected_state_value: 5,
            expected_compute_value: 10,
            shared_success: Arc::clone(&success),
        });

        // Enqueue and flush command
        ctx.enqueue_command::<SnapshotReadingCommand>();
        ctx.flush_commands();
        ctx.sync_computes();

        // Verify command ran successfully and assertions passed
        assert_eq!(success.load(Ordering::SeqCst), 1);

        // Verify command updated the compute via updater
        assert_eq!(ctx.cached::<DummyComputeFromCommand>().unwrap().value, 500);
    }

    /// Tests the recommended end-of-frame pattern.
    #[test]
    fn test_end_of_frame_pattern() {
        let mut ctx = StateCtx::new();
        ctx.add_state(DummyState { base_value: 1 });
        ctx.record_compute(DummyComputeFromCommand { value: 0 });
        ctx.record_command(SetComputeValueCommand { value: 42 });

        // Simulate frame loop
        // 1. Sync any async results
        ctx.sync_computes();

        // 2. UI interaction enqueues command
        ctx.enqueue_command::<SetComputeValueCommand>();

        // 3. End of frame: flush commands
        ctx.flush_commands();

        // 4. Sync command results
        ctx.sync_computes();

        // 5. Verify state was updated
        assert_eq!(ctx.cached::<DummyComputeFromCommand>().unwrap().value, 42);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // ASYNC DEPENDENCY VERIFICATION TESTS
    // ═══════════════════════════════════════════════════════════════════════

    /// Verifies that tokio, tokio_util, and async-trait dependencies are
    /// correctly integrated and functional for future async migration.
    ///
    /// This test confirms:
    /// 1. CancellationToken from tokio_util::sync works correctly
    /// 2. tokio runtime can be created and used
    /// 3. async-trait macro is available for future async trait definitions
    mod async_deps_tests {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        use tokio_util::sync::CancellationToken;

        /// Tests that CancellationToken can be created and cancelled.
        #[test]
        fn test_cancellation_token_basic() {
            let token = CancellationToken::new();
            assert!(!token.is_cancelled());

            token.cancel();
            assert!(token.is_cancelled());
        }

        /// Tests that child tokens are cancelled when parent is cancelled.
        #[test]
        fn test_cancellation_token_hierarchy() {
            let parent = CancellationToken::new();
            let child = parent.child_token();

            assert!(!parent.is_cancelled());
            assert!(!child.is_cancelled());

            // Cancelling parent should cancel child
            parent.cancel();

            assert!(parent.is_cancelled());
            assert!(child.is_cancelled());
        }

        /// Tests that cancelling child does not cancel parent.
        #[test]
        fn test_cancellation_token_child_independence() {
            let parent = CancellationToken::new();
            let child = parent.child_token();

            // Cancelling child should NOT cancel parent
            child.cancel();

            assert!(!parent.is_cancelled());
            assert!(child.is_cancelled());
        }

        /// Tests async cancellation with tokio runtime.
        #[tokio::test]
        async fn test_cancellation_token_async() {
            let token = CancellationToken::new();
            let token_clone = token.clone();
            let completed = Arc::new(AtomicBool::new(false));
            let completed_clone = Arc::clone(&completed);

            // Spawn a task that waits for cancellation
            let handle = tokio::spawn(async move {
                token_clone.cancelled().await;
                // Task was cancelled
                completed_clone.store(true, Ordering::SeqCst);
            });

            // Cancel the token
            token.cancel();

            // Wait for the task to complete
            handle.await.expect("task should complete");

            assert!(completed.load(Ordering::SeqCst));
        }

        /// Tests that async-trait macro is available and functional.
        #[async_trait::async_trait]
        trait AsyncTestTrait: Send + Sync {
            async fn do_work(&self) -> i32;
        }

        struct TestImpl {
            value: i32,
        }

        #[async_trait::async_trait]
        impl AsyncTestTrait for TestImpl {
            async fn do_work(&self) -> i32 {
                self.value * 2
            }
        }

        #[tokio::test]
        async fn test_async_trait_works() {
            let impl_obj = TestImpl { value: 21 };
            let result = impl_obj.do_work().await;
            assert_eq!(result, 42);
        }

        /// Tests that boxed async trait objects work correctly.
        #[tokio::test]
        async fn test_async_trait_boxed() {
            let impl_obj: Box<dyn AsyncTestTrait> = Box::new(TestImpl { value: 10 });
            let result = impl_obj.do_work().await;
            assert_eq!(result, 20);
        }
    }
}
