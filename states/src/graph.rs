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
    #[error("Cycle detected in dependency graph, from {:?}", .0)]
    CycleDetected(DepRoute<T>),
    #[error("Duplicate edge detected in dependency graph, from {:?} to {:?}", .0.route[0], .0.route[1])]
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
        if len == 0 {
            return write!(f, "[]");
        }
        for item in &self.route[..len - 1] {
            write!(f, "{:?} -> ", item)?;
        }
        write!(f, "{:?}", self.route[len - 1])
    }
}

#[derive(Debug)]
pub struct Graph<Node, Edge = ()>
where
    Node: Debug + PartialEq + Copy + Ord,
    Edge: Debug + PartialEq,
{
    routes: Vec<(Node, Edge, Node)>,

    route_cache: BTreeMap<Node, BTreeSet<Node>>,
}

impl<Node, Edge> Default for Graph<Node, Edge>
where
    Node: Debug + PartialEq + Copy + Ord,
    Edge: Debug + PartialEq,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<Node, Edge> Graph<Node, Edge>
where
    Node: Debug + PartialEq + Copy + Ord,
    Edge: Debug + PartialEq,
{
    pub fn new() -> Self {
        Self {
            routes: Vec::new(),

            route_cache: BTreeMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            routes: Vec::with_capacity(capacity),

            route_cache: BTreeMap::new(),
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
                let keys: Vec<Node> = in_out.keys().cloned().collect();
                if let Some(cycle) = self.find_cycle(&keys) {
                    return Err(TopologyError::CycleDetected(DepRoute { route: cycle }));
                }
                // Should not happen if logic is correct, but fallback
                return Err(TopologyError::CycleDetected(DepRoute { route: vec![] }));
            }
        }

        Ok(())
    }

    fn find_cycle(&self, nodes: &[Node]) -> Option<Vec<Node>> {
        // Iterative DFS to find cycle among the remaining nodes
        let mut visited = BTreeSet::new();
        // Set of nodes currently in the recursion stack (path)
        let mut path_set = BTreeSet::new();
        // The path itself, to reconstruct the cycle
        let mut path = Vec::new();

        // Stack for DFS: stores (node, neighbors_iterator)
        // Using Box<dyn Iterator> to handle the BTreeSet iterator type
        let mut stack: Vec<(Node, std::vec::IntoIter<Node>)> = Vec::new();

        for &start_node in nodes {
            if visited.contains(&start_node) {
                continue;
            }

            // Start DFS from start_node
            // Neighbors are collected into a Vec to manage the iterator easily
            let neighbors = self
                .direct_connected_nodes(start_node)
                .unwrap_or_default()
                .into_iter()
                .filter(|n| nodes.contains(n))
                .collect::<Vec<_>>()
                .into_iter();

            stack.push((start_node, neighbors));
            visited.insert(start_node);
            path_set.insert(start_node);
            path.push(start_node);

            while let Some((current_node, neighbors)) = stack.last_mut() {
                if let Some(neighbor) = neighbors.next() {
                    if path_set.contains(&neighbor) {
                        // Cycle found
                        // Extract the cycle from path
                        if let Some(pos) = path.iter().position(|&x| x == neighbor) {
                            let mut cycle = path[pos..].to_vec();
                            cycle.push(neighbor);
                            return Some(cycle);
                        }
                    } else if !visited.contains(&neighbor) {
                        // Visit new node
                        let next_neighbors = self
                            .direct_connected_nodes(neighbor)
                            .unwrap_or_default()
                            .into_iter()
                            .filter(|n| nodes.contains(n))
                            .collect::<Vec<_>>()
                            .into_iter();

                        visited.insert(neighbor);
                        path_set.insert(neighbor);
                        path.push(neighbor);
                        stack.push((neighbor, next_neighbors));
                    }
                } else {
                    // Backtrack
                    // Need to drop the borrow of stack first
                    let node_to_remove = *current_node;
                    stack.pop();
                    path_set.remove(&node_to_remove);
                    path.pop();
                }
            }
        }
        None
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
        let mut collected = BTreeSet::new();

        for (from, _via, to) in self.routes.iter() {
            if from == &node {
                if collected.contains(to) {
                    return Err(TopologyError::DuplicateEdge(DepRoute {
                        route: vec![node, *to],
                    }));
                }
                collected.insert(*to);
            }
        }

        Ok(collected)
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

    #[test]
    fn duplicate_edge_detection_error_msg() {
        let mut graph: Graph<u32, &str> = Graph::with_capacity(10);
        graph.route_to(1, 2, "edge_1_2");
        graph.route_to(1, 2, "edge_1_2_dup");

        let result = graph.topology_sort();
        match result {
             Err(TopologyError::DuplicateEdge(dep_route)) => {
                 let debug_str = format!("{:?}", dep_route);
                 // Should show "1 -> 2"
                 assert!(debug_str.contains("1 -> 2"));

                 let err = TopologyError::DuplicateEdge(dep_route);
                 let err_str = format!("{}", err);
                 assert!(err_str.contains("Duplicate edge detected"));
                 assert!(err_str.contains("from 1 to 2"));
             }
             _ => panic!("Expected DuplicateEdge error"),
        }
    }

    #[test]
    fn cycle_detection_error_msg() {
        let mut graph: Graph<u32, &str> = Graph::with_capacity(10);
        // Create a cycle: 1 -> 2 -> 3 -> 1
        graph.route_to(1, 2, "edge_1_2");
        graph.route_to(2, 3, "edge_2_3");
        graph.route_to(3, 1, "edge_3_1");

        let result = graph.topology_sort();
        match result {
             Err(TopologyError::CycleDetected(dep_route)) => {
                 let debug_str = format!("{:?}", dep_route);
                 // We expect "1 -> 2 -> 3 -> 1" or a rotation of it, but it must be a closed loop
                 assert!(debug_str.len() > 0);

                 let err = TopologyError::CycleDetected(dep_route);
                 let err_str = format!("{}", err);
                 assert!(err_str.contains("Cycle detected"));
                 // Check that it contains the nodes involved
                 assert!(err_str.contains("1"));
                 assert!(err_str.contains("2"));
                 assert!(err_str.contains("3"));
                 assert!(err_str.contains("->"));
             }
             _ => panic!("Expected CycleDetected error"),
        }
    }
}
