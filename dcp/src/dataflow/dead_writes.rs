use std::collections::HashSet;

use crate::{cfg, lir, dataflow::Abi, expr};

fn has_reader(cfg: &cfg::ControlFlowGraph, nodes: &[lir::LirNode], node: usize, stmt: usize, name: &str, abi: &Abi, visited: &mut HashSet<usize>) -> bool {
    if !visited.insert(node) {
        return false;
    }

    for assignment in nodes[node].code.iter().skip(stmt) {
        if assignment.count_reads(name) > 0 {
            return true;
        }

        if let lir::Lir::Return(_) = assignment && abi.callee_saved.contains(&name) {
            return true;
        }

        if assignment.writes_to(name) {
            return false;
        }
    }

    for outgoing in cfg.outgoing_for(node) {
        if has_reader(cfg, nodes, *outgoing, 0, name, abi, visited) {
            return true;
        }
    }

    false
}

/// If x = a can be removed if x is not read before it is next written
fn elim_dead_write_in(cfg: &cfg::ControlFlowGraph, node: usize, nodes: &mut Vec<lir::LirNode>, abi: &Abi) -> bool {
    let mut changed = false;

    let mut i = 0;
    while i < nodes[node].code.len() {
        let lir::Lir::Assign { dst: expr::Expr::Name(name), src } = &nodes[node].code[i] else {
            i += 1;
            continue;
        };

        if abi.global.contains(&name.as_str()) {
            i += 1;
            continue;
        }

        if src.has_side_effects() {
            i += 1;
            continue;
        }

        if has_reader(cfg, nodes, node, i + 1, name, abi, &mut HashSet::new()) {
            i += 1;
            continue;
        }

        nodes[node].code.remove(i);
        changed = true;
    }

    changed
}

pub fn elim_dead_writes(graph: &cfg::ControlFlowGraph, nodes: &mut Vec<lir::LirNode>, abi: &Abi) {
    let mut i = 0;
    while i < nodes.len() {
        while elim_dead_write_in(graph, i, nodes, abi) {}
        i += 1;
    }
}
