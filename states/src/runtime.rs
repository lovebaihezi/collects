use std::any::Any;

use flume::{Receiver, Sender};

use crate::{Compute, Graph, Reg, graph::TopologyError};

#[derive(Debug)]
pub struct StateRuntime {
    send: Sender<Box<dyn Any>>,
    recv: Receiver<Box<dyn Any>>,

    graph: Graph<Reg>,
}

impl Default for StateRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl StateRuntime {
    pub fn new() -> Self {
        let (send, recv) = flume::unbounded();
        Self {
            send,
            recv,
            graph: Graph::with_capacity(Reg::amount()),
        }
    }

    pub fn sender(&self) -> Sender<Box<dyn Any>> {
        self.send.clone()
    }

    pub fn receiver(&self) -> Receiver<Box<dyn Any>> {
        self.recv.clone()
    }

    pub fn record<T: Compute>(&mut self) {
        for dep in T::DEPS {
            self.graph.route_to(*dep, T::ID, ());
        }
    }

    pub fn verify_deps(&mut self) -> Result<(), TopologyError<Reg>> {
        self.graph.topology_sort()
    }

    pub fn shuold_update(&self, id: Reg) -> impl Iterator<Item = Reg> {
        Vec::new().into_iter()
    }
}
