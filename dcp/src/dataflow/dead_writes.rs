use std::collections::HashSet;

use crate::{cfg, lir, Abi, expr};

use super::{stmt_reads_or_writes, ReadWrite, reads_before_writes};

// If x = a can be removed if x is not read before it is next written
fn elim_dead_write_in(graph: &cfg::ControlFlowGraph, node: usize, nodes: &mut Vec<lir::LirNode>, abi: &Abi) -> bool {
    let mut changed = false;

    let mut i = 0;
    'outer: while i < nodes[node].code.len() {
        if let lir::Lir::Assign { dst: expr::Expr::Name(name), .. } = &nodes[node].code[i] {
            i += 1;
            for j in i..nodes[node].code.len() {
                match stmt_reads_or_writes(&nodes[node].code[j], name, abi) {
                    ReadWrite::Reads => {
                        continue 'outer
                    }
                    ReadWrite::Writes => {
                        i -= 1;
                        nodes[node].code.remove(i);
                        changed = true;
                        println!("!");
                        continue 'outer;
                    }
                    ReadWrite::Neither => {}
                }
            }

            let mut visited = HashSet::new();
            for outgoing in graph.outgoing_for(node) {
                if reads_before_writes(graph, *outgoing, name, nodes, &mut visited, abi) {
                    continue 'outer;
                }
            }

            i -= 1;
            nodes[node].code.remove(i);
            changed = true;
        } else {
            i += 1;
        }
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
