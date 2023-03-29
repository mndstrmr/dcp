use std::collections::{HashMap, HashSet};

pub type NodeId = usize;

pub struct Node {
    pub incoming: HashSet<NodeId>,
    pub outgoing: HashSet<NodeId>,
}

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

pub struct ControlFlowGraph {
    entry: Option<NodeId>,
    nodes: HashMap<NodeId, Node>,
    next_id: usize,
}

impl ControlFlowGraph {
    pub fn new() -> ControlFlowGraph {
        ControlFlowGraph {
            entry: None,
            nodes: HashMap::new(),
            next_id: 0,
        }
    }

    pub fn set_entry(&mut self, entry: NodeId) {
        self.entry = Some(entry);
    }

    pub fn get_entry(&self) -> Option<NodeId> {
        self.entry
    }

    pub fn add_node(&mut self) -> NodeId {
        self.nodes.insert(self.next_id, Node {
            incoming: HashSet::new(),
            outgoing: HashSet::new(),
        });
        self.next_id += 1;

        self.next_id - 1
    }

    pub fn nodes_mut(&mut self) -> &mut HashMap<NodeId, Node> {
        &mut self.nodes
    }

    pub fn add_edge(&mut self, src: NodeId, dest: NodeId) {
        self.nodes.get_mut(&src).expect("Not a valid src node").outgoing.insert(dest);
        self.nodes.get_mut(&dest).expect("Not a valid dest node").incoming.insert(src);
    }

    pub fn has_node(&self, node: NodeId) -> bool {
        self.nodes.contains_key(&node)
    }

    pub fn outgoing_for(&self, node: NodeId) -> &HashSet<NodeId> {
        &self.nodes.get(&node).unwrap().outgoing
    }

    pub fn outgoing_for_mut(&mut self, node: NodeId) -> &mut HashSet<NodeId> {
        &mut self.nodes.get_mut(&node).unwrap().outgoing
    }

    pub fn incoming_for(&self, node: NodeId) -> &HashSet<NodeId> {
        &self.nodes.get(&node).unwrap().incoming
    }

    pub fn incoming_for_mut(&mut self, node: NodeId) -> &mut HashSet<NodeId> {
        &mut self.nodes.get_mut(&node).unwrap().incoming
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

    fn full_dfs(&self, node: NodeId, parent: Option<NodeId>, time: &mut usize, results: &mut HashMap<NodeId, (usize, usize, Option<NodeId>)>, dominators: &Dominators) {
        let discover = *time;
        *time += 1;

        // Mark node as already visited
        results.insert(node, (0, 0, None));

        for neighbour in self.outgoing_for(node) {
            if !dominators.implies_backwards_edge(node, *neighbour) && !results.contains_key(&neighbour) {
                self.full_dfs(*neighbour, Some(node), time, results, dominators);
            }
        }

        let finish = *time;
        *time += 1;

        results.insert(node, (discover, finish, parent));
    }

    pub fn order_nodes_topo_bubble(&self, dominators: &Dominators) -> Vec<NodeId> {
        let mut dfs = HashMap::new();
        let mut time = 0;
        self.full_dfs(self.entry.unwrap(), None, &mut time, &mut dfs, dominators);

        let mut nodes = self.nodes().into_iter().collect::<Vec<_>>();
        nodes.sort_by_key(|n: &NodeId| -(dfs.get(n).expect("Node not found in tree").1 as isize));

        let mut indices = HashMap::new();
        for (n, node) in nodes.iter().enumerate() {
            indices.insert(*node, n);
        }

        // Bubble nodes
        let mut changed = true;
        while changed {
            changed = false;

            for i in 1..nodes.len() {
                if nodes[i] > nodes[i - 1] {
                    continue;
                }

                let mut can_move_back = true;
                for neighbour in self.incoming_for(nodes[i]) {
                    if *indices.get(neighbour).unwrap() >= i - 1 {
                        can_move_back = false;
                        break;
                    }
                }

                if can_move_back {
                    nodes.swap(i - 1, i);
                    indices.insert(nodes[i], i);
                    indices.insert(nodes[i - 1], i - 1);
                    changed = true;
                }
            }
        }

        nodes
    }

    pub fn trim_unreachable(&mut self) {
        let mut changed = true;

        while changed {
            changed = false;
            for node in self.nodes() {
                if Some(node) != self.entry && self.incoming_for(node).is_empty() {
                    for out in self.outgoing_for(node).clone() {
                        self.nodes.get_mut(&out).expect("Invalid node").incoming.remove(&node);
                    }

                    self.nodes.remove(&node);
                    changed = true;
                }
            }
        }
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

    pub fn to_dot(&self) -> String {
        use std::fmt::Write;

        let mut dot = String::new();

        dot.push_str("digraph G {");

        if let Some(start) = self.entry {
            write!(dot, "start -> {};", start).unwrap();
        }

        for node_id in self.nodes.keys() {
            for dest in self.outgoing_for(*node_id) {
                write!(dot, "{} -> {};", node_id, dest).unwrap();
            }
        }

        dot.push_str("}");

        dot
    }
}
