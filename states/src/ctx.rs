use std::{
    any::TypeId,
    cell::{RefCell, RefMut},
    collections::BTreeMap,
    ptr::NonNull,
};

use log::{Level, info, log_enabled, trace};

use crate::{Command, Dep, Reader, Updater};

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
    /// `dispatch::<T>()`.
    commands: BTreeMap<TypeId, RefCell<Box<dyn Command>>>,
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
        Self {
            runtime,
            states,
            computes,
            commands,
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
        info!("Record State: id={:?}, state={:?}", id, state);
        self.states
            .insert(id, (RefCell::new(Box::new(state)), Stage::BeforeInit));
    }

    /// Registers a `Compute` (derived state) to the context.
    ///
    /// The compute is recorded in the runtime and initialized.
    pub fn record_compute<T: Compute>(&mut self, compute: T) {
        let id = TypeId::of::<T>();
        info!("Record Compute: id={:?}, compute={:?}", id, compute);
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
        info!("Record Command: id={:?}, command={:?}", id, command);
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

    /// Directly mutates a compute value without going through the async Updater.
    ///
    /// Use this only for UI state changes that don't affect compute dependencies
    /// or when immediate synchronous updates are required (e.g., toggle states).
    ///
    /// Note: This does NOT mark the compute as dirty or trigger re-computation.
    /// For changes that should trigger dependency updates, use the normal
    /// `Updater::set()` mechanism within `Compute::compute()`.
    pub fn update_compute<T: Compute>(&self, f: impl FnOnce(&mut T)) {
        let id = TypeId::of::<T>();
        let compute = self.get_compute_mut(&id);
        f(compute.as_any_mut().downcast_mut::<T>().unwrap());
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

    /// Dispatches a manual-only command by its type.
    ///
    /// The command is executed immediately and synchronously in the caller's thread.
    /// Any async work should be spawned inside the command implementation (e.g. using tokio),
    /// and results should flow back through your chosen state update mechanism.
    pub fn dispatch<T: Command + 'static>(&mut self) {
        let id = TypeId::of::<T>();
        let Some(cell) = self.commands.get(&id) else {
            panic!("No command found for id: {:?}", id);
        };

        // Build deps from currently registered states + computes (commands may read both).
        //
        // Commands are intentionally not part of the dependency graph, so we construct
        // the dependency access from what's available in this context at dispatch time.
        let state_ids: Vec<TypeId> = self.states.keys().copied().collect();
        let compute_ids: Vec<TypeId> = self.computes.keys().copied().collect();

        let deps = Dep::new(
            state_ids
                .into_iter()
                .map(|dep_id| (dep_id, self.get_state_ptr(&dep_id))),
            compute_ids
                .into_iter()
                .map(|dep_id| (dep_id, self.get_compute_ptr(&dep_id))),
        );

        let borrowed = cell.borrow();
        borrowed.run(deps, self.updater());
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

        info!("Run compute: {:?}", borrowed.name());
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
            info!("Run compute: {:?}", dirty_compute.name());
            if log_enabled!(Level::Info) {
                pending_compute_names.push(dirty_compute.name());
            }
            pending_ids.push(*id);
            dirty_compute.compute(deps, self.updater());
        }
        for id in pending_ids {
            self.mark_pending(&id);
        }
        if log_enabled!(Level::Info) {
            for name in pending_compute_names {
                info!("Compute pending: {:?}", name);
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

    /// Returns a mutable reference to a state.
    ///
    /// **Warning**: Prefer using `update::<T>()` instead, as it automatically
    /// propagates dirty status to dependent computes.
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

    /// Synchronizes the computes by processing updates from the runtime.
    ///
    /// This processes any pending updates sent via the `Updater` and applies them
    /// to the respective computes, marking them as clean.
    pub fn sync_computes(&mut self) {
        let cur_len = self.runtime().receiver().len();
        trace!(
            "Start Sync Compute State, Cur Received {:?} Compute Result",
            cur_len
        );
        for _ in 0..cur_len {
            if let Ok((id, boxed)) = self.runtime().receiver().try_recv() {
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
}
