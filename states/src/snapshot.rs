use std::any::{Any, TypeId, type_name};
use std::collections::BTreeMap;

use crate::{Compute, State};

#[derive(Default)]
pub struct StateSnapshot {
    inner: BTreeMap<TypeId, Box<dyn Any + Send>>,
}

impl StateSnapshot {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn insert_cloned(&mut self, id: TypeId, value: Box<dyn Any + Send>) {
        self.inner.insert(id, value);
    }

    pub fn get<T: State + Clone + Send + 'static>(&self) -> Option<T> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
            .cloned()
    }
}

#[derive(Default)]
pub struct ComputeSnapshot {
    inner: BTreeMap<TypeId, Box<dyn Any + Send>>,
}

impl ComputeSnapshot {
    pub fn new() -> Self {
        Self {
            inner: BTreeMap::new(),
        }
    }

    pub fn insert_cloned(&mut self, id: TypeId, value: Box<dyn Any + Send>) {
        self.inner.insert(id, value);
    }

    pub fn get<T: Compute + Clone + Send + 'static>(&self) -> Option<T> {
        self.inner
            .get(&TypeId::of::<T>())
            .and_then(|boxed| boxed.downcast_ref::<T>())
            .cloned()
    }
}

#[derive(Default)]
pub struct CommandSnapshot {
    states: StateSnapshot,
    computes: ComputeSnapshot,
}

impl CommandSnapshot {
    pub fn new(states: StateSnapshot, computes: ComputeSnapshot) -> Self {
        Self { states, computes }
    }

    pub fn get_state<T: State + Clone + Send + 'static>(&self) -> T {
        self.states
            .get::<T>()
            .unwrap_or_else(|| panic!("State snapshot for {} is missing", type_name::<T>()))
    }

    pub fn get_compute<T: Compute + Clone + Send + 'static>(&self) -> T {
        self.computes
            .get::<T>()
            .unwrap_or_else(|| panic!("Compute snapshot for {} is missing", type_name::<T>()))
    }
}
