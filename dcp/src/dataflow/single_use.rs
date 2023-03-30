use std::collections::{HashSet, hash_map::RandomState};

use crate::{cfg, lir, Abi, expr};

use super::{stmt_reads_or_writes, ReadWrite, reads_before_writes};

// use self::{reads_before_writes, ReadWrite, stmt_reads_or_writes, Abi};

// If x = a comes before b, and b depends on x, `x = a` can be removed, and b can have `x` replaced with `a` under the following conditions:
// 1. x cannot be reassigned between a and b
// 2. no dependency of a is reassigned between a and b
// 3. x cannot be used between a and b
// 4. x must be written to before it is next read from following b
// 5. b only uses x once
fn inline_single_use_names_in(graph: &cfg::ControlFlowGraph, node: usize, nodes: &mut Vec<lir::LirNode>, abi: &Abi) -> bool {
    let mut changed = false;

    let mut i = 0;
    while i < nodes[node].code.len() {
        if let lir::Lir::Assign { dst: expr::Expr::Name(name), src } = &nodes[node].code[i] {
            let deps = HashSet::<_, RandomState>::from_iter(src.read_names_rhs());

            i += 1;

            let mut j = i;
            'outer: while j < nodes[node].code.len() {
                let count = nodes[node].code[j].count_reads(name);

                // Condition 2 must fail
                if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = &nodes[node].code[j] && deps.contains(nm) && nm != name {
                    break;
                }

                // Condition 5 must fail
                if count > 1 {
                    break;
                }

                if count < 1 {
                    // Condition 1 must fail
                    if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = &nodes[node].code[j] && nm == name {
                        break;
                    }

                    j += 1;
                    continue;
                }

                // Satisfies conditions 1, 2, 3, 5

                // Allow immediate writes
                if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = &nodes[node].code[j] && nm == name {
                } else {
                    let mut allow = false;
                    for k in j + 1..nodes[node].code.len() {
                        match stmt_reads_or_writes(&nodes[node].code[k], name, abi) {
                            ReadWrite::Reads => break 'outer,
                            ReadWrite::Writes => {
                                allow = true;
                                break
                            },
                            ReadWrite::Neither => {}
                        }
                    }

                    if !allow {
                        for outgoing in graph.outgoing_for(node) {
                            let mut visited = HashSet::new();
                            if reads_before_writes(graph, *outgoing, name, nodes, &mut visited, abi) {
                                break 'outer;
                            }
                        }
                    }
                }

                // Satisfies condition 4

                i -= 1;

                let src = src.clone();
                let name = name.clone();
                nodes[node].code[j].replace_name(&name, &src);
                nodes[node].code.remove(i);

                changed = true;

                break;
            }
        } else {
            i += 1;
        }
    }

    changed
}

pub fn inline_single_use_names(graph: &cfg::ControlFlowGraph, nodes: &mut Vec<lir::LirNode>, abi: &Abi) {
    let mut i = 0;
    while i < nodes.len() {
        while inline_single_use_names_in(graph, i, nodes, abi) {}
        i += 1;
    }
}
