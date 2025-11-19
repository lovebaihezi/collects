use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt::{Debug, Formatter},
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TopologyError<T>
where
    T: Debug,
{
    #[error("Cycle detected in dependency graph, from ")]
    CycleDetected(DepRoute<T>),
    #[error("Duplicate edge detected in dependency graph, at node {:?}", .0.route[0])]
    DuplicateEdge(DepRoute<T>),
}

pub struct DepRoute<T> {
    // first means the start node, last means the end node
    route: Vec<T>,
}

impl<T> Debug for DepRoute<T>
where
    T: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let len = self.route.len();
        for item in &self.route[..len - 1] {
            write!(f, "{:?} -> ", item)?;
        }
        write!(f, "{:?}", self.route[len - 1])
    }
}

#[derive(Debug, Default)]
pub struct Graph<Node, Edge = ()>
where
    Node: Default + Debug + PartialEq + Copy + Ord,
    Edge: Default + Debug + PartialEq,
{
    routes: Vec<(Node, Edge, Node)>,

    route_cache: BTreeMap<Node, BTreeSet<Node>>,
}

impl<Node, Edge> Graph<Node, Edge>
where
    Node: Default + Debug + PartialEq + Copy + Ord,
    Edge: Default + Debug + PartialEq,
{
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            routes: Vec::with_capacity(capacity),

            route_cache: Default::default(),
        }
    }

    pub fn route_to(&mut self, from: Node, to: Node, via: Edge) {
        self.routes.push((from, via, to));
    }

    fn cal_in_out(&self) -> BTreeMap<Node, (usize, usize)> {
        let mut in_out = BTreeMap::<Node, (usize, usize)>::new();

        for edge in self.routes.iter() {
            let (from, _via, to) = edge;

            let entry_from = in_out.entry(*from).or_insert((0, 0));
            entry_from.1 += 1;

            let entry_to = in_out.entry(*to).or_insert((0, 0));
            entry_to.0 += 1;
        }

        in_out
    }

    pub fn topology_sort(&mut self) -> Result<(), TopologyError<Node>> {
        let mut in_out = self.cal_in_out();

        while !in_out.is_empty() {
            if let Some((&node, _)) = in_out.iter().find(|(_, deg)| deg.0 == 0) {
                // remove node
                in_out.remove(&node);

                // decrease out degree of connected nodes
                // Dynamic Programming to speed up or cache
                for connected in self.direct_connected_nodes(node)? {
                    if let Some(entry) = in_out.get_mut(&connected) {
                        entry.0 -= 1;
                    }
                }
            } else {
                return Err(TopologyError::CycleDetected(DepRoute { route: vec![] }));
            }
        }

        Ok(())
    }

    /// # Connected Nodes, node that deps on the given node
    pub fn connected(&mut self, node: Node) -> impl Iterator<Item = &Node> {
        if self.route_cache.contains_key(&node) {
            self.route_cache.get(&node).unwrap().iter()
        } else {
            let collected = self.connected_nodes(node);
            self.route_cache.insert(node, collected);
            self.route_cache.get(&node).unwrap().iter()
        }
    }

    fn direct_connected_nodes(&self, node: Node) -> Result<BTreeSet<Node>, TopologyError<Node>> {
        let mut collected = Vec::new();

        for (from, _via, to) in self.routes.iter() {
            if from == &node {
                collected.push(*to);
            }
        }

        let collected_nodes_len = collected.len();
        let set: BTreeSet<Node> = collected.into_iter().collect();

        if set.len() != collected_nodes_len {
            // TODO: better error
            Err(TopologyError::DuplicateEdge(DepRoute { route: vec![node] }))
        } else {
            Ok(set)
        }
    }

    fn connected_nodes(&self, node: Node) -> BTreeSet<Node> {
        // Simple BFS
        let mut collected = BTreeSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(node);

        while let Some(current) = queue.pop_front() {
            for (from, _via, to) in self.routes.iter() {
                if from == &current {
                    // Actuall we check for node already collected, which means even if there is cycle, we won't stuck in infinite loop
                    if !collected.contains(to) {
                        collected.insert(*to);
                        queue.push_back(*to);
                    }
                }
            }
        }

        collected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_graph_build() {
        let mut graph: Graph<u32, &str> = Graph::with_capacity(10);
        graph.route_to(1, 2, "edge_1_2");
        graph.route_to(2, 3, "edge_2_3");
        graph.route_to(1, 3, "edge_1_3");

        assert_eq!(graph.routes.len(), 3);
    }

    #[test]
    fn simple_topology_sort() {
        let mut graph: Graph<u32, &str> = Graph::with_capacity(10);
        graph.route_to(1, 2, "edge_1_2");
        graph.route_to(2, 3, "edge_2_3");
        graph.route_to(1, 3, "edge_1_3");

        let result = graph.topology_sort();
        assert!(result.is_ok());
    }

    #[test]
    fn cycle_topology_sort() {
        let mut graph: Graph<u32, &str> = Graph::with_capacity(10);
        graph.route_to(1, 2, "edge_1_2");
        graph.route_to(2, 3, "edge_2_3");
        graph.route_to(3, 1, "edge_3_1");

        let result = graph.topology_sort();
        assert!(result.is_err());
    }
}
