use std::collections::{HashMap, HashSet};

pub type NodeId = usize;

#[derive(Debug)]
struct Node {
    pub incoming: HashSet<NodeId>,
    pub outgoing: HashSet<NodeId>,
}

#[derive(Debug)]
pub struct Dominators {
    dominators: HashMap<NodeId, HashSet<NodeId>>
}

impl Dominators {
    pub fn dominates(&self, a: NodeId, b: NodeId) -> bool {
        self.dominators.get(&b).expect("Invalid src node").contains(&a)
    }

    pub fn implies_backwards_edge(&self, src: NodeId, dst: NodeId) -> bool {
        self.dominators.get(&src).expect("Invalid src node").contains(&dst)
    }
}

#[derive(Debug)]
pub struct ControlFlowGraph {
    entry: Option<NodeId>,
    nodes: HashMap<NodeId, Node>,
}

impl ControlFlowGraph {
    pub fn new() -> ControlFlowGraph {
        ControlFlowGraph {
            entry: None,
            nodes: HashMap::new(),
        }
    }

    pub fn set_entry(&mut self, entry: NodeId) {
        self.entry = Some(entry);
    }

    pub fn get_entry(&self) -> Option<NodeId> {
        self.entry
    }

    pub fn add_node(&mut self, idx: NodeId) {
        assert!(self.nodes.insert(idx, Node {
            incoming: HashSet::new(),
            outgoing: HashSet::new(),
        }).is_none());
    }

    pub fn remove_node(&mut self, idx: NodeId) {
        for incoming in self.incoming_for(idx).clone() {
            self.nodes.get_mut(&incoming).unwrap().outgoing.remove(&idx);
        }
        for outgoing in self.outgoing_for(idx).clone() {
            self.nodes.get_mut(&outgoing).unwrap().incoming.remove(&idx);
        }
        self.nodes.remove(&idx);
    }

    pub fn remove_node_edges(&mut self, idx: NodeId) {
        for outgoing in self.outgoing_for(idx).clone() {
            self.nodes.get_mut(&outgoing).unwrap().incoming.remove(&idx);
            self.nodes.get_mut(&idx).unwrap().outgoing.remove(&outgoing);
        }
        for incoming in self.incoming_for(idx).clone() {
            self.nodes.get_mut(&incoming).unwrap().outgoing.remove(&idx);
            self.nodes.get_mut(&idx).unwrap().incoming.remove(&incoming);
        }
    }

    pub fn add_edge(&mut self, src: NodeId, dest: NodeId) {
        self.nodes.get_mut(&src).expect("Not a valid src node").outgoing.insert(dest);
        self.nodes.get_mut(&dest).expect("Not a valid dest node").incoming.insert(src);
    }

    pub fn outgoing_for(&self, node: NodeId) -> &HashSet<NodeId> {
        &self.nodes.get(&node).unwrap().outgoing
    }

    pub fn incoming_for(&self, node: NodeId) -> &HashSet<NodeId> {
        &self.nodes.get(&node).unwrap().incoming
    }

    pub fn remove_edge(&mut self, src: NodeId, dst: NodeId) {
        assert!(self.nodes.get_mut(&src).unwrap().outgoing.remove(&dst));
        assert!(self.nodes.get_mut(&dst).unwrap().incoming.remove(&src));
    }

    pub fn nodes(&self) -> HashSet<NodeId> {
        let mut nodes = HashSet::new();
        for node in self.nodes.keys() {
            nodes.insert(*node);
        }
        nodes
    }

    pub fn dominators(&self) -> Dominators {
        let entry = self.entry.expect("No entry node");
        let mut dominators = HashMap::new();

        dominators.insert(entry, {
            let mut set = HashSet::new();
            set.insert(entry);
            set
        });

        let nodes = self.nodes();
        for node in &nodes {
            if *node == entry {
                continue;
            }
            dominators.insert(*node, nodes.clone());
        }

        let mut did_change = true;
        while did_change {
            did_change = false;

            for node in &nodes {
                if *node == entry {
                    continue;
                }

                let mut new_dominators = HashSet::new();
                let mut is_first = true;
                for other_node in self.incoming_for(*node) {
                    if is_first {
                        new_dominators.extend(&dominators[other_node]);
                        is_first = false;
                    } else {
                        new_dominators = new_dominators.intersection(&dominators[other_node]).copied().collect();
                    }
                }

                new_dominators.insert(*node);

                if &new_dominators != dominators.get(node).unwrap() {
                    dominators.insert(*node, new_dominators);
                    did_change = true;
                }
            }
        }

        Dominators { dominators }
    }

    pub fn consistency_check(&self) {
        for node in self.nodes() {
            for out in self.outgoing_for(node) {
                assert!(self.nodes.get(out).expect("Invalid outgoing node").incoming.contains(&node));
            }

            for inc in self.incoming_for(node) {
                assert!(self.nodes.get(inc).expect("Invalid incoming node").outgoing.contains(&node));
            }
        }
    }

    pub fn to_dot<F>(&self, name: F) -> String where F: Fn(NodeId) -> String {
        use std::fmt::Write;

        let mut dot = String::new();

        dot.push_str("digraph G {");

        if let Some(start) = self.entry {
            write!(dot, "start -> {};", name(start)).unwrap();
        }

        for node_id in self.nodes.keys() {
            for dest in self.outgoing_for(*node_id) {
                write!(dot, "{} -> {};", name(*node_id), name(*dest)).unwrap();
            }
        }

        dot.push_str("}");

        dot
    }

    pub fn trim_unreachable(&mut self) {
        for node in self.nodes() {
            if self.entry != Some(node) && self.incoming_for(node).is_empty() {
                self.remove_node_edges(node);
            }
        }
    }
}
