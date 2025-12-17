use std::{
    any::TypeId,
    cell::{RefCell, RefMut},
    collections::BTreeMap,
    ptr::NonNull,
};

use log::{Level, info, log_enabled};

use crate::{ComputeStage, Dep, Reader, Updater};

use super::{Compute, Stage, State, StateRuntime};

/// `StateCtx` acts as the central manager for all states and computes.
///
/// It holds the storage for states and computes, manages their lifecycle,
/// and orchestrates the re-computation of derived states when dependencies change.
#[derive(Debug)]
pub struct StateCtx {
    runtime: StateRuntime,

    states: BTreeMap<TypeId, (RefCell<Box<dyn State>>, Stage)>,
    // TODO: We better not store Box, consider using raw pointer to reduce indirection
    // We will not using RefCell with Box, the State should be Sized, and it will not needs to by Any to downcast, we just use NoNullPointer with unsafe
    computes: BTreeMap<TypeId, (RefCell<Box<dyn Compute>>, Stage)>,
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
        Self {
            runtime,
            states,
            computes,
        }
    }

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

    /// Triggers the execution of all dirty computes.
    ///
    /// This iterates through computes marked as dirty or before init, resolves their
    /// dependencies, and executes their `compute` method.
    pub fn run_computed(&mut self) {
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
            let stage = dirty_compute.compute(deps, self.updater());
            if stage == ComputeStage::Pending {
                pending_ids.push(*id);
            }
        }
        if log_enabled!(Level::Info) {
            for name in pending_compute_names {
                info!("Compute pending: {:?}", name);
            }
        }
        // We use Vec to collect, or using RefCell to wrap, or using pointer to avoid borrow checker
        for id in pending_ids {
            self.computes.get_mut(&id).unwrap().1 = Stage::Pending;
        }
    }

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

    /// Synchronizes the computes by processing updates from the runtime.
    ///
    /// This processes any pending updates sent via the `Updater` and applies them
    /// to the respective computes, marking them as clean.
    pub fn sync_computes(&mut self) {
        let cur_len = self.runtime().receiver().len();
        for _ in 0..cur_len {
            if let Ok((id, boxed)) = self.runtime().receiver().try_recv() {
                //debug_assert_eq!(
                //    unsafe { self.storage[id_usize].assume_init_ref() }.2,
                //    StateSyncStatus::Pending
                //);
                let compute = unsafe { self.computes.get_mut(&id).unwrap_unchecked() };
                let computed_name = compute.0.borrow().name();
                info!("Received Compute Update, compute={:?}", computed_name);
                compute.0.borrow_mut().assign_box(boxed);
                self.mark_clean(&id);
            }
        }
    }

    pub fn runtime(&self) -> &StateRuntime {
        &self.runtime
    }

    // TODO: Doc for how state and compute state transforms and how they works
    pub fn dirty_computes(&self) -> impl Iterator<Item = (&TypeId, RefMut<'_, Box<dyn Compute>>)> {
        // TODO(chaibowen): cal from graph with state
        //let dirty_states = self
        //    .storage
        //    .iter()
        //    .enumerate()
        //    .filter_map(|(i, (ct, _, status))| {
        //        if *status == StateSyncStatus::Dirty && ct.is {
        //            Some(TypeId::from_usize(i))
        //        } else {
        //            None
        //        }
        //    });
        self.computes
            .iter()
            .filter_map(|(type_id, (state_cell, compute_state))| {
                if matches!(compute_state, &Stage::Dirty | &Stage::BeforeInit) {
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
    }

    pub fn reader(&self) -> Reader {
        self.runtime().into()
    }

    pub fn updater(&self) -> Updater {
        self.runtime().into()
    }
}
