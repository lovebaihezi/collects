use std::any::{Any, TypeId};

use super::{Compute, Reg, State, StateRuntime, StateSyncStatus};

#[derive(Debug, Default)]
pub struct StateCtx {
    runtime: StateRuntime,

    // states(State, Compute)
    storage: Vec<(Box<dyn Any>, StateSyncStatus)>,
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
        let id = state.id() as usize;
        self.storage[id] = (Box::new(state), StateSyncStatus::BeforeInit);
    }

    pub fn record_compute<T: Compute>(&mut self, compute: T) {
        let id = compute.id() as usize;
        self.runtime.record(&compute);
        self.storage[id] = (Box::new(compute), StateSyncStatus::BeforeInit);
    }

    pub fn run_computed(&mut self) {
        let len = self.storage.len();
        for i in 0..len {
            if let Some(compute) = self.storage[i].0.downcast_mut::<Box<dyn Compute>>() {
                // TODO: compute shuold accept [&mut State] or RefCell<Cell<State>> to read other state
                compute.compute(self);
            }
        }
    }

    pub fn cached<T: State>(&self, id: Reg) -> Option<&T> {
        self.storage[id as usize].0.downcast_ref::<T>()
    }

    pub fn runtime(&self) -> &StateRuntime {
        &self.runtime
    }

    pub fn mark_dirty(&mut self, id: Reg) {
        self.storage[id as usize].1 = StateSyncStatus::Dirty;
    }

    pub fn mark_pending(&mut self, id: Reg) {
        self.storage[id as usize].1 = StateSyncStatus::Pending;
    }

    pub fn mark_clean(&mut self, id: Reg) {
        self.storage[id as usize].1 = StateSyncStatus::Clean;
    }

    pub fn clear(&mut self) {
        self.storage.clear();
    }
}
