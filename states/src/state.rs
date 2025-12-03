use std::{any::Any, fmt::Debug};

use flume::{Receiver, Sender};

use crate::{Reg, StateRuntime};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentType {
    State,
    Compute,
}

pub trait State: Any + Debug {
    fn init(&mut self) {}
    // TODO: Maybe TypeID could be better
    fn id(&self) -> Reg;
}

pub struct StateUpdater {
    send: Sender<(Reg, Box<dyn Any>)>,
}

impl StateUpdater {
    pub fn from_runtime(runtime: &StateRuntime) -> Self {
        Self {
            send: runtime.sender(),
        }
    }

    pub fn set<T: State>(&self, state: T) {
        let id = state.id();
        let boxed: Box<dyn Any> = Box::new(state);
        self.send.send((id, boxed)).unwrap();
    }
}

unsafe impl Send for StateUpdater {}

pub struct StateReader {
    recv: Receiver<(Reg, Box<dyn Any>)>,
}

impl StateReader {
    pub fn from_runtime(runtime: &StateRuntime) -> Self {
        Self {
            recv: runtime.receiver(),
        }
    }

    pub fn read<T: State>(&self) -> Option<(Reg, Box<T>)> {
        if let Ok((reg, boxed)) = self.recv.try_recv()
            && let Ok(state) = boxed.downcast::<T>()
        {
            return Some((reg, state));
        }
        None
    }
}

unsafe impl Send for StateReader {}
