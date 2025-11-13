use std::any::Any;

use flume::{Receiver, Sender};

#[derive(Debug)]
pub struct StateRuntime {
    send: Sender<Box<dyn Any>>,
    recv: Receiver<Box<dyn Any>>,
}

impl Default for StateRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl StateRuntime {
    pub fn new() -> Self {
        let (send, recv) = flume::unbounded();
        Self { send, recv }
    }

    pub fn sender(&self) -> Sender<Box<dyn Any>> {
        self.send.clone()
    }

    pub fn receiver(&self) -> Receiver<Box<dyn Any>> {
        self.recv.clone()
    }

    pub fn start_worker(&self) {}

    pub fn run_compute(&self) {}
}
