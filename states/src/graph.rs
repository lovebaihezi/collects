use std::fmt::Debug;

#[derive(Debug, Default)]
pub struct Graph<Node, Edge = ()>
where
    Node: Default + Debug,
    Edge: Default + Debug,
{
    routes: Vec<(Node, Edge, Node)>,
}

impl<Node, Edge> Graph<Node, Edge>
where
    Node: Default + Debug,
    Edge: Default + Debug,
{
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            routes: Vec::with_capacity(capacity),
        }
    }

    pub fn route_to(&mut self, from: Node, to: Node, via: Edge) {
        self.routes.push((from, via, to));
    }
}
