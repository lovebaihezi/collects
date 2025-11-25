use std::any::{Any, TypeId};

use super::{Compute, Reg, State, StateRuntime, StateSyncStatus};

#[derive(Debug, Default)]
pub struct StateCtx {
    runtime: StateRuntime,

    // states(State, Compute)
    storage: Vec<(TypeId, Box<dyn Any>, StateSyncStatus)>,
}

impl StateCtx {
    pub fn new() -> Self {
        let runtime = StateRuntime::new();

        Self {
            runtime,
            storage: Vec::with_capacity(Reg::amount()),
        }
    }

    pub fn add_state<T: State>(&mut self, state: T) {
        let id = T::ID as usize;
        self.storage[id] = (
            state.type_id(),
            Box::new(state),
            StateSyncStatus::BeforeInit,
        );
    }

    pub fn record_compute<T: Compute>(&mut self, compute: T) {
        let id = T::ID as usize;
        self.storage[id] = (
            compute.type_id(),
            Box::new(compute),
            StateSyncStatus::BeforeInit,
        );
        self.runtime.record::<T>();
    }

    pub fn cached<T: State>(&self) -> T {
        T::default()
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
}
