use std::{any::Any, cell::RefCell, ptr::NonNull};

use crate::{Dep, StateReader, StateUpdater, state::ComponentType};

use super::{Compute, Reg, State, StateRuntime, StateSyncStatus};

#[derive(Debug, Default)]
pub struct StateCtx {
    runtime: StateRuntime,

    // states(State, Compute)
    // TODO: We better not store Box, consider using raw pointer to reduce indirection
    // We will not using RefCell with Box, the State should be Sized, and it will not needs to by Any to downcast, we just use NoNullPointer with unsafe
    storage: Vec<(ComponentType, RefCell<Box<dyn Any>>, StateSyncStatus)>,
}

impl StateCtx {
    pub fn new() -> Self {
        let runtime = StateRuntime::new();
        let storage = Vec::with_capacity(Reg::amount());

        Self { runtime, storage }
    }

    pub fn add_state<T: State>(&mut self, state: T) {
        let id = state.id() as usize;
        self.storage[id] = (
            ComponentType::State,
            RefCell::new(Box::new(state)),
            StateSyncStatus::BeforeInit,
        );
    }

    pub fn record_compute<T: Compute>(&mut self, compute: T) {
        let id = compute.id() as usize;
        self.runtime.record(&compute);
        self.storage[id] = (
            ComponentType::Compute,
            RefCell::new(Box::new(compute)),
            StateSyncStatus::BeforeInit,
        );
    }

    pub fn run_computed(&mut self) {
        let len = self.storage.len();
        // TODO: Only run computes that is dirty, so we needs to run one graph compute first
        for i in 0..len {
            if self.storage[i].0 == ComponentType::Compute {
                let mut borrowed = self.storage[i].1.borrow_mut();
                if let Some(compute) = borrowed.downcast_mut::<Box<dyn Compute>>() {
                    let deps_ids = compute.deps();
                    let deps = Dep::new(
                        deps_ids
                            .iter()
                            .map(|&dep_id| (dep_id, self.get_ref(dep_id))),
                    );
                    compute.compute(deps, self.updater());
                }
            }
        }
    }

    fn get_ref(&self, id: Reg) -> Option<NonNull<dyn Any>> {
        NonNull::new(self.storage[id as usize].1.as_ptr())
    }

    pub fn cached<T: State>(&self, id: Reg) -> Option<&'static T> {
        // TODO: Using address santizer to check if it will leaked or not, asumming it will not
        self.get_ref(id).and_then(|v| unsafe {
            // SAFETY: The lifetime 'static is safe here because the StateCtx owns the data,
            v.as_ref().downcast_ref::<Box<T>>().map(|b| b.as_ref())
        })
    }

    pub fn runtime(&self) -> &StateRuntime {
        &self.runtime
    }

    pub fn mark_dirty(&mut self, id: Reg) {
        self.storage[id as usize].2 = StateSyncStatus::Dirty;
    }

    pub fn mark_pending(&mut self, id: Reg) {
        self.storage[id as usize].2 = StateSyncStatus::Pending;
    }

    pub fn mark_clean(&mut self, id: Reg) {
        self.storage[id as usize].2 = StateSyncStatus::Clean;
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
