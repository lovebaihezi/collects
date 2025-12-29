use std::any::{Any, TypeId};

use flume::{Receiver, Sender};

use crate::{Compute, Graph, graph::TopologyError};

#[derive(Debug)]
pub struct StateRuntime {
    send: Sender<(TypeId, Box<dyn Any>)>,
    recv: Receiver<(TypeId, Box<dyn Any>)>,

    graph: Graph<TypeId>,
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

    /// Returns a mutable reference to the dependency graph.
    /// Used by StateCtx for dirty propagation and dependency-aware compute execution.
    pub fn graph_mut(&mut self) -> &mut Graph<TypeId> {
        &mut self.graph
    }
}
