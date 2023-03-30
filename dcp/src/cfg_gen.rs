use std::collections::{HashSet, HashMap};

use crate::{cfg, lir};

pub fn gen_cfg(blir: &[lir::LirNode]) -> cfg::ControlFlowGraph {
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
