use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fmt::{Debug, Formatter},
    mem::MaybeUninit,
};

use thiserror::Error;

#[derive(Debug, Error)]
pub enum TopologyError {
    #[error("Cycle detected in dependency graph, from ")]
    CycleDetected(),
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

    pub fn topology_sort(&mut self) -> Result<(), TopologyError> {
        let mut in_out = self.cal_in_out();

        let mut queue = VecDeque::new();

        for (node, &(in_count, _)) in in_out.iter() {
            if in_count == 0 {
                queue.push_back(*node);
            }
        }

        while let Some(current) = queue.pop_front() {
            let connected_nodes = self.connected(current);
            for connected_node in connected_nodes {
                let (in_c, _) = in_out.get_mut(connected_node).unwrap();
                *in_c -= 1;
                if *in_c == 0 {
                    queue.push_back(*connected_node);
                }
            }
        }

        Ok(())
    }

    pub fn connected(&mut self, node: Node) -> impl Iterator<Item = &Node> {
        if self.route_cache.contains_key(&node) {
            self.route_cache.get(&node).unwrap().into_iter()
        } else {
            let collected = self.connected_nodes(node);
            self.route_cache.insert(node, collected);
            self.route_cache.get(&node).unwrap().into_iter()
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
}
