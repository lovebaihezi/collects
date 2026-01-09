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
mod task;

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
pub use task::{TaskHandle, TaskId, TaskIdGenerator};

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
/// ### Cooperative cancellation
///
/// Commands receive a `CancellationToken` to support cooperative cancellation. Long-running
/// commands should periodically check `cancel.is_cancelled()` or use `tokio::select!` with
/// `cancel.cancelled()` to respond to cancellation requests gracefully.
///
/// ### UI-affine state boundary
///
/// States containing non-Send UI types (e.g. `egui::TextureHandle`) must be mutated only on the
/// UI thread and must not be updated from async completion via `Updater::set_state()`.
/// Keep that state in UI code and update it via `StateCtx::update()` / `StateCtx::state_mut()`.
pub trait Command: std::fmt::Debug + Send + Sync + 'static {
    /// Runs the command asynchronously with snapshot-based access to states and computes.
    ///
    /// Commands read from `CommandSnapshot` (owned clones) and write updates via `Updater`.
    /// This ensures commands never hold mutable references to live state during execution.
    ///
    /// The `cancel` token enables cooperative cancellation. Commands should check
    /// `cancel.is_cancelled()` or await `cancel.cancelled()` to respond to cancellation.
    ///
    /// Returns a boxed future that must be `'static` (no references to `self`). Clone any
    /// needed data from `self` into the async block.
    ///
    /// # Example
    ///
    /// ```ignore
    /// impl Command for MyCommand {
    ///     fn run(
    ///         &self,
    ///         snap: CommandSnapshot,
    ///         updater: Updater,
    ///         cancel: CancellationToken,
    ///     ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
    ///         // Clone any data from self that you need in the async block
    ///         let my_data = self.data.clone();
    ///         Box::pin(async move {
    ///             tokio::select! {
    ///                 _ = cancel.cancelled() => {
    ///                     // Cancelled
    ///                 }
    ///                 result = do_async_work(my_data) => {
    ///                     updater.set(MyCompute { data: result });
    ///                 }
    ///             }
    ///         })
    ///     }
    /// }
    /// ```
    fn run(
        &self,
        snap: CommandSnapshot,
        updater: Updater,
        cancel: tokio_util::sync::CancellationToken,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>;
}

#[cfg(test)]
mod state_runtime_test {
    use std::{
        any::{Any, TypeId},
        sync::{
            Arc,
            atomic::{AtomicBool, AtomicUsize, Ordering},
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
        fn run(
            &self,
            _snap: CommandSnapshot,
            _updater: Updater,
            _cancel: tokio_util::sync::CancellationToken,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
            let shared = Arc::clone(&self.shared);
            Box::pin(async move {
                shared.fetch_add(1, Ordering::SeqCst);
            })
        }
    }

    #[tokio::test]
    async fn test_command_dispatch_is_manual_only() {
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

        // Yield to let the spawned task complete
        tokio::task::yield_now().await;

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
        fn run(
            &self,
            _snap: CommandSnapshot,
            updater: Updater,
            _cancel: tokio_util::sync::CancellationToken,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
            let value = self.value;
            Box::pin(async move {
                updater.set(DummyComputeFromCommand { value });
            })
        }
    }

    #[tokio::test]
    async fn test_command_can_update_compute_via_updater_and_sync() {
        let mut ctx = StateCtx::new();

        // Register the compute so it can receive updates via `Updater`.
        ctx.record_compute(DummyComputeFromCommand { value: 0 });

        // Register the command and execute it via enqueue + flush.
        ctx.record_command(SetComputeValueCommand { value: 123 });
        ctx.enqueue_command::<SetComputeValueCommand>();
        ctx.flush_commands();

        // Yield to let the spawned task complete
        tokio::task::yield_now().await;

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
        fn run(
            &self,
            _snap: CommandSnapshot,
            updater: Updater,
            _cancel: tokio_util::sync::CancellationToken,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
            let value = self.value;
            Box::pin(async move {
                // Intentionally send an update for a compute type that was never registered.
                updater.set(UnregisteredCompute { value });
            })
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
        fn run(
            &self,
            snap: CommandSnapshot,
            updater: Updater,
            _cancel: tokio_util::sync::CancellationToken,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
            let expected_state_value = self.expected_state_value;
            let expected_compute_value = self.expected_compute_value;
            let shared_success = Arc::clone(&self.shared_success);
            Box::pin(async move {
                // Read state from snapshot
                let state: &DummyState = snap.state();
                assert_eq!(state.base_value, expected_state_value);

                // Read compute from snapshot
                let compute: &DummyComputeA = snap.compute();
                assert_eq!(compute.doubled, expected_compute_value);

                // Signal success
                shared_success.fetch_add(1, Ordering::SeqCst);

                // Update another compute via updater
                updater.set(DummyComputeFromCommand {
                    value: state.base_value * 100,
                });
            })
        }
    }

    #[tokio::test]
    async fn test_command_reads_from_snapshot() {
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

        // Yield to let the spawned task complete
        tokio::task::yield_now().await;

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
    #[tokio::test]
    async fn test_flush_commands_executes_queued_commands() {
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

        // Yield to let the spawned tasks complete
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

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
        fn run(
            &self,
            _snap: CommandSnapshot,
            _updater: Updater,
            _cancel: tokio_util::sync::CancellationToken,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
            let counter = Arc::clone(&self.counter);
            Box::pin(async move {
                counter.fetch_add(1, Ordering::SeqCst);
            })
        }
    }

    #[tokio::test]
    async fn test_flush_commands_order() {
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

        // Yield to let the spawned tasks complete
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

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
    #[tokio::test]
    async fn test_enqueue_command_reads_from_snapshot() {
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

        // Yield to let the spawned task complete
        tokio::task::yield_now().await;

        ctx.sync_computes();

        // Verify command ran successfully and assertions passed
        assert_eq!(success.load(Ordering::SeqCst), 1);

        // Verify command updated the compute via updater
        assert_eq!(ctx.cached::<DummyComputeFromCommand>().unwrap().value, 500);
    }

    /// Tests the recommended end-of-frame pattern.
    #[tokio::test]
    async fn test_end_of_frame_pattern() {
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

        // Yield to let the spawned task complete
        tokio::task::yield_now().await;

        // 4. Sync command results
        ctx.sync_computes();

        // 5. Verify state was updated
        assert_eq!(ctx.cached::<DummyComputeFromCommand>().unwrap().value, 42);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // ASYNC COMMAND TRAIT TESTS
    // ═══════════════════════════════════════════════════════════════════════

    /// Tests that async Command receives a CancellationToken.
    /// This verifies that the new Command trait signature with CancellationToken works correctly.
    #[derive(Debug)]
    struct CancellationAwareCommand {
        cancel_received: Arc<AtomicUsize>,
    }

    impl Command for CancellationAwareCommand {
        fn run(
            &self,
            _snap: CommandSnapshot,
            _updater: Updater,
            cancel: tokio_util::sync::CancellationToken,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> {
            let cancel_received = Arc::clone(&self.cancel_received);
            Box::pin(async move {
                // Verify that the cancel token is valid and not initially cancelled
                if !cancel.is_cancelled() {
                    cancel_received.fetch_add(1, Ordering::SeqCst);
                }
            })
        }
    }

    #[tokio::test]
    async fn test_async_command_receives_cancellation_token() {
        let cancel_received = Arc::new(AtomicUsize::new(0));

        let mut ctx = StateCtx::new();
        ctx.record_command(CancellationAwareCommand {
            cancel_received: Arc::clone(&cancel_received),
        });

        // Execute command
        ctx.enqueue_command::<CancellationAwareCommand>();
        ctx.flush_commands();

        // Yield to let the spawned task complete
        tokio::task::yield_now().await;

        // Verify the command received a valid (non-cancelled) token
        assert_eq!(cancel_received.load(Ordering::SeqCst), 1);
    }

    // ═══════════════════════════════════════════════════════════════════════
    // TASK MANAGEMENT TESTS
    // ═══════════════════════════════════════════════════════════════════════

    /// Tests that StateCtx initializes with empty task management state.
    #[test]
    fn test_task_management_initial_state() {
        let ctx = StateCtx::new();

        assert_eq!(ctx.task_count(), 0);
        assert_eq!(ctx.active_task_type_count(), 0);
        assert!(ctx.active_tasks().is_empty());
    }

    /// Tests registering a task handle for a compute type.
    #[test]
    fn test_register_task_handle() {
        use tokio_util::sync::CancellationToken;

        let mut ctx = StateCtx::new();
        let task_id = ctx.task_id_generator().next::<DummyState>();
        let token = CancellationToken::new();
        let handle = TaskHandle::new(task_id, token);

        assert!(!ctx.has_active_task::<DummyState>());

        ctx.register_task_handle::<DummyState>(handle);

        assert!(ctx.has_active_task::<DummyState>());
        assert_eq!(ctx.active_task_type_count(), 1);
    }

    /// Tests getting an active task handle.
    #[test]
    fn test_get_active_task() {
        use tokio_util::sync::CancellationToken;

        let mut ctx = StateCtx::new();
        let task_id = ctx.task_id_generator().next::<DummyState>();
        let token = CancellationToken::new();
        let handle = TaskHandle::new(task_id, token);

        ctx.register_task_handle::<DummyState>(handle);

        let retrieved = ctx.get_active_task::<DummyState>();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id(), task_id);

        // Non-existent type should return None
        assert!(ctx.get_active_task::<DummyComputeA>().is_none());
    }

    /// Tests that registering a new task handle auto-cancels the previous one.
    #[test]
    fn test_register_task_handle_auto_cancels_previous() {
        use tokio_util::sync::CancellationToken;

        let mut ctx = StateCtx::new();

        // Register first task
        let task_id1 = ctx.task_id_generator().next::<DummyState>();
        let token1 = CancellationToken::new();
        let handle1 = TaskHandle::new(task_id1, token1.clone());
        ctx.register_task_handle::<DummyState>(handle1);

        assert!(!token1.is_cancelled());

        // Register second task for same type
        let task_id2 = ctx.task_id_generator().next::<DummyState>();
        let token2 = CancellationToken::new();
        let handle2 = TaskHandle::new(task_id2, token2.clone());
        ctx.register_task_handle::<DummyState>(handle2);

        // First token should be cancelled
        assert!(token1.is_cancelled());
        // Second token should not be cancelled
        assert!(!token2.is_cancelled());

        // Only one active task for the type
        assert_eq!(ctx.active_task_type_count(), 1);

        // Active task should be the second one
        let active = ctx.get_active_task::<DummyState>().unwrap();
        assert_eq!(active.id(), task_id2);
    }

    /// Tests cancelling an active task.
    #[test]
    fn test_cancel_active_task() {
        use tokio_util::sync::CancellationToken;

        let mut ctx = StateCtx::new();
        let task_id = ctx.task_id_generator().next::<DummyState>();
        let token = CancellationToken::new();
        let handle = TaskHandle::new(task_id, token.clone());

        ctx.register_task_handle::<DummyState>(handle);

        assert!(!token.is_cancelled());
        assert!(ctx.has_active_task::<DummyState>());

        let cancelled = ctx.cancel_active_task::<DummyState>();

        assert!(cancelled);
        assert!(token.is_cancelled());
        assert!(!ctx.has_active_task::<DummyState>());
    }

    /// Tests cancelling a non-existent task returns false.
    #[test]
    fn test_cancel_nonexistent_task() {
        let mut ctx = StateCtx::new();

        let cancelled = ctx.cancel_active_task::<DummyState>();

        assert!(!cancelled);
    }

    /// Tests cancelling all tasks.
    #[test]
    fn test_cancel_all_tasks() {
        use tokio_util::sync::CancellationToken;

        let mut ctx = StateCtx::new();

        // Register multiple tasks for different types
        let token1 = CancellationToken::new();
        let handle1 = TaskHandle::new(ctx.task_id_generator().next::<DummyState>(), token1.clone());
        ctx.register_task_handle::<DummyState>(handle1);

        let token2 = CancellationToken::new();
        let handle2 = TaskHandle::new(
            ctx.task_id_generator().next::<DummyComputeA>(),
            token2.clone(),
        );
        ctx.register_task_handle::<DummyComputeA>(handle2);

        assert_eq!(ctx.active_task_type_count(), 2);
        assert!(!token1.is_cancelled());
        assert!(!token2.is_cancelled());

        ctx.cancel_all_tasks();

        assert_eq!(ctx.active_task_type_count(), 0);
        assert!(token1.is_cancelled());
        assert!(token2.is_cancelled());
    }

    /// Tests removing a task handle without cancelling it.
    #[test]
    fn test_remove_task_handle() {
        use tokio_util::sync::CancellationToken;

        let mut ctx = StateCtx::new();
        let task_id = ctx.task_id_generator().next::<DummyState>();
        let token = CancellationToken::new();
        let handle = TaskHandle::new(task_id, token.clone());

        ctx.register_task_handle::<DummyState>(handle);

        let removed = ctx.remove_task_handle::<DummyState>();

        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id(), task_id);
        // Token should NOT be cancelled (we just removed, not cancelled)
        assert!(!token.is_cancelled());
        assert!(!ctx.has_active_task::<DummyState>());
    }

    /// Tests that clear() clears task management state.
    #[test]
    fn test_clear_clears_tasks() {
        use tokio_util::sync::CancellationToken;

        let mut ctx = StateCtx::new();
        ctx.add_state(DummyState::default());

        let token = CancellationToken::new();
        let handle = TaskHandle::new(ctx.task_id_generator().next::<DummyState>(), token.clone());
        ctx.register_task_handle::<DummyState>(handle);

        assert!(ctx.has_active_task::<DummyState>());

        ctx.clear();

        assert!(!ctx.has_active_task::<DummyState>());
        assert_eq!(ctx.active_task_type_count(), 0);
    }

    /// Tests task_id_generator produces unique sequential IDs.
    #[test]
    fn test_task_id_generator_sequential() {
        let ctx = StateCtx::new();

        let id1 = ctx.task_id_generator().next::<DummyState>();
        let id2 = ctx.task_id_generator().next::<DummyState>();
        let id3 = ctx.task_id_generator().next::<DummyComputeA>();

        // Same type for first two
        assert_eq!(id1.type_id(), id2.type_id());
        // Different type for third
        assert_ne!(id1.type_id(), id3.type_id());

        // All should have different generations (sequential)
        assert_eq!(id1.generation(), 0);
        assert_eq!(id2.generation(), 1);
        assert_eq!(id3.generation(), 2);
    }

    /// Tests that task_set can be accessed mutably.
    #[test]
    fn test_task_set_mut_access() {
        let mut ctx = StateCtx::new();

        // Should be able to get mutable access to task_set
        let task_set = ctx.task_set_mut();
        assert_eq!(task_set.len(), 0);
    }

    /// Tests multiple task types can be tracked independently.
    #[test]
    fn test_multiple_task_types_independent() {
        use tokio_util::sync::CancellationToken;

        let mut ctx = StateCtx::new();

        // Register tasks for different types
        let handle1 = TaskHandle::new(
            ctx.task_id_generator().next::<DummyState>(),
            CancellationToken::new(),
        );
        ctx.register_task_handle::<DummyState>(handle1);

        let handle2 = TaskHandle::new(
            ctx.task_id_generator().next::<DummyComputeA>(),
            CancellationToken::new(),
        );
        ctx.register_task_handle::<DummyComputeA>(handle2);

        let handle3 = TaskHandle::new(
            ctx.task_id_generator().next::<DummyComputeB>(),
            CancellationToken::new(),
        );
        ctx.register_task_handle::<DummyComputeB>(handle3);

        assert_eq!(ctx.active_task_type_count(), 3);
        assert!(ctx.has_active_task::<DummyState>());
        assert!(ctx.has_active_task::<DummyComputeA>());
        assert!(ctx.has_active_task::<DummyComputeB>());

        // Cancel one type
        ctx.cancel_active_task::<DummyComputeA>();

        assert_eq!(ctx.active_task_type_count(), 2);
        assert!(ctx.has_active_task::<DummyState>());
        assert!(!ctx.has_active_task::<DummyComputeA>());
        assert!(ctx.has_active_task::<DummyComputeB>());
    }

    // ═══════════════════════════════════════════════════════════════════════
    // SPAWN_TASK, CANCEL_TASK, SHUTDOWN TESTS
    // ═══════════════════════════════════════════════════════════════════════

    /// Tests that spawn_task creates a task and returns a valid handle.
    #[tokio::test]
    async fn test_spawn_task_returns_handle() {
        let mut ctx = StateCtx::new();

        let handle = ctx.spawn_task::<DummyState, _, _>(|_cancel| async {
            // Simple task that completes immediately
        });

        // Handle should have correct type
        assert_eq!(handle.id().type_id(), TypeId::of::<DummyState>());
        assert_eq!(handle.id().generation(), 0);
        assert!(!handle.is_cancelled());

        // Task should be tracked
        assert!(ctx.has_active_task::<DummyState>());
        assert_eq!(ctx.active_task_type_count(), 1);
    }

    /// Tests that spawn_task auto-cancels previous task for same type.
    #[tokio::test]
    async fn test_spawn_task_auto_cancels_previous() {
        let mut ctx = StateCtx::new();

        // Spawn first task
        let handle1 = ctx.spawn_task::<DummyState, _, _>(|cancel| async move {
            // Wait for cancellation
            cancel.cancelled().await;
        });

        assert!(!handle1.is_cancelled());
        assert_eq!(handle1.id().generation(), 0);

        // Spawn second task for same type
        let handle2 = ctx.spawn_task::<DummyState, _, _>(|_cancel| async {
            // Complete immediately
        });

        // First task should be cancelled
        assert!(handle1.is_cancelled());
        // Second task should not be cancelled
        assert!(!handle2.is_cancelled());
        assert_eq!(handle2.id().generation(), 1);

        // Only one active task for the type
        assert_eq!(ctx.active_task_type_count(), 1);

        // Active task should be the second one
        let active = ctx.get_active_task::<DummyState>().unwrap();
        assert_eq!(active.id(), handle2.id());
    }

    /// Tests that spawn_task increments task generation.
    #[tokio::test]
    async fn test_spawn_task_increments_generation() {
        let mut ctx = StateCtx::new();

        let handle1 = ctx.spawn_task::<DummyState, _, _>(|_| async {});
        let handle2 = ctx.spawn_task::<DummyComputeA, _, _>(|_| async {});
        let handle3 = ctx.spawn_task::<DummyState, _, _>(|_| async {});

        assert_eq!(handle1.id().generation(), 0);
        assert_eq!(handle2.id().generation(), 1);
        assert_eq!(handle3.id().generation(), 2);
    }

    /// Tests that cancel_task cancels the specified task.
    #[tokio::test]
    async fn test_cancel_task() {
        let mut ctx = StateCtx::new();

        let handle = ctx.spawn_task::<DummyState, _, _>(|cancel| async move {
            cancel.cancelled().await;
        });

        assert!(!handle.is_cancelled());

        ctx.cancel_task(&handle);

        assert!(handle.is_cancelled());
    }

    /// Tests that cancel_task only cancels the specified task.
    #[tokio::test]
    async fn test_cancel_task_only_cancels_specified() {
        let mut ctx = StateCtx::new();

        let handle1 = ctx.spawn_task::<DummyState, _, _>(|cancel| async move {
            cancel.cancelled().await;
        });

        let handle2 = ctx.spawn_task::<DummyComputeA, _, _>(|cancel| async move {
            cancel.cancelled().await;
        });

        assert!(!handle1.is_cancelled());
        assert!(!handle2.is_cancelled());

        ctx.cancel_task(&handle1);

        assert!(handle1.is_cancelled());
        assert!(!handle2.is_cancelled());
    }

    /// Tests that shutdown cancels all tasks and awaits completion.
    #[tokio::test]
    async fn test_shutdown_cancels_all_tasks() {
        let mut ctx = StateCtx::new();

        let handle1 = ctx.spawn_task::<DummyState, _, _>(|cancel| async move {
            cancel.cancelled().await;
        });

        let handle2 = ctx.spawn_task::<DummyComputeA, _, _>(|cancel| async move {
            cancel.cancelled().await;
        });

        assert_eq!(ctx.active_task_type_count(), 2);
        assert!(!handle1.is_cancelled());
        assert!(!handle2.is_cancelled());

        // Shutdown should cancel all tasks and await their completion
        ctx.shutdown().await;

        // Cancellation tokens should be triggered
        assert!(handle1.is_cancelled());
        assert!(handle2.is_cancelled());
        // Active tasks map should be empty
        assert_eq!(ctx.active_task_type_count(), 0);
        // Task set should be empty
        assert_eq!(ctx.task_count(), 0);
    }

    /// Tests that shutdown works with empty task set.
    #[tokio::test]
    async fn test_shutdown_empty() {
        let mut ctx = StateCtx::new();

        // Should not panic with empty task set
        ctx.shutdown().await;

        assert_eq!(ctx.task_count(), 0);
        assert_eq!(ctx.active_task_type_count(), 0);
    }

    /// Tests that spawned tasks can access the cancellation token.
    #[tokio::test]
    async fn test_spawn_task_cancellation_token_accessible() {
        let mut ctx = StateCtx::new();

        let handle = ctx.spawn_task::<DummyState, _, _>(|cancel| async move {
            // Task should be able to check cancellation status
            assert!(!cancel.is_cancelled());
            cancel.cancelled().await;
            // After cancellation signal, this should be true
            assert!(cancel.is_cancelled());
        });

        // Task should not be cancelled yet
        assert!(!handle.is_cancelled());

        // Cancel the task
        ctx.cancel_task(&handle);

        // Handle should show cancelled
        assert!(handle.is_cancelled());

        // Clean up
        ctx.shutdown().await;
    }

    /// Tests that tasks can complete normally (not just via cancellation).
    #[tokio::test]
    async fn test_spawn_task_normal_completion() {
        let mut ctx = StateCtx::new();
        let completed = Arc::new(AtomicBool::new(false));
        let completed_clone = Arc::clone(&completed);

        let _handle = ctx.spawn_task::<DummyState, _, _>(|_cancel| async move {
            // Task completes without checking cancellation
            completed_clone.store(true, Ordering::SeqCst);
        });

        // Yield to allow task to run
        tokio::task::yield_now().await;

        // Wait for all tasks
        ctx.shutdown().await;

        assert!(completed.load(Ordering::SeqCst));
    }

    /// Tests that multiple task types can be spawned and tracked independently.
    #[tokio::test]
    async fn test_spawn_multiple_task_types() {
        let mut ctx = StateCtx::new();

        let handle1 = ctx.spawn_task::<DummyState, _, _>(|cancel| async move {
            cancel.cancelled().await;
        });

        let handle2 = ctx.spawn_task::<DummyComputeA, _, _>(|cancel| async move {
            cancel.cancelled().await;
        });

        let handle3 = ctx.spawn_task::<DummyComputeB, _, _>(|cancel| async move {
            cancel.cancelled().await;
        });

        assert_eq!(ctx.active_task_type_count(), 3);
        assert!(ctx.has_active_task::<DummyState>());
        assert!(ctx.has_active_task::<DummyComputeA>());
        assert!(ctx.has_active_task::<DummyComputeB>());

        // All handles should be distinct
        assert_ne!(handle1.id(), handle2.id());
        assert_ne!(handle2.id(), handle3.id());
        assert_ne!(handle1.id(), handle3.id());

        ctx.shutdown().await;
    }

    /// Tests that spawn_task works with the updater pattern.
    #[tokio::test]
    async fn test_spawn_task_with_updater() {
        let mut ctx = StateCtx::new();
        ctx.record_compute(DummyComputeFromCommand { value: 0 });

        let updater = ctx.updater();

        let _handle = ctx.spawn_task::<DummyComputeFromCommand, _, _>(|_cancel| async move {
            // Yield to simulate async work
            tokio::task::yield_now().await;
            // Update compute via updater
            updater.set(DummyComputeFromCommand { value: 42 });
        });

        // Yield to allow task to run and complete
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        // Sync the compute values
        ctx.sync_computes();

        assert_eq!(ctx.cached::<DummyComputeFromCommand>().unwrap().value, 42);

        ctx.shutdown().await;
    }
}
