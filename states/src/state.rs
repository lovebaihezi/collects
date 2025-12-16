use std::{
    any::{Any, TypeId, type_name},
    fmt::Debug,
};

use flume::{Receiver, Sender};

use crate::{Compute, StateRuntime};

/// The `State` trait represents a fundamental unit of state in the application.
///
/// It provides basic identity and initialization logic for state objects.
pub trait State: Any + Debug {
    /// Initializes the state.
    ///
    /// This method is called when the state is first added to the context.
    fn init(&mut self) {}

    /// Returns the name of the state type.
    ///
    /// Defaults to the type name.
    fn name(&self) -> &'static str {
        type_name::<Self>()
    }

    /// Returns the unique type ID of the state.
    fn id(&self) -> TypeId {
        TypeId::of::<Self>()
    }
}

pub struct Updater {
    send: Sender<(TypeId, Box<dyn Any>)>,
}

impl From<&StateRuntime> for Updater {
    fn from(run_time: &StateRuntime) -> Self {
        Self {
            send: run_time.sender(),
        }
    }
}

impl Updater {
    pub fn set<T: Compute + 'static>(&self, state: T) {
        let id = TypeId::of::<T>();
        let boxed: Box<dyn Any> = Box::new(state);
        self.send.send((id, boxed)).unwrap();
    }
}

unsafe impl Send for Updater {}

pub struct Reader {
    recv: Receiver<(TypeId, Box<dyn Any>)>,
}

impl Reader {
    pub fn read<T: Compute + 'static>(&self) -> Option<(TypeId, Box<T>)> {
        if let Ok((reg, boxed)) = self.recv.try_recv()
            && let Ok(state) = boxed.downcast::<T>()
        {
            return Some((reg, state));
        }
        None
    }
}

impl From<&StateRuntime> for Reader {
    fn from(run_time: &StateRuntime) -> Self {
        Self {
            recv: run_time.receiver(),
        }
    }
}

unsafe impl Send for Reader {}
