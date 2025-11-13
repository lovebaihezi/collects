use std::any::Any;

use crate::{StateReader, StateRuntime, StateUpdater};

use super::{BasicStates, Reg, State, StateSyncStatus};

#[derive(Debug, Default)]
pub struct StateCtx {
    runtime: StateRuntime,

    // simple state tracking
    state_status: [StateSyncStatus; Reg::amount()],
    storage: Vec<Box<dyn Any>>,
}

impl StateCtx {
    pub fn new() -> Self {
        let runtime = StateRuntime::new();
        let status = [StateSyncStatus::Init; Reg::amount()];

        Self {
            runtime,
            state_status: status,
            storage: Vec::with_capacity(Reg::amount()),
        }
    }

    pub fn cached<T: State>(&self) -> T {
        T::default()
    }

    pub fn runtime(&self) -> &StateRuntime {
        &self.runtime
    }

    pub fn mark_dirty(&mut self, id: Reg) {
        self.state_status[id as usize] = StateSyncStatus::Dirty;
    }

    pub fn mark_pending(&mut self, id: Reg) {
        self.state_status[id as usize] = StateSyncStatus::Pending;
    }

    pub fn mark_clean(&mut self, id: Reg) {
        self.state_status[id as usize] = StateSyncStatus::Clean;
    }

    pub fn clear(&mut self) {
        self.storage.clear();
    }
}
