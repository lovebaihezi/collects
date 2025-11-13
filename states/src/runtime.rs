use std::any::Any;

use flume::{Receiver, Sender};
use thiserror::Error;

use crate::{Compute, Reg};

#[derive(Debug)]
pub struct StateRuntime {
    send: Sender<Box<dyn Any>>,
    recv: Receiver<Box<dyn Any>>,

    graph: Graph<Reg, Reg>,
}

impl Default for StateRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Error)]
pub enum DepsConflict {}

impl StateRuntime {
    pub fn new() -> Self {
        let (send, recv) = flume::unbounded();
        Self {
            send,
            recv,
            graph: Vec::with_capacity(Reg::amount()),
        }
    }

    pub fn sender(&self) -> Sender<Box<dyn Any>> {
        self.send.clone()
    }

    pub fn receiver(&self) -> Receiver<Box<dyn Any>> {
        self.recv.clone()
    }

    pub fn record<T: Compute>(&mut self) -> Result<(), String> {}
}
