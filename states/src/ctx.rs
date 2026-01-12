use std::{
    any::TypeId,
    cell::{RefCell, RefMut},
    collections::{BTreeMap, HashMap, VecDeque},
    ptr::NonNull,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use log::{Level, log_enabled, trace};
#[cfg(not(target_arch = "wasm32"))]
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::{
    Command, CommandSnapshot, Dep, LatestOnlyUpdater, Reader, TaskHandle, TaskId, TaskIdGenerator,
    Updater, state::UpdateMessage,
};

use super::{Compute, Stage, State, StateRuntime};

/// `StateCtx` acts as the central manager for all states and computes.
///
/// It holds the storage for states and computes, manages their lifecycle,
/// and orchestrates the re-computation of derived states when dependencies change.
///
/// # Control Flow
///
/// 1. **State mutation**: Use `update::<T>()` to modify state - this automatically
///    marks all dependent computes as dirty via the dependency graph.
///
/// 2. **Compute execution**: Two modes available:
///    - `run_all_dirty()`: Runs all dirty computes (for frame loop)
///    - `run::<T>()`: Runs a specific compute and its dirty dependencies (for user events)
///
/// 3. **Sync results**: Call `sync_computes()` to apply async compute results.
///
/// 4. **Command execution**: Commands are queued via `enqueue_command::<T>()` and executed
///    at end-of-frame via `flush_commands()`. This ensures commands never execute mid-frame
///    and always operate on snapshot copies of state/compute values.
///
/// # Task Management
///
/// `StateCtx` manages async tasks via structured concurrency:
/// - `JoinSet` tracks all spawned async tasks
/// - `TaskHandle` by `TypeId` enables auto-cancellation when a new task is spawned for the same type
/// - `TaskIdGenerator` provides unique task identifiers
/// - `current_generation` provides out-of-order safety for async completion (latest-only writes)
#[derive(Debug)]
pub struct StateCtx {
    runtime: StateRuntime,

    states: BTreeMap<TypeId, (RefCell<Box<dyn State>>, Stage)>,
    // TODO: We better not store Box, consider using raw pointer to reduce indirection
    // We will not using RefCell with Box, the State should be Sized, and it will not needs to by Any to downcast, we just use NoNullPointer with unsafe
    computes: BTreeMap<TypeId, (RefCell<Box<dyn Compute>>, Stage)>,

    /// Manual-only commands/effects.
    ///
    /// Commands are *not* part of the compute dependency graph and will never be
    /// executed by `run_all_dirty()`. They must be invoked explicitly via
    /// `enqueue_command::<T>()` and `flush_commands()` (preferred), or the
    /// deprecated `dispatch::<T>()`.
    commands: BTreeMap<TypeId, RefCell<Box<dyn Command>>>,

    /// Queue of commands to be executed at end-of-frame.
    ///
    /// Commands are enqueued via `enqueue_command::<T>()` and executed sequentially
    /// during `flush_commands()`. Each command receives a `CommandSnapshot` containing
    /// owned clones of states and computes at flush time.
    command_queue: VecDeque<TypeId>,

    /// Set of spawned async tasks for structured concurrency (native only).
    ///
    /// On native targets, we use Tokio's `JoinSet` so tasks can be awaited/aborted.
    /// On WASM, there is no Tokio runtime reactor, so commands/tasks are spawned via
    /// `wasm_bindgen_futures::spawn_local` and are not tracked here.
    #[cfg(not(target_arch = "wasm32"))]
    task_set: JoinSet<()>,

    /// Active task handles indexed by compute type.
    ///
    /// When a new task is spawned for a compute type that already has an active task,
    /// the previous task's cancellation token is triggered before spawning the new one.
    /// This implements the auto-cancellation pattern for superseded tasks.
    active_tasks: HashMap<TypeId, TaskHandle>,

    /// Per-type "current generation" counters used for latest-only enforcement.
    ///
    /// For each async task type `T`, we keep an `Arc<AtomicU64>` that stores the generation of
    /// the most recently spawned task for that type. Async completion must check that its
    /// captured generation still matches this atomic before publishing results.
    current_generation: HashMap<TypeId, Arc<AtomicU64>>,

    /// Generator for unique task identifiers.
    ///
    /// Combines `TypeId` with a monotonically increasing generation counter
    /// to produce globally unique `TaskId`s.
    task_id_generator: TaskIdGenerator,
}

impl Default for StateCtx {
    fn default() -> Self {
        Self::new()
    }
}

type MarkEntity<'a> = (
    Option<&'a mut (RefCell<Box<dyn State + 'static>>, Stage)>,
    Option<&'a mut (RefCell<Box<dyn Compute + 'static>>, Stage)>,
);

impl StateCtx {
    /// Creates a new, empty `StateCtx`.
    pub fn new() -> Self {
        let runtime = StateRuntime::new();
        let computes = BTreeMap::new();
        let states = BTreeMap::new();
        let commands = BTreeMap::new();
        let command_queue = VecDeque::new();
        #[cfg(not(target_arch = "wasm32"))]
        let task_set = JoinSet::new();
        let active_tasks = HashMap::new();
        let current_generation = HashMap::new();
        let task_id_generator = TaskIdGenerator::new();
        Self {
            runtime,
            states,
            computes,
            commands,
            command_queue,
            #[cfg(not(target_arch = "wasm32"))]
            task_set,
            active_tasks,
            current_generation,
            task_id_generator,
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // STATE REGISTRATION
    // ═══════════════════════════════════════════════════════════════════════

    /// Adds a new `State` to the context.
    ///
    /// The state is initialized and marked as `BeforeInit`.
    pub fn add_state<T: State>(&mut self, state: T) {
        let id = TypeId::of::<T>();
        trace!("Record State: id={:?}, state={:?}", id, state);
        self.states
            .insert(id, (RefCell::new(Box::new(state)), Stage::BeforeInit));
    }

    /// Registers a `Compute` (derived state) to the context.
    ///
    /// The compute is recorded in the runtime and initialized.
    pub fn record_compute<T: Compute>(&mut self, compute: T) {
        let id = TypeId::of::<T>();
        trace!("Record Compute: id={:?}, compute={:?}", id, compute);
        self.runtime.record(&compute);
        self.computes
            .insert(id, (RefCell::new(Box::new(compute)), Stage::BeforeInit));
    }

    /// Registers a manual-only `Command` to the context.
    ///
    /// Commands are intentionally *not* recorded in the dependency graph, and thus
    /// will never be auto-dirtied or auto-executed. The command type must be known
    /// at the call-site (i.e. you dispatch by type).
    pub fn record_command<T: Command>(&mut self, command: T) {
        let id = TypeId::of::<T>();
        trace!("Record Command: id={:?}, command={:?}", id, command);
        self.commands.insert(id, RefCell::new(Box::new(command)));
    }

    // ═══════════════════════════════════════════════════════════════════════
    // STATE MUTATION WITH AUTO-DIRTY PROPAGATION
    // ═══════════════════════════════════════════════════════════════════════

    /// Updates a state and automatically marks all dependent computes as dirty.
    ///
    /// This is the preferred way to modify state as it ensures the dependency
    /// graph is respected and dependent computes will be re-run.
    ///
    /// # Example
    ///
    /// ```ignore
    /// ctx.update::<MyState>(|state| {
    ///     state.value = 42;
    /// });
    /// // All computes that depend on MyState are now marked dirty
    /// ```
    pub fn update<T: State>(&mut self, f: impl FnOnce(&mut T)) {
        let id = TypeId::of::<T>();

        // Apply the mutation
        {
            let state = self.get_state_mut(&id);
            f(state.as_any_mut().downcast_mut::<T>().unwrap());
        }

        // Auto-propagate dirty to all dependent computes
        self.propagate_dirty_from(&id);
    }

    /// Propagates dirty status from a changed state/compute to all its dependents.
    fn propagate_dirty_from(&mut self, source_id: &TypeId) {
        // Collect dependent IDs first to avoid borrow issues
        let dependent_ids: Vec<TypeId> = self
            .runtime
            .graph_mut()
            .dependents(*source_id)
            .copied()
            .collect();

        for dep_id in dependent_ids {
            // Only mark computes as dirty (states don't get dirty from propagation)
            if self.computes.contains_key(&dep_id) {
                trace!(
                    "Auto-marking compute {:?} as dirty (dependency changed)",
                    dep_id
                );
                self.mark_dirty(&dep_id);
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // COMPUTE EXECUTION
    // ═══════════════════════════════════════════════════════════════════════

    /// Runs a specific compute and all its dirty dependencies in topological order.
    ///
    /// This is useful for user-triggered actions where you want to run a specific
    /// compute immediately rather than waiting for the next frame.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // User clicks "Create User" button
    /// ctx.update::<CreateUserInput>(|input| {
    ///     input.username = Some("alice".to_string());
    /// });
    /// ctx.run::<CreateUserCompute>(); // Run immediately
    /// ```
    pub fn run<T: Compute + 'static>(&mut self) {
        let target_id = TypeId::of::<T>();
        // Mark as dirty to ensure it runs even if not automatically dirtied
        self.mark_dirty(&target_id);
        self.run_by_id_with_deps(&target_id);
    }

    /// Enqueues a command for execution at end-of-frame.
    ///
    /// Commands are stored in a queue and executed sequentially during `flush_commands()`.
    /// This ensures commands never execute mid-frame and always operate on snapshot copies
    /// of state/compute values taken at flush time.
    ///
    /// # Panics
    ///
    /// Panics if the command type was not previously registered via `record_command::<T>()`.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In UI event handler:
    /// ctx.enqueue_command::<ToggleApiStatusCommand>();
    ///
    /// // At end-of-frame (in eframe::App::update):
    /// ctx.flush_commands();
    /// ```
    pub fn enqueue_command<T: Command + 'static>(&mut self) {
        let id = TypeId::of::<T>();
        if !self.commands.contains_key(&id) {
            panic!(
                "No command found for type {}. Did you forget to call `record_command::<T>()`?",
                std::any::type_name::<T>()
            );
        }
        trace!("Enqueue command: {}", std::any::type_name::<T>());
        self.command_queue.push_back(id);
    }

    /// Executes all queued commands at end-of-frame.
    ///
    /// Each command receives a `CommandSnapshot` containing owned clones of states and computes
    /// at flush time. Commands are executed sequentially in the order they were enqueued.
    ///
    /// This method should be called once per frame, after UI rendering and before the next
    /// `sync_computes()` call to apply any updates emitted by commands.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In eframe::App::update, after UI rendering:
    /// ctx.sync_computes();      // Apply async results from prior frame
    /// ctx.flush_commands();     // Execute queued commands
    /// ctx.sync_computes();      // Apply updates from commands
    /// ctx.run_all_dirty();      // Run dirty computes
    /// ```
    pub fn flush_commands(&mut self) {
        let queue_len = self.command_queue.len();
        if queue_len == 0 {
            return;
        }

        trace!("Flushing {} queued commands", queue_len);

        // Execute each queued command with a fresh snapshot
        // Each command gets a snapshot of the current state at execution time
        while let Some(id) = self.command_queue.pop_front() {
            self.execute_command_by_id(&id);
        }
    }

    /// Returns the number of commands currently in the queue.
    pub fn command_queue_len(&self) -> usize {
        self.command_queue.len()
    }

    /// Executes a single command by spawning it as an async task.
    ///
    /// The command is given a snapshot of current state/compute values and a cancellation token.
    /// Results are delivered via the Updater channel and applied during subsequent sync_computes() calls.
    ///
    /// ## Out-of-order safety (latest-only)
    ///
    /// Commands can be triggered multiple times (e.g. rapid UI interactions), so multiple
    /// in-flight async completions of the *same* command type may race. We therefore run each
    /// command under a per-command `TypeId` generation gate ("latest-only") and pass a
    /// `LatestOnlyUpdater` into the command future so stale completions cannot publish.
    fn execute_command_by_id(&mut self, id: &TypeId) {
        let Some(cell) = self.commands.get(id) else {
            panic!("No command found for id: {id:?}");
        };

        // Build snapshot from currently registered states + computes (commands may read both).
        //
        // Commands are intentionally not part of the dependency graph, so we construct
        // the snapshot from what's available in this context at execution time.
        // Only states/computes that implement SnapshotClone::clone_boxed will be included.
        let snapshot = self.create_command_snapshot();

        // Create a cancellation token for this command execution
        let cancel = CancellationToken::new();

        // Latest-only generation gate keyed by command TypeId
        let command_id = *id;
        let gen_cell = self
            .current_generation
            .entry(command_id)
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .clone();

        let generation = gen_cell.fetch_add(1, Ordering::Relaxed) + 1;
        gen_cell.store(generation, Ordering::Relaxed);

        let gen_cell_for_check = Arc::clone(&gen_cell);
        let current_check: Arc<dyn Fn(TaskId) -> bool + Send + Sync> = Arc::new(move |tid| {
            tid.type_id() == command_id
                && gen_cell_for_check.load(Ordering::Relaxed) == tid.generation()
        });

        let updater = self.updater();
        let task_id = TaskId::new(command_id, generation);
        let latest: LatestOnlyUpdater = updater.latest_only(task_id, current_check);

        // We must not hold any borrows into `self.commands` across a call that mutably borrows
        // `self` (spawning). So we create the future inside a short scope and drop the `Ref`
        // before spawning.
        let future = {
            let borrowed = cell.borrow();
            trace!("Executing command: id={:?}", id);
            borrowed.run(snapshot, latest, cancel)
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.task_set.spawn(future);
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                future.await;
            });
        }
    }

    /// Creates a `CommandSnapshot` from the current states and computes.
    ///
    /// Only states and computes that implement `SnapshotClone::clone_boxed` returning `Some`
    /// will be included in the snapshot.
    fn create_command_snapshot(&self) -> CommandSnapshot {
        let states = self
            .states
            .iter()
            .filter_map(|(id, (cell, _))| {
                let borrowed = cell.borrow();
                borrowed.clone_boxed().map(|boxed| (*id, boxed))
            })
            .collect::<Vec<_>>();

        let computes = self
            .computes
            .iter()
            .filter_map(|(id, (cell, _))| {
                let borrowed = cell.borrow();
                borrowed.clone_boxed().map(|boxed| (*id, boxed))
            })
            .collect::<Vec<_>>();

        CommandSnapshot::from_iters(states.into_iter(), computes.into_iter())
    }

    /// Runs a compute by TypeId, first running any dirty dependencies in topological order.
    /// Each dependency is synced before the next one runs, ensuring dependent computes
    /// can read the updated values.
    fn run_by_id_with_deps(&mut self, target_id: &TypeId) {
        // Get dirty dependencies in topological order
        let deps_sorted: Vec<TypeId> = self.runtime.graph_mut().dependencies_sorted(*target_id);

        // Run dirty dependencies first, syncing after each so subsequent computes
        // can read the updated values
        for dep_id in deps_sorted {
            if self.is_compute_dirty(&dep_id) {
                self.run_single_compute(&dep_id);
                // Sync immediately so dependent computes can read the result
                self.sync_computes();
            }
        }

        // Run the target compute if it's dirty or before init
        if self.is_compute_dirty(target_id) {
            self.run_single_compute(target_id);
        }
    }

    /// Checks if a compute is in a state that requires running (Dirty or BeforeInit).
    fn is_compute_dirty(&self, id: &TypeId) -> bool {
        self.computes
            .get(id)
            .map(|(_, stage)| matches!(stage, Stage::Dirty | Stage::BeforeInit))
            .unwrap_or(false)
    }

    /// Runs a single compute by TypeId (without checking dependencies).
    fn run_single_compute(&mut self, id: &TypeId) {
        let compute = self.computes.get(id);
        if compute.is_none() {
            trace!("Skipping non-compute dependency: {:?}", id);
            return;
        }

        let compute = compute.unwrap();
        let borrowed = compute.0.borrow();
        let (state_deps, compute_deps) = borrowed.deps();

        let deps = Dep::new(
            state_deps
                .iter()
                .map(|&dep_id| (dep_id, self.get_state_ptr(&dep_id))),
            compute_deps
                .iter()
                .map(|&dep_id| (dep_id, self.get_compute_ptr(&dep_id))),
        );

        trace!("Run compute: {:?}", borrowed.name());
        borrowed.compute(deps, self.updater());
        drop(borrowed);

        self.mark_pending(id);
    }

    /// Triggers the execution of all dirty computes.
    ///
    /// This iterates through computes marked as dirty or before init, resolves their
    /// dependencies, and executes their `compute` method.
    ///
    /// Typically called once per frame in the UI loop.
    pub fn run_all_dirty(&mut self) {
        let dirty_computes = self.dirty_computes();
        let mut pending_ids: Vec<TypeId> = Vec::new();
        let mut pending_compute_names = Vec::new();
        for (id, dirty_compute) in dirty_computes {
            let (states, computes) = dirty_compute.deps();
            let deps = Dep::new(
                states
                    .iter()
                    .map(|&dep_id| (dep_id, self.get_state_ptr(&dep_id))),
                computes
                    .iter()
                    .map(|&dep_id| (dep_id, self.get_compute_ptr(&dep_id))),
            );
            trace!("Run compute: {:?}", dirty_compute.name());
            if log_enabled!(Level::Trace) {
                pending_compute_names.push(dirty_compute.name());
            }
            pending_ids.push(*id);
            dirty_compute.compute(deps, self.updater());
        }
        for id in pending_ids {
            self.mark_pending(&id);
        }
        if log_enabled!(Level::Trace) {
            for name in pending_compute_names {
                trace!("Compute pending: {:?}", name);
            }
        }
    }

    /// Legacy alias for `run_all_dirty()`.
    #[deprecated(note = "Use `run_all_dirty()` instead for clarity")]
    pub fn run_computed(&mut self) {
        self.run_all_dirty();
    }

    // ═══════════════════════════════════════════════════════════════════════
    // STATE ACCESS
    // ═══════════════════════════════════════════════════════════════════════

    fn get_state_mut(&self, id: &TypeId) -> &'static mut dyn State {
        unsafe {
            self.states[id]
                .0
                .as_ptr()
                .as_mut()
                .map(|v| v.as_mut())
                .unwrap()
        }
    }

    /// Returns a read-only reference to a state.
    ///
    /// Use this method when you only need to read state values without modifying them.
    /// For modifications, use `update::<T>()` which automatically propagates dirty status.
    pub fn state<T: State>(&self) -> &T {
        self.get_state_mut(&TypeId::of::<T>())
            .as_any()
            .downcast_ref::<T>()
            .unwrap()
    }

    /// Returns a mutable reference to a state.
    ///
    /// **Warning**: Prefer using `update::<T>()` instead for modifications, as it automatically
    /// propagates dirty status to dependent computes. Use `state::<T>()` for read-only access.
    pub fn state_mut<T: State>(&self) -> &'static mut T {
        self.get_state_mut(&TypeId::of::<T>())
            .as_any_mut()
            .downcast_mut::<T>()
            .unwrap()
    }

    fn get_state_ptr(&self, id: &TypeId) -> NonNull<dyn State> {
        // TODO: Maybe we should use more serius error here, cause the state should exists in state
        unsafe { NonNull::new_unchecked(self.get_state_mut(id)) }
    }

    fn get_compute_mut(&self, id: &TypeId) -> &'static mut dyn Compute {
        unsafe {
            self.computes[id]
                .0
                .as_ptr()
                .as_mut()
                .map(|v| v.as_mut())
                .unwrap()
        }
    }

    fn get_compute_ptr(&self, id: &TypeId) -> NonNull<dyn Compute> {
        unsafe { NonNull::new_unchecked(self.get_compute_mut(id)) }
    }

    /// Returns a read-only reference to a compute.
    ///
    /// This mirrors `state::<T>()` but for computes. Prefer this for reading compute values
    /// from UI code and for tests.
    ///
    /// # Panics
    ///
    /// Panics if the compute type `T` was not registered via `record_compute::<T>(...)`.
    pub fn compute<T: Compute>(&self) -> &T {
        self.get_compute_mut(&TypeId::of::<T>())
            .as_any()
            .downcast_ref::<T>()
            .unwrap()
    }

    /// Retrieves a reference to a cached compute value if available.
    pub fn cached<T: Compute + Sized>(&self) -> Option<&'static T> {
        unsafe {
            self.computes[&TypeId::of::<T>()]
                .0
                .as_ptr()
                .as_mut()
                .and_then(|v| v.as_any().downcast_ref())
        }
    }

    // ═══════════════════════════════════════════════════════════════════════
    // SYNC AND RUNTIME
    // ═══════════════════════════════════════════════════════════════════════

    /// Synchronizes both computes and states by processing updates from the runtime.
    ///
    /// This processes any pending updates sent via the `Updater` and applies them
    /// to the respective computes or states, marking computes as clean.
    pub fn sync_computes(&mut self) {
        let cur_len = self.runtime().receiver().len();
        trace!(
            "Start Sync Updates, Cur Received {:?} Update Messages",
            cur_len
        );
        for _ in 0..cur_len {
            if let Ok(msg) = self.runtime().receiver().try_recv() {
                match msg {
                    UpdateMessage::Compute(id, boxed) => {
                        let compute = self.computes.get_mut(&id).unwrap_or_else(|| {
                            panic!(
                                "Received compute update for an unregistered compute id={:?}. \
This is a programmer error (e.g. a Command/Compute called `Updater::set(...)` for a compute type that was never `record_compute(...)`).",
                                id
                            )
                        });

                        let computed_name = compute.0.borrow().name();

                        // A compute result may arrive when the compute is in various states:
                        // - Pending: normal case, compute was run and we're receiving its result
                        // - Clean: compute was already synced (e.g., from a previous async response)
                        // - Dirty/BeforeInit: compute was re-triggered before previous result arrived
                        // In all cases, we should apply the result if it arrives.
                        trace!(
                            "Received Compute Update, compute={:?}, current_stage={:?}",
                            computed_name, compute.1
                        );
                        compute.0.borrow_mut().assign_box(boxed);
                        self.mark_clean(&id);
                    }
                    UpdateMessage::State(id, boxed) => {
                        let state = self.states.get_mut(&id).unwrap_or_else(|| {
                            panic!(
                                "Received state update for an unregistered state id={:?}. \
This is a programmer error (e.g. a Command called `Updater::set_state(...)` for a state type that was never `add_state(...)`).",
                                id
                            )
                        });

                        let state_name = state.0.borrow().name();
                        trace!(
                            "Received State Update, state={:?}, current_stage={:?}",
                            state_name, state.1
                        );

                        // Replace the state with the new value using assign_box
                        state.0.borrow_mut().assign_box(boxed);

                        // Propagate dirty to dependent computes
                        self.propagate_dirty_from(&id);
                    }
                    UpdateMessage::EnqueueCommand(id) => {
                        // Enqueue command requested by a Compute via Updater.
                        // This allows Computes to trigger commands without doing IO themselves.
                        if !self.commands.contains_key(&id) {
                            panic!(
                                "Received enqueue request for an unregistered command id={:?}. \
This is a programmer error (e.g. a Compute called `Updater::enqueue_command::<T>()` for a command type that was never `record_command(...)`).",
                                id
                            );
                        }
                        trace!("Enqueue command from Updater: id={:?}", id);
                        self.command_queue.push_back(id);
                    }
                }
            }
        }
    }

    pub fn runtime(&self) -> &StateRuntime {
        &self.runtime
    }

    // ═══════════════════════════════════════════════════════════════════════
    // DIRTY TRACKING
    // ═══════════════════════════════════════════════════════════════════════

    // TODO: Doc for how state and compute state transforms and how they works
    pub fn dirty_computes(&self) -> impl Iterator<Item = (&TypeId, RefMut<'_, Box<dyn Compute>>)> {
        self.computes
            .iter()
            .filter_map(|(type_id, (state_cell, compute_state))| {
                if matches!(compute_state, &Stage::Dirty | &Stage::BeforeInit) {
                    trace!(
                        "Run Compute which is {:?} = {:?}",
                        compute_state,
                        state_cell.borrow().name()
                    );
                    Some((type_id, state_cell.borrow_mut()))
                } else {
                    None
                }
            })
    }

    fn get_mut_ref(&mut self, id: &TypeId) -> MarkEntity<'_> {
        let state_entry = self.states.get_mut(id);
        let compute_entry = self.computes.get_mut(id);
        (state_entry, compute_entry)
    }

    fn mark_as(&mut self, id: &TypeId, tobe: Stage) {
        let (state_entry, compute_entry) = self.get_mut_ref(id);
        match (state_entry, compute_entry) {
            (Some(state), None) => {
                state.1 = tobe;
            }
            (None, Some(compute)) => {
                compute.1 = tobe;
            }
            _ => {
                panic!("No state or compute found for id: {:?}", id);
            }
        }
    }

    pub fn mark_before_init(&mut self, id: &TypeId) {
        self.mark_as(id, Stage::BeforeInit);
    }

    pub fn mark_dirty(&mut self, id: &TypeId) {
        self.mark_as(id, Stage::Dirty);
    }

    pub fn mark_pending(&mut self, id: &TypeId) {
        self.mark_as(id, Stage::Pending);
    }

    pub fn mark_clean(&mut self, id: &TypeId) {
        self.mark_as(id, Stage::Clean);
    }

    pub fn clear(&mut self) {
        // Cancel all tasks first to ensure proper cleanup before clearing state
        self.cancel_all_tasks();
        self.states.clear();
        self.computes.clear();
        self.commands.clear();
    }

    pub fn reader(&self) -> Reader {
        self.runtime().into()
    }

    pub fn updater(&self) -> Updater {
        self.runtime().into()
    }

    // ═══════════════════════════════════════════════════════════════════════
    // TASK MANAGEMENT
    // ═══════════════════════════════════════════════════════════════════════

    /// Returns a reference to the `JoinSet` managing all spawned async tasks.
    ///
    /// This provides read access to the task set for inspection purposes.
    /// Use `task_set_mut()` for spawning or aborting tasks.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn task_set(&self) -> &JoinSet<()> {
        &self.task_set
    }

    /// Returns a mutable reference to the `JoinSet` managing all spawned async tasks.
    ///
    /// This allows spawning new tasks, aborting tasks, and awaiting task completion.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn task_set_mut(&mut self) -> &mut JoinSet<()> {
        &mut self.task_set
    }

    /// Returns a reference to the map of active task handles by compute type.
    ///
    /// Each entry maps a `TypeId` (representing a compute type) to the `TaskHandle`
    /// of the currently active task for that type.
    pub fn active_tasks(&self) -> &HashMap<TypeId, TaskHandle> {
        &self.active_tasks
    }

    /// Returns a mutable reference to the map of active task handles.
    pub fn active_tasks_mut(&mut self) -> &mut HashMap<TypeId, TaskHandle> {
        &mut self.active_tasks
    }

    /// Returns a reference to the task ID generator.
    ///
    /// The generator produces unique `TaskId`s combining `TypeId` with a
    /// monotonically increasing generation counter.
    pub fn task_id_generator(&self) -> &TaskIdGenerator {
        &self.task_id_generator
    }

    /// Returns the number of tasks currently in the `JoinSet`.
    ///
    /// This includes both running and completed (but not yet joined) tasks.
    pub fn task_count(&self) -> usize {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.task_set.len()
        }

        #[cfg(target_arch = "wasm32")]
        {
            0
        }
    }

    /// Returns the number of compute types with active task handles.
    ///
    /// This represents the number of compute types that currently have
    /// a tracked task (which may or may not still be running).
    pub fn active_task_type_count(&self) -> usize {
        self.active_tasks.len()
    }

    /// Checks if there is an active task for the given compute type.
    pub fn has_active_task<T: 'static>(&self) -> bool {
        self.active_tasks.contains_key(&TypeId::of::<T>())
    }

    /// Gets the active `TaskHandle` for the given compute type, if any.
    pub fn get_active_task<T: 'static>(&self) -> Option<&TaskHandle> {
        self.active_tasks.get(&TypeId::of::<T>())
    }

    /// Returns `true` if `task_id` is still the current active task for type `T`.
    ///
    /// This is the core out-of-order safety primitive: async completions should check
    /// they are still "current" before publishing results via `Updater::set(...)`.
    pub fn is_task_current<T: 'static>(&self, task_id: TaskId) -> bool {
        self.get_active_task::<T>()
            .is_some_and(|h| h.id() == task_id)
    }

    /// Debug-only assertion that a task is still current for type `T`.
    ///
    /// Use this right before publishing results from async work. In release builds
    /// this compiles to a fast boolean check (`is_task_current`) that you can use
    /// to early-return and drop stale results.
    #[cfg(debug_assertions)]
    pub fn debug_assert_task_current<T: 'static>(&self, task_id: TaskId) {
        let active = self.get_active_task::<T>().map(|h| h.id());
        debug_assert!(
            active == Some(task_id),
            "stale async publish attempt for type {}: task_id={:?}, active={:?}",
            std::any::type_name::<T>(),
            task_id,
            active
        );
    }

    /// Cancels the active task for the given compute type, if any.
    ///
    /// This triggers the cancellation token for the task, signaling it
    /// to stop at its next cancellation check point. The task handle
    /// is removed from the active tasks map.
    ///
    /// Returns `true` if a task was cancelled, `false` if no active task
    /// existed for the type.
    pub fn cancel_active_task<T: 'static>(&mut self) -> bool {
        if let Some(handle) = self.active_tasks.remove(&TypeId::of::<T>()) {
            trace!(
                "Cancelling active task for type {:?}",
                std::any::type_name::<T>()
            );
            handle.cancel();
            true
        } else {
            false
        }
    }

    /// Cancels all active tasks.
    ///
    /// This triggers the cancellation token for all tracked tasks and clears
    /// the active tasks map. Additionally, aborts all tasks in the `JoinSet`.
    pub fn cancel_all_tasks(&mut self) {
        trace!("Cancelling all {} active tasks", self.active_tasks.len());
        for (type_id, handle) in self.active_tasks.drain() {
            trace!("Cancelling task for type {:?}", type_id);
            handle.cancel();
        }
        #[cfg(not(target_arch = "wasm32"))]
        self.task_set.abort_all();
    }

    /// Registers a new task handle for a compute type.
    ///
    /// If a task handle already exists for the type, its cancellation token
    /// is triggered before being replaced with the new handle.
    ///
    /// This is typically called by `spawn_task` implementations to track
    /// the new task and enable auto-cancellation of superseded tasks.
    pub fn register_task_handle<T: 'static>(&mut self, handle: TaskHandle) {
        let type_id = TypeId::of::<T>();

        // Cancel previous task for this type if it exists
        if let Some(old_handle) = self.active_tasks.remove(&type_id) {
            trace!(
                "Auto-cancelling previous task for type {} (generation {})",
                std::any::type_name::<T>(),
                old_handle.id().generation()
            );
            old_handle.cancel();
        }

        trace!(
            "Registering new task for type {} (generation {})",
            std::any::type_name::<T>(),
            handle.id().generation()
        );
        self.active_tasks.insert(type_id, handle);
    }

    /// Removes and returns the task handle for a compute type without cancelling it.
    ///
    /// This is useful when a task completes normally and should be removed
    /// from tracking without triggering cancellation.
    pub fn remove_task_handle<T: 'static>(&mut self) -> Option<TaskHandle> {
        self.active_tasks.remove(&TypeId::of::<T>())
    }

    /// Spawns an async task for a compute type, auto-cancelling any previous task for the same type.
    ///
    /// This method implements structured concurrency by:
    /// 1. Generating a unique `TaskId` for the task
    /// 2. Creating a `CancellationToken` for cooperative cancellation
    /// 3. Auto-cancelling any existing task for the same compute type
    /// 4. Spawning the future onto the `JoinSet`
    /// 5. Registering the `TaskHandle` for tracking
    ///
    /// The spawned future receives a `CancellationToken` that it should check periodically
    /// to respond to cancellation requests gracefully.
    ///
    /// # Type Parameters
    ///
    /// * `T` - The compute type this task is associated with. Used for auto-cancellation
    ///   when a new task is spawned for the same type.
    /// * `F` - The future type that will be spawned.
    ///
    /// # Arguments
    ///
    /// * `f` - A function that takes a `CancellationToken` and returns a future to spawn.
    ///
    /// # Returns
    ///
    /// A `TaskHandle` that can be used to cancel the task or check its cancellation status.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let handle = ctx.spawn_task::<ApiStatus>(|cancel| async move {
    ///     tokio::select! {
    ///         _ = cancel.cancelled() => {
    ///             // Task was cancelled
    ///         }
    ///         result = fetch_api_status() => {
    ///             // Process result
    ///             updater.set(ApiStatus { data: result });
    ///         }
    ///     }
    /// });
    /// ```
    pub fn spawn_task<T, F, Fut>(&mut self, f: F) -> TaskHandle
    where
        T: 'static,
        F: FnOnce(CancellationToken) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // Legacy API: keep the old closure signature.
        //
        // NOTE:
        // This does NOT enforce latest-only writes. Callers that publish results should
        // prefer `spawn_task_latest::<T>(...)` so async completion is out-of-order safe.
        let cancel = CancellationToken::new();
        let fut = f(cancel);
        self.spawn_task_latest::<T, _, _>(|_latest, _cancel| async move {
            fut.await;
        })
    }

    /// Spawns an async task for a compute type with enforced out-of-order safety ("latest-only" writes).
    ///
    /// This is the recommended API for tasks that publish results via `Updater::set(...)` /
    /// `Updater::set_state(...)` / `Updater::enqueue_command(...)`.
    ///
    /// The task body receives a `LatestOnlyUpdater` that will:
    /// - allow publishes only if this task's generation is still current for type `T`
    /// - drop stale publishes (and `debug_assert!` in debug builds)
    pub fn spawn_task_latest<T, F, Fut>(&mut self, f: F) -> TaskHandle
    where
        T: 'static,
        F: FnOnce(crate::LatestOnlyUpdater, CancellationToken) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    {
        // Generate unique task ID
        let task_id = self.task_id_generator.next::<T>();
        let cancel_token = CancellationToken::new();
        let handle = TaskHandle::new(task_id, cancel_token.clone());

        trace!(
            "Spawning task (latest-only) for type {} (generation {})",
            std::any::type_name::<T>(),
            task_id.generation()
        );

        // Register handle (this auto-cancels any previous task for this type)
        self.register_task_handle::<T>(handle.clone());

        // Latest-only enforcement (out-of-order safety):
        let type_id = TypeId::of::<T>();

        let gen_cell = self
            .current_generation
            .entry(type_id)
            .or_insert_with(|| Arc::new(AtomicU64::new(0)))
            .clone();

        // Use the TaskId generation as the authoritative generation number.
        gen_cell.store(task_id.generation(), Ordering::Relaxed);

        let gen_cell_for_check = Arc::clone(&gen_cell);
        let current_check: Arc<dyn Fn(TaskId) -> bool + Send + Sync> = Arc::new(move |id| {
            if id.type_id() != type_id {
                return false;
            }
            gen_cell_for_check.load(Ordering::Relaxed) == id.generation()
        });

        let updater = self.updater();
        let latest = updater.latest_only(task_id, current_check);

        let future = f(latest, cancel_token);
        let guarded = async move {
            future.await;
        };

        #[cfg(not(target_arch = "wasm32"))]
        {
            self.task_set.spawn(guarded);
        }

        #[cfg(target_arch = "wasm32")]
        {
            wasm_bindgen_futures::spawn_local(async move {
                guarded.await;
            });
        }

        handle
    }

    /// Cancels a specific task by its handle.
    ///
    /// This triggers the cancellation token for the task, signaling it
    /// to stop at its next cancellation check point.
    ///
    /// Note: This only signals cancellation - the task must cooperatively
    /// check the cancellation token to actually stop. The task will remain
    /// in the `JoinSet` until it completes or is aborted.
    ///
    /// # Arguments
    ///
    /// * `handle` - The `TaskHandle` of the task to cancel.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let handle = ctx.spawn_task::<ApiStatus>(|cancel| async move { ... });
    /// // Later...
    /// ctx.cancel_task(&handle);
    /// ```
    pub fn cancel_task(&self, handle: &TaskHandle) {
        trace!(
            "Cancelling task with id {:?} (generation {})",
            handle.id().type_id(),
            handle.id().generation()
        );
        handle.cancel();
    }

    /// Cancels all tasks and awaits their completion.
    ///
    /// This method implements graceful shutdown by:
    /// 1. Cancelling all active task cancellation tokens
    /// 2. Aborting all tasks in the `JoinSet`
    /// 3. Awaiting all tasks to complete (either normally or via abort)
    ///
    /// This should be called during application shutdown to ensure all
    /// async work is properly cleaned up.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // In application shutdown:
    /// ctx.shutdown().await;
    /// ```
    pub async fn shutdown(&mut self) {
        trace!(
            "Shutting down: cancelling {} active tasks",
            self.active_tasks.len()
        );

        // Cancel all active tasks via their cancellation tokens
        for (type_id, handle) in self.active_tasks.drain() {
            trace!("Cancelling task for type {:?} during shutdown", type_id);
            handle.cancel();
        }

        // Abort all tasks in the JoinSet (native only)
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.task_set.abort_all();

            // Await all tasks to complete
            while self.task_set.join_next().await.is_some() {
                // Tasks are completing (either normally or via abort)
            }
        }

        // On WASM we don't track spawned futures here, so there's nothing to abort/join.
        #[cfg(target_arch = "wasm32")]
        {
            // no-op
        }

        trace!("Shutdown complete");
    }
}
