use crate::BasicStates;
use flume::{Receiver, Sender};

#[derive(Debug)]
pub struct StateRuntime {
    send: Sender<BasicStates>,
    recv: Receiver<BasicStates>,
}

impl StateRuntime {
    pub fn new() -> Self {
        let (send, recv) = flume::unbounded();
        Self { send, recv }
    }

    pub fn start_worker(&self) {}

    pub fn run_compute(&self) {}
}
