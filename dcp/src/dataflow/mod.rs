mod dead_writes;
pub use dead_writes::*;

mod single_use;
pub use single_use::*;

use std::collections::HashSet;

use crate::{cfg, lir, expr};

pub struct Abi {
    pub callee_saved: Vec<&'static str>
}

enum ReadWrite {
    Reads,
    Writes,
    Neither
}

fn stmt_reads_or_writes(stmt: &lir::Lir, name: &str, abi: &Abi) -> ReadWrite {
    if stmt.count_reads(name) > 0 {
        return ReadWrite::Reads;
    }

    if let lir::Lir::Return(_) = stmt && abi.callee_saved.contains(&name) {
        return ReadWrite::Reads;
    }

    if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = stmt && *nm == name {
        return ReadWrite::Writes;
    }

    ReadWrite::Neither
}

fn self_reads_or_writes(node: &lir::LirNode, name: &str, abi: &Abi) -> ReadWrite {
    for stmt in &node.code {
        match stmt_reads_or_writes(stmt, name, abi) {
            ReadWrite::Reads => return ReadWrite::Reads,
            ReadWrite::Writes => return ReadWrite::Writes,
            ReadWrite::Neither => {}
        }
    }

    ReadWrite::Neither
}

fn reads_before_writes(graph: &cfg::ControlFlowGraph, node: usize, name: &str, nodes: &Vec<lir::LirNode>, visited: &mut HashSet<cfg::NodeId>, abi: &Abi) -> bool {
    if !visited.insert(node) {
        return false;
    }

    let mut to_visit = vec![node];

    while let Some(node) = to_visit.pop() {
        match self_reads_or_writes(&nodes[node], name, abi) {
            ReadWrite::Reads => {
                return true
            },
            ReadWrite::Writes => {
                continue
            },
            ReadWrite::Neither => {}
        }

        for neighbour in graph.outgoing_for(node) {
            if visited.insert(*neighbour) {
                to_visit.push(*neighbour);
            }
        }
    }

    false
}
