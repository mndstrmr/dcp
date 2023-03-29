#![feature(let_chains)]

mod dead_writes;
pub use dead_writes::*;

mod single_use;
pub use single_use::*;

use std::collections::HashSet;

enum ReadWrite {
    Reads,
    Writes,
    Neither
}

fn stmt_reads_or_writes(stmt: &ir::Stmt, name: &str, abi: &abi::Abi) -> ReadWrite {
    if stmt.count_reads(name) > 0 {
        return ReadWrite::Reads;
    }

    if let ir::Stmt::Return(_) = stmt && abi.callee_saved.contains(&name) {
        return ReadWrite::Reads;
    }

    if let ir::Stmt::Assign { lhs: ir::Expr::Name(nm), .. } = stmt && nm == name {
        return ReadWrite::Writes;
    }

    ReadWrite::Neither
}

fn self_reads_or_writes(node: &ir_to_cfg::IrNode, name: &str, abi: &abi::Abi) -> ReadWrite {
    for stmt in &node.code {
        match stmt_reads_or_writes(stmt, name, abi) {
            ReadWrite::Reads => return ReadWrite::Reads,
            ReadWrite::Writes => return ReadWrite::Writes,
            ReadWrite::Neither => {}
        }
    }

    ReadWrite::Neither
}

fn reads_before_writes(graph: &cfg::ControlFlowGraph, node: usize, name: &str, nodes: &Vec<ir_to_cfg::IrNode>, visited: &mut HashSet<cfg::NodeId>, abi: &abi::Abi) -> bool {
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

        for neighbour in graph.outgoing_for(nodes[node].id) {
            let neighbour = ir_to_cfg::irnode_by_cfg_node(*neighbour, nodes);
            if visited.insert(neighbour) {
                to_visit.push(neighbour);
            }
        }
    }

    false
}
