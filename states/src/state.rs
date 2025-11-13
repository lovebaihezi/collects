use std::any::Any;

use flume::{Receiver, Sender};

use crate::{Reg, StateRuntime};

pub trait State: Any + Default {
    const TYPE: &'static str = "state";
    const ID: Reg;
}

pub struct StateUpdater<T: State> {
    _marker: std::marker::PhantomData<T>,
    send: Sender<Box<dyn Any>>,
}

impl<T> StateUpdater<T>
where
    T: State,
{
    pub fn from_runtime(runtime: &StateRuntime) -> Self {
        Self {
            _marker: std::marker::PhantomData::<T>,
            send: runtime.sender(),
        }
    }

    pub fn set(&self, state: T) {
        let boxed: Box<dyn Any> = Box::new(state);
        self.send.send(boxed).unwrap();
    }
}

unsafe impl<T> Send for StateUpdater<T> where T: State {}

pub struct StateReader<T: State> {
    _marker: std::marker::PhantomData<T>,
    recv: Receiver<Box<dyn Any>>,
}

impl<T> StateReader<T>
where
    T: State,
{
    pub fn from_runtime(runtime: &StateRuntime) -> Self {
        Self {
            _marker: std::marker::PhantomData::<T>,
            recv: runtime.receiver(),
        }
    }

    pub fn read(&self) -> Option<Box<T>> {
        if let Ok(boxed) = self.recv.try_recv()
            && let Ok(state) = boxed.downcast::<T>() {
                return Some(state);
            }
        None
    }
}

unsafe impl<T> Send for StateReader<T> where T: State {}
