use std::{
    any::TypeId,
    cell::{RefCell, RefMut},
    collections::BTreeMap,
    ptr::NonNull,
};

use log::debug;

use crate::{Dep, Reader, Updater};

use super::{Compute, State, StateRuntime, StateSyncStatus};

#[derive(Debug)]
pub struct StateCtx {
    runtime: StateRuntime,

    states: BTreeMap<TypeId, (RefCell<Box<dyn State>>, StateSyncStatus)>,
    // TODO: We better not store Box, consider using raw pointer to reduce indirection
    // We will not using RefCell with Box, the State should be Sized, and it will not needs to by Any to downcast, we just use NoNullPointer with unsafe
    computes: BTreeMap<TypeId, (RefCell<Box<dyn Compute>>, StateSyncStatus)>,
}

impl Default for StateCtx {
    fn default() -> Self {
        Self::new()
    }
}

impl StateCtx {
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

    pub fn add_state<T: State>(&mut self, state: T) {
        let id = TypeId::of::<T>();
        debug!("Record State: id={:?}, state={:?}", id, state);
        self.states.insert(
            id,
            (RefCell::new(Box::new(state)), StateSyncStatus::BeforeInit),
        );
    }

    pub fn record_compute<T: State + Compute>(&mut self, compute: T) {
        let id = TypeId::of::<T>();
        debug!("Record Compute: id={:?}, compute={:?}", id, compute);
        self.runtime.record(&compute);
        self.computes.insert(
            id,
            (RefCell::new(Box::new(compute)), StateSyncStatus::BeforeInit),
        );
    }

    pub fn run_computed(&mut self) {
        let dirty_computes = self.dirty_computes();
        for dirty_compute in dirty_computes {
            let (states, computes) = dirty_compute.deps();
            let deps = Dep::new(
                states
                    .into_iter()
                    .map(|&dep_id| (dep_id, self.get_state_ptr(&dep_id))),
                computes
                    .into_iter()
                    .map(|&dep_id| (dep_id, self.get_compute_ptr(&dep_id))),
            );
            dirty_compute.compute(deps, self.updater());
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

    pub fn cached<T: Compute + Sized>(&self) -> Option<&'static T> {
        unsafe {
            self.computes[&TypeId::of::<T>()]
                .0
                .as_ptr()
                .as_mut()
                .and_then(|v| v.as_any().downcast_ref())
        }
    }

    pub fn sync_computes(&mut self) {
        let cur_len = self.runtime().receiver().len();
        for _ in 0..cur_len {
            if let Ok((id, boxed)) = self.runtime().receiver().try_recv() {
                //debug_assert_eq!(
                //    unsafe { self.storage[id_usize].assume_init_ref() }.2,
                //    StateSyncStatus::Pending
                //);
                unsafe { self.computes.get_mut(&id).unwrap_unchecked() }
                    .0
                    .borrow_mut()
                    .assign_box(boxed);
                self.mark_clean(&id);
            }
        }
    }

    pub fn runtime(&self) -> &StateRuntime {
        &self.runtime
    }

    // TODO: Doc for how state and compute state transforms and how they works
    pub fn dirty_computes(&self) -> impl Iterator<Item = RefMut<'_, Box<dyn Compute>>> {
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
        self.computes.values().map(|(state_cell, _)| {
            // Only Pending one needs to be computed
            state_cell.borrow_mut()
        })
    }

    fn get_mut_ref(
        &mut self,
        id: &TypeId,
    ) -> (
        Option<&mut (RefCell<Box<dyn State + 'static>>, StateSyncStatus)>,
        Option<&mut (RefCell<Box<dyn Compute + 'static>>, StateSyncStatus)>,
    ) {
        let state_entry = self.states.get_mut(id);
        let compute_entry = self.computes.get_mut(id);
        (state_entry, compute_entry)
    }

    fn mark_as(&mut self, id: &TypeId, tobe: StateSyncStatus) {
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

    pub fn mark_dirty(&mut self, id: &TypeId) {
        self.mark_as(id, StateSyncStatus::Dirty);
    }

    pub fn mark_pending(&mut self, id: &TypeId) {
        self.mark_as(id, StateSyncStatus::Pending);
    }

    pub fn mark_clean(&mut self, id: &TypeId) {
        self.mark_as(id, StateSyncStatus::Clean);
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
