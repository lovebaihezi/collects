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
            write!(f, "{item:?} -> ")?;
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

    /// Cache for forward edges: node -> nodes that depend on it
    dependents_cache: BTreeMap<Node, BTreeSet<Node>>,
    /// Cache for reverse edges: node -> nodes it depends on
    dependencies_cache: BTreeMap<Node, BTreeSet<Node>>,
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
            dependents_cache: BTreeMap::new(),
            dependencies_cache: BTreeMap::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            routes: Vec::with_capacity(capacity),
            dependents_cache: BTreeMap::new(),
            dependencies_cache: BTreeMap::new(),
        }
    }

    pub fn route_to(&mut self, from: Node, to: Node, via: Edge) {
        self.routes.push((from, via, to));
        // Invalidate caches when graph changes
        self.dependents_cache.clear();
        self.dependencies_cache.clear();
    }

    fn cal_in_out(&self) -> BTreeMap<Node, (usize, usize)> {
        let mut in_out = BTreeMap::<Node, (usize, usize)>::new();

        for edge in &self.routes {
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
                for connected in self.direct_dependents(node)? {
                    if let Some(entry) = in_out.get_mut(&connected) {
                        entry.0 -= 1;
                    }
                }
            } else {
                let first = in_out.keys().next().unwrap();
                let route = self.connected(first.clone()).cloned().collect();
                return Err(TopologyError::CycleDetected(DepRoute { route }));
            }
        }

        Ok(())
    }

    // ═══════════════════════════════════════════════════════════════════════
    // FORWARD EDGES: Nodes that DEPEND ON the given node (for dirty propagation)
    // ═══════════════════════════════════════════════════════════════════════

    /// Returns all nodes that directly or transitively depend on the given node.
    /// Used for dirty propagation: when `node` changes, all returned nodes should be marked dirty.
    ///
    /// Example: If A -> B -> C (B depends on A, C depends on B)
    /// `dependents(A)` returns {B, C}
    pub fn dependents(&mut self, node: Node) -> impl Iterator<Item = &Node> {
        if !self.dependents_cache.contains_key(&node) {
            let collected = self.collect_dependents(node);
            self.dependents_cache.insert(node, collected);
        }
        self.dependents_cache
            .get(&node)
            .expect("dependents_cache should contain node after insert")
            .iter()
    }

    /// Returns nodes that directly depend on the given node (one level only)
    fn direct_dependents(&self, node: Node) -> Result<BTreeSet<Node>, TopologyError<Node>> {
        let mut collected = Vec::new();

        for (from, _via, to) in &self.routes {
            if from == &node {
                collected.push(*to);
            }
        }

        let collected_nodes_len = collected.len();
        let set: BTreeSet<Node> = collected.into_iter().collect();

        if set.len() != collected_nodes_len {
            Err(TopologyError::DuplicateEdge(DepRoute { route: vec![node] }))
        } else {
            Ok(set)
        }
    }

    /// Collects all nodes that transitively depend on the given node (BFS)
    fn collect_dependents(&self, node: Node) -> BTreeSet<Node> {
        let mut collected = BTreeSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(node);

        while let Some(current) = queue.pop_front() {
            for (from, _via, to) in &self.routes {
                if from == &current && !collected.contains(to) {
                    collected.insert(*to);
                    queue.push_back(*to);
                }
            }
        }

        collected
    }

    // ═══════════════════════════════════════════════════════════════════════
    // REVERSE EDGES: Nodes that the given node DEPENDS ON (for run_with_deps)
    // ═══════════════════════════════════════════════════════════════════════

    /// Returns all nodes that the given node directly or transitively depends on.
    /// Used for `run::<T>()`: before running T, all returned nodes that are dirty should run first.
    ///
    /// Example: If A -> B -> C (B depends on A, C depends on B)
    /// `dependencies(C)` returns {A, B}
    pub fn dependencies(&mut self, node: Node) -> impl Iterator<Item = &Node> {
        if !self.dependencies_cache.contains_key(&node) {
            let collected = self.collect_dependencies(node);
            self.dependencies_cache.insert(node, collected);
        }
        self.dependencies_cache
            .get(&node)
            .expect("dependencies_cache should contain node after insert")
            .iter()
    }

    /// Collects all nodes that the given node transitively depends on (BFS)
    fn collect_dependencies(&self, node: Node) -> BTreeSet<Node> {
        let mut collected = BTreeSet::new();
        let mut queue = VecDeque::new();

        queue.push_back(node);

        while let Some(current) = queue.pop_front() {
            for (from, _via, to) in &self.routes {
                if to == &current && !collected.contains(from) {
                    collected.insert(*from);
                    queue.push_back(*from);
                }
            }
        }

        collected
    }

    /// Returns dependencies of the given node in topological order (dependencies first).
    /// This is the order in which dirty computes should be executed before running `node`.
    ///
    /// Example: If A -> B -> C and A -> C (C depends on both A and B, B depends on A)
    /// `dependencies_sorted(C)` returns [A, B] (A must run before B)
    pub fn dependencies_sorted(&mut self, node: Node) -> Vec<Node> {
        let deps: BTreeSet<Node> = self.collect_dependencies(node);

        if deps.is_empty() {
            return Vec::new();
        }

        // Build a subgraph of only the dependencies
        // Then topologically sort them
        let mut in_degree: BTreeMap<Node, usize> = deps.iter().map(|&n| (n, 0)).collect();

        // Calculate in-degrees within the dependency subgraph
        for (from, _via, to) in &self.routes {
            if deps.contains(to) && deps.contains(from) {
                *in_degree
                    .get_mut(to)
                    .expect("in_degree should contain all deps") += 1;
            }
            // Also count edges from outside deps into deps (these start with 0 in-degree within subgraph)
        }

        // For nodes that are dependencies but have no other deps in the set, they have 0 in-degree
        // Kahn's algorithm
        let mut result = Vec::with_capacity(deps.len());
        let mut queue: VecDeque<Node> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(&n, _)| n)
            .collect();

        while let Some(current) = queue.pop_front() {
            result.push(current);

            for (from, _via, to) in &self.routes {
                if from == &current && deps.contains(to) {
                    let deg = in_degree
                        .get_mut(to)
                        .expect("in_degree should contain all deps");
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(*to);
                    }
                }
            }
        }

        result
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
    fn test_dependents() {
        // A -> B -> C, A -> C
        let mut graph: Graph<u32> = Graph::new();
        graph.route_to(1, 2, ()); // A -> B
        graph.route_to(2, 3, ()); // B -> C
        graph.route_to(1, 3, ()); // A -> C

        // Dependents of A should be {B, C}
        let deps: BTreeSet<_> = graph.dependents(1).copied().collect();
        assert_eq!(deps, BTreeSet::from([2, 3]));

        // Dependents of B should be {C}
        let deps: BTreeSet<_> = graph.dependents(2).copied().collect();
        assert_eq!(deps, BTreeSet::from([3]));

        // Dependents of C should be empty
        let deps: BTreeSet<_> = graph.dependents(3).copied().collect();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_dependencies() {
        // A -> B -> C, A -> C
        let mut graph: Graph<u32> = Graph::new();
        graph.route_to(1, 2, ()); // A -> B
        graph.route_to(2, 3, ()); // B -> C
        graph.route_to(1, 3, ()); // A -> C

        // Dependencies of C should be {A, B}
        let deps: BTreeSet<_> = graph.dependencies(3).copied().collect();
        assert_eq!(deps, BTreeSet::from([1, 2]));

        // Dependencies of B should be {A}
        let deps: BTreeSet<_> = graph.dependencies(2).copied().collect();
        assert_eq!(deps, BTreeSet::from([1]));

        // Dependencies of A should be empty
        let deps: BTreeSet<_> = graph.dependencies(1).copied().collect();
        assert!(deps.is_empty());
    }

    #[test]
    fn test_dependencies_sorted() {
        // A -> B -> C, A -> C
        let mut graph: Graph<u32> = Graph::new();
        graph.route_to(1, 2, ()); // A -> B
        graph.route_to(2, 3, ()); // B -> C
        graph.route_to(1, 3, ()); // A -> C

        // Sorted dependencies of C should be [A, B] (A before B because B depends on A)
        let sorted = graph.dependencies_sorted(3);
        assert_eq!(sorted, vec![1, 2]);

        // Sorted dependencies of B should be [A]
        let sorted = graph.dependencies_sorted(2);
        assert_eq!(sorted, vec![1]);

        // Sorted dependencies of A should be empty
        let sorted = graph.dependencies_sorted(1);
        assert!(sorted.is_empty());
    }

    #[test]
    fn test_dependencies_sorted_diamond() {
        // Diamond: A -> B, A -> C, B -> D, C -> D
        let mut graph: Graph<u32> = Graph::new();
        graph.route_to(1, 2, ()); // A -> B
        graph.route_to(1, 3, ()); // A -> C
        graph.route_to(2, 4, ()); // B -> D
        graph.route_to(3, 4, ()); // C -> D

        // Sorted dependencies of D should have A first, then B and C in any order
        let sorted = graph.dependencies_sorted(4);
        assert_eq!(sorted.len(), 3);
        assert_eq!(sorted[0], 1); // A must be first
        assert!(sorted.contains(&2)); // B somewhere after A
        assert!(sorted.contains(&3)); // C somewhere after A
    }
}
