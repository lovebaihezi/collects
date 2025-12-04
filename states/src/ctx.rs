use std::{
    any::{Any, TypeId},
    cell::{RefCell, RefMut},
    collections::BTreeMap,
    ptr::NonNull,
};

use log::debug;

use crate::{Dep, StateReader, StateUpdater, state::ComponentType};

use super::{Compute, State, StateRuntime, StateSyncStatus};

#[derive(Debug)]
pub struct StateCtx {
    runtime: StateRuntime,

    // states(State, Compute)
    // TODO: We better not store Box, consider using raw pointer to reduce indirection
    // We will not using RefCell with Box, the State should be Sized, and it will not needs to by Any to downcast, we just use NoNullPointer with unsafe
    storage: BTreeMap<TypeId, (ComponentType, RefCell<Box<dyn Any>>, StateSyncStatus)>,
}

impl StateCtx {
    pub fn new() -> Self {
        let runtime = StateRuntime::new();
        let storage = BTreeMap::new();
        Self { runtime, storage }
    }

    pub fn add_state<T: State>(&mut self, state: T) {
        let id = TypeId::of::<T>();
        debug!("Record State: id={:?}, state={:?}", id, state);
        self.storage.insert(
            id,
            (
                ComponentType::State,
                RefCell::new(Box::new(state)),
                StateSyncStatus::BeforeInit,
            ),
        );
    }

    pub fn record_compute<T: Compute>(&mut self, compute: T) {
        let id = TypeId::of::<T>();
        debug!("Record Compute: id={:?}, compute={:?}", id, compute);
        self.runtime.record(&compute);
        self.storage.insert(
            id,
            (
                ComponentType::Compute,
                RefCell::new(Box::new(compute)),
                StateSyncStatus::BeforeInit,
            ),
        );
    }

    pub fn run_computed(&mut self) {
        let dirty_computes = self.dirty_computes();
        for mut dirty_compute in dirty_computes {
            let compute = dirty_compute.downcast_mut::<Box<dyn Compute>>().unwrap();
            let deps_ids = compute.deps();
            let deps = Dep::new(
                deps_ids
                    .iter()
                    .map(|&dep_id| (dep_id, self.get_ref(&dep_id))),
            );
            compute.compute(deps, self.updater());
        }
    }

    fn get_ref_mut(&self, id: &TypeId) -> RefMut<'_, Box<dyn Any + 'static>> {
        self.storage[id].1.borrow_mut()
    }

    fn get_ref(&self, id: &TypeId) -> Option<NonNull<dyn Any>> {
        NonNull::new(self.storage[id].1.as_ptr())
    }

    pub fn cached<T: State>(&self) -> Option<&'static T> {
        // TODO: Using address santizer to check if it will leaked or not, asumming it will not
        self.get_ref(&TypeId::of::<T>()).and_then(|v| unsafe {
            // SAFETY: The lifetime 'static is safe here because the StateCtx owns the data,
            v.as_ref().downcast_ref::<Box<T>>().map(|b| b.as_ref())
        })
    }

    pub fn sync_computes(&mut self) {
        let cur_len = self.runtime().receiver().len();
        for _ in 0..cur_len {
            if let Ok((id, boxed)) = self.runtime().receiver().try_recv() {
                //debug_assert_eq!(
                //    unsafe { self.storage[id_usize].assume_init_ref() }.2,
                //    StateSyncStatus::Pending
                //);
                unsafe { self.storage.get_mut(&id).unwrap_unchecked() }
                    .1
                    .replace(boxed);
                self.mark_clean(&id);
            }
        }
    }

    pub fn runtime(&self) -> &StateRuntime {
        &self.runtime
    }

    // TODO: Doc for how state and compute state transforms and how they works
    pub fn dirty_computes(&self) -> impl Iterator<Item = RefMut<'_, Box<dyn Any>>> {
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
        self.storage.values().filter_map(|(ct, state_cell, _)| {
            // Only Pending one needs to be computed
            if ct.is_compute() {
                Some(state_cell.borrow_mut())
            } else {
                None
            }
        })
    }

    pub fn mark_dirty_t<T: Compute>(&mut self) {
        unsafe { self.storage.get_mut(&TypeId::of::<T>()).unwrap_unchecked() }.2 =
            StateSyncStatus::Dirty;
    }

    pub fn mark_pending_t<T: Compute>(&mut self) {
        unsafe { self.storage.get_mut(&TypeId::of::<T>()).unwrap_unchecked() }.2 =
            StateSyncStatus::Pending;
    }

    pub fn mark_clean_t<T: Compute>(&mut self) {
        unsafe { self.storage.get_mut(&TypeId::of::<T>()).unwrap_unchecked() }.2 =
            StateSyncStatus::Clean;
    }

    pub fn mark_dirty(&mut self, id: &TypeId) {
        unsafe { self.storage.get_mut(id).unwrap_unchecked() }.2 = StateSyncStatus::Dirty;
    }

    pub fn mark_pending(&mut self, id: &TypeId) {
        unsafe { self.storage.get_mut(id).unwrap_unchecked() }.2 = StateSyncStatus::Pending;
    }

    pub fn mark_clean(&mut self, id: &TypeId) {
        unsafe { self.storage.get_mut(id).unwrap_unchecked() }.2 = StateSyncStatus::Clean;
    }

    pub fn clear(&mut self) {
        self.storage.clear();
    }

    pub fn reader(&self) -> StateReader {
        StateReader::from_runtime(self.runtime())
    }

    pub fn updater(&self) -> StateUpdater {
        crate::StateUpdater::from_runtime(self.runtime())
    }
}
