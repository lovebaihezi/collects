use std::any::Any;

use flume::{Receiver, Sender};

use crate::{Compute, Graph, Reg, graph::TopologyError};

#[derive(Debug)]
pub struct StateRuntime {
    send: Sender<(Reg, Box<dyn Any>)>,
    recv: Receiver<(Reg, Box<dyn Any>)>,

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

    pub fn sender(&self) -> Sender<(Reg, Box<dyn Any>)> {
        self.send.clone()
    }

    pub fn receiver(&self) -> Receiver<(Reg, Box<dyn Any>)> {
        self.recv.clone()
    }

    pub fn record<T: Compute>(&mut self, compute: &T) {
        for dep in compute.deps() {
            self.graph.route_to(*dep, compute.id(), ());
        }
    }

    pub fn verify_deps(&mut self) -> Result<(), TopologyError<Reg>> {
        self.graph.topology_sort()
    }

    fn should_update_states(&self, id: Reg) -> impl Iterator<Item = Reg> {
        Vec::new().into_iter()
    }
}
