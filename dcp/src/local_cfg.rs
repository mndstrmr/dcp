use std::collections::{HashSet, HashMap};

use crate::{cfg, lir, mir};

pub fn gen_local_cfg(blir: &[lir::LirNode]) -> cfg::ControlFlowGraph {
    let mut cfg = cfg::ControlFlowGraph::new();

    for i in 0..blir.len() {
        cfg.add_node(i);
    }

    for (i, node) in blir.iter().enumerate() {
        match node.code.last() {
            Some(lir::Lir::Return(_)) => {},
            Some(lir::Lir::Branch { cond: Some(_), target }) => {
                cfg.add_edge(i, target.0);
                cfg.add_edge(i, i + 1);
            },
            Some(lir::Lir::Branch { cond: None, target }) => cfg.add_edge(i, target.0),
            _ => cfg.add_edge(i, i + 1),
        }
    }

    cfg.set_entry(0);
    cfg
}

pub fn lir_to_lirnodes(mut lir: lir::LirFunc) -> Vec<lir::LirNode> {
    let mut used_labels = HashSet::new();
    let mut i = 0;
    while i < lir.len() {
        let stmt = lir.at(i).unwrap();

        if let lir::Lir::Branch { target, .. } = stmt {
            used_labels.insert(*target);

            match lir.at(i + 1) {
                Some(lir::Lir::Label(label)) => used_labels.insert(*label),
                None => false,
                Some(_) => panic!("Branch must be followed by a label")
            };
        }

        i += 1;
    }

    let mut label_to_node_id = HashMap::new();
    let mut node_id = 0;
    let mut i = 0;
    while i < lir.len() {
        let stmt = lir.at(i).unwrap();

        if let lir::Lir::Label(label) = stmt && used_labels.contains(label) {
            node_id += 1;
            label_to_node_id.insert(*label, node_id);
        }

        i += 1;
    }
    
    let mut nodes = Vec::new();

    let mut i = 0;
    while i < lir.len() {
        let stmt = lir.at(i).unwrap();

        let block = 
            if let lir::Lir::Label(label) = stmt && used_labels.contains(label) {
                let res = Some(lir.get_mut().drain(..i));
                i = 1;
                res
            } else if i == lir.len() - 1 {
                i = 0;
                Some(lir.get_mut().drain(..))
            } else {
                i += 1;
                None
            };

        if let Some(block) = block {
            nodes.push(lir::LirNode {
                code: block.filter_map(|lir| match lir {
                    lir::Lir::Branch { cond, target } => Some(lir::Lir::Branch { cond, target: lir::Label(*label_to_node_id.get(&target).unwrap()) }),
                    lir::Lir::Label(_) => None,
                    x => Some(x),
                }).collect()
            });
        }
    }

    nodes
}

fn discover_nodes(subgraph: &HashSet<cfg::NodeId>, entry: cfg::NodeId, cfg: &cfg::ControlFlowGraph, dominators: &cfg::Dominators) -> HashSet<cfg::NodeId> {
    let mut to_visit = vec![entry];
    let mut visited = HashSet::new();
    visited.insert(entry);

    while let Some(node) = to_visit.pop() {
        for neighbour in cfg.outgoing_for(node) {
            if !dominators.implies_backwards_edge(node, *neighbour) && subgraph.contains(neighbour) && visited.insert(*neighbour) {
                to_visit.push(*neighbour);
            }
        }
    }

    visited
}

fn append_subgraph_to_block(subgraph: HashSet<cfg::NodeId>, entry: cfg::NodeId, cfg: &cfg::ControlFlowGraph, dominators: &cfg::Dominators, nodes: &mut Vec<lir::LirNode>, block: &mut Vec<mir::Mir>) {
    if !subgraph.contains(&entry) {
        return;
    }

    block.push(mir::Mir::Label(lir::Label(entry)));
    block.extend(nodes[entry].code.drain(..).map(lir::Lir::into));

    let mut node = entry;
    let outgoing = loop {
        let outgoing = cfg.outgoing_for(node).iter()
            .filter(|x| subgraph.contains(x) && !dominators.implies_backwards_edge(node, **x))
            .collect::<Vec<_>>();
        
        if outgoing.len() == 0 {
            return;
        }

        if outgoing.len() == 2 {
            break outgoing;
        }

        assert!(outgoing.len() == 1);

        block.push(mir::Mir::Label(lir::Label(*outgoing[0])));
        block.extend(nodes[*outgoing[0]].code.drain(..).map(lir::Lir::into));

        node = *outgoing[0];
    };

    let Some(mir::Mir::Branch { cond: Some(mut cond), target }) = block.pop() else {
        unreachable!()
    };

    assert_eq!(outgoing.len(), 2);

    let (mut a, mut b) =
        if *outgoing[0] == target.0 {
            (*outgoing[0], *outgoing[1])
        } else {
            assert_eq!(*outgoing[1], target.0);
            (*outgoing[1], *outgoing[0])
        };

    let red = discover_nodes(&subgraph, a, cfg, dominators);
    let blue = discover_nodes(&subgraph, b, cfg, dominators);

    let purple = red.intersection(&blue).copied().collect();

    let mut red: HashSet<_> = red.difference(&purple).copied().collect();
    let mut blue: HashSet<_> = blue.difference(&purple).copied().collect();
    
    // Move return statements towards the end
    for node in &red {
        if matches!(nodes[*node].code.last(), Some(lir::Lir::Return(_))) {
            (red, blue) = (blue, red);
            (a, b) = (b, a);
            cond = cond.neg();
            break;
        }
    }

    if !red.is_empty() && !blue.is_empty() {
        let mut true_then = Vec::new();
        append_subgraph_to_block(red, a, cfg, dominators, nodes, &mut true_then);

        let mut false_then = Vec::new();
        append_subgraph_to_block(blue, b, cfg, dominators, nodes, &mut false_then);

        block.push(mir::Mir::If {
            cond,
            true_then,
            false_then,
        });
    } else if red.is_empty() {
        let mut false_then = Vec::new();
        append_subgraph_to_block(blue, b, cfg, dominators, nodes, &mut false_then);

        block.push(mir::Mir::If {
            cond: cond.neg(),
            true_then: false_then,
            false_then: Vec::new(),
        });
    }

    let purple_starts = purple.iter()
                            .filter(|x| cfg.incoming_for(**x).intersection(&purple).next().is_none())
                            .copied().collect::<Vec<_>>();

    if !purple_starts.is_empty() {
        assert!(purple_starts.len() == 1);
        append_subgraph_to_block(purple, purple_starts[0], cfg, dominators, nodes, block);
    }
}

pub fn reorder_code(graph: &cfg::ControlFlowGraph, dominators: &cfg::Dominators, mut nodes: Vec<lir::LirNode>) -> mir::MirFunc {
    let mut code = Vec::new();
    append_subgraph_to_block(graph.nodes(), graph.get_entry().expect("No entry"), &graph, &dominators, &mut nodes, &mut code);
    mir::MirFunc::new(vec![], vec![], code)
}
