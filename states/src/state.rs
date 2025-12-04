use std::{
    any::{Any, TypeId, type_name},
    fmt::Debug,
};

use flume::{Receiver, Sender};

use crate::StateRuntime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentType {
    State,
    Compute,
}

impl ComponentType {
    pub fn is_state(&self) -> bool {
        matches!(self, ComponentType::State)
    }

    pub fn is_compute(&self) -> bool {
        matches!(self, ComponentType::Compute)
    }
}

pub trait State: Any + Debug {
    fn init(&mut self) {}

    fn name(&self) -> &'static str {
        type_name::<Self>()
    }

    fn id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

pub struct StateUpdater {
    send: Sender<(TypeId, Box<dyn Any>)>,
}

impl StateUpdater {
    pub fn from_runtime(runtime: &StateRuntime) -> Self {
        Self {
            send: runtime.sender(),
        }
    }

    pub fn set<T: State>(&self, state: T) {
        let id = TypeId::of::<T>();
        let boxed: Box<dyn Any> = Box::new(state);
        self.send.send((id, boxed)).unwrap();
    }
}

unsafe impl Send for StateUpdater {}

pub struct StateReader {
    recv: Receiver<(TypeId, Box<dyn Any>)>,
}

impl StateReader {
    pub fn from_runtime(runtime: &StateRuntime) -> Self {
        Self {
            recv: runtime.receiver(),
        }
    }

    pub fn read<T: State>(&self) -> Option<(TypeId, Box<T>)> {
        if let Ok((reg, boxed)) = self.recv.try_recv()
            && let Ok(state) = boxed.downcast::<T>()
        {
            return Some((reg, state));
        }
        None
    }
}

unsafe impl Send for StateReader {}
