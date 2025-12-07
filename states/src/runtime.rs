use std::any::{Any, TypeId};

use flume::{Receiver, Sender};

use crate::{Compute, Graph, State, graph::TopologyError};

#[derive(Debug)]
pub struct StateRuntime {
    send: Sender<(TypeId, Box<dyn Any>)>,
    recv: Receiver<(TypeId, Box<dyn Any>)>,

    graph: Graph<TypeId>,
}

impl StateRuntime {
    pub fn new() -> Self {
        let (send, recv) = flume::unbounded();
        Self {
            send,
            recv,
            graph: Graph::new(),
        }
    }

    pub fn sender(&self) -> Sender<(TypeId, Box<dyn Any>)> {
        self.send.clone()
    }

    pub fn receiver(&self) -> Receiver<(TypeId, Box<dyn Any>)> {
        self.recv.clone()
    }

    pub fn record<T: Compute + 'static>(&mut self, compute: &T) {
        let (states, computes) = compute.deps();
        // The Graph
        for dep in states {
            self.graph.route_to(*dep, TypeId::of::<T>(), ());
        }
        for dep in computes {
            self.graph.route_to(*dep, TypeId::of::<T>(), ());
        }
    }

    pub fn verify_deps(&mut self) -> Result<(), TopologyError<TypeId>> {
        self.graph.topology_sort()
    }

    fn should_update_states<T: State>(&self) -> impl Iterator<Item = TypeId> {
        Vec::new().into_iter()
    }
}
