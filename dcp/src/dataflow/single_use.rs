use std::collections::HashSet;

use crate::{cfg, lir, dataflow::Abi, expr};

// if (1) is `x = a` and (2) is `b`. (1) can be deleted and `b` can have `x` replaced with `a` under the following conditions:
//  1. the value of x at (2) will always be derived from (1) expression
//          there exists no backward path form (2) to any assignment of x that is not (1)
//  2. no other expressions use this value of `x`
//          there exists no read before write from (1) other than (2)
//  3. the value of x calculates to the same (i.e. all dependents are unchanged, including memory or side effects) at the location b
//          either there exists no path from (1) to (2) where a dependent of a is assigned to, memory is written to or a function is called

struct Reader {
    /// vector of (node, start_stmt, end_stmt), where the region of affected code is [start_stmt..end_stmt)
    path: Vec<(usize, usize, usize)>,

    /// (node, stmt) for read
    dst: (usize, usize)
}

/// Finds the readers of name starting from and including (node, stmt). The readers may not have unique destinations, but will have unique paths.
fn find_readers(cfg: &cfg::ControlFlowGraph, nodes: &[lir::LirNode], node: usize, stmt: usize, name: &str, visited: &mut HashSet<usize>) -> Vec<Reader> {
    if !visited.insert(node) {
        return vec![];
    }

    let mut readers = Vec::new();

    for (s, assignment) in nodes[node].code.iter().enumerate().skip(stmt) {
        if assignment.count_reads(name) > 0 {
            readers.push(Reader {
                path: vec![(node, stmt, s + 1)],
                dst: (node, s)
            });
        }

        if assignment.writes_to(name) {
            return readers;
        }
    }

    for outgoing in cfg.outgoing_for(node) {
        readers.extend(find_readers(cfg, nodes, *outgoing, 0, name, visited).into_iter().map(|mut x| Reader {
            dst: x.dst,
            path: {
                x.path.insert(0, (node, stmt, nodes[node].code.len()));
                x.path
            }
        }))
    }

    readers
}

/// Counts the assignments which might write to name, moving backward from and not including (node, stmt)
fn count_writers(cfg: &cfg::ControlFlowGraph, nodes: &[lir::LirNode], node: usize, stmt: usize, name: &str, visited: &mut HashSet<usize>) -> usize {
    if !visited.insert(node) {
        return 0;
    }

    let mut writers = 0;

    for assignment in nodes[node].code.iter().take(stmt).rev() {
        if assignment.writes_to(name) {
            writers += 1;
            return writers;
        }
    }

    for incoming in cfg.incoming_for(node) {
        writers += count_writers(cfg, nodes, *incoming, nodes[*incoming].code.len(), name, visited);
    }

    writers
}

fn reader_path_is_candidate(_cfg: &cfg::ControlFlowGraph, nodes: &[lir::LirNode], clobbers: &[&str], reader: &Reader) -> bool {
    for (c, component) in reader.path.iter().enumerate() {
        for s in component.1..component.2 {
            let stmt = &nodes[component.0].code[s];

            for clobber in clobbers {
                if stmt.writes_to(&clobber) {
                    return false;
                }
            }

            // Don't allow side effects, unless they are in the last instruction
            // FIXME: This is incorrect in general
            if (s != component.2 - 1 || c != reader.path.len() - 1) && stmt.has_side_effects() {
                return false;
            }
        }
    }

    true
}

fn inline_single_use_names_in(cfg: &cfg::ControlFlowGraph, node: usize, nodes: &mut Vec<lir::LirNode>, _abi: &Abi) -> bool {
    let mut changed = false;

    let mut s = 0;
    'outer: while s < nodes[node].code.len() {
        let lir::Lir::Assign { dst: expr::Expr::Name(name), src } = &nodes[node].code[s] else {
            s += 1;
            continue;
        };

        // s + 1, because we don't want to include ourselves
        let readers = find_readers(cfg, nodes, node, s + 1, name, &mut HashSet::new());
        
        // Cannot be a unique reader if there are none
        if readers.is_empty() {
            s += 1;
            continue;
        }

        // Check they all have the same dest
        let dst = readers[0].dst;
        for reader in &readers[1..] {
            if reader.dst != dst {
                s += 1;
                continue 'outer
            }
        }

        // Check dst only reads once
        if nodes[dst.0].code[dst.1].count_reads(name) != 1 {
            s += 1;
            continue 'outer;
        }

        // Check all paths satisfy clobbering conditions
        let clobbers = src.read_names_rhs();
        for reader in &readers {
            if !reader_path_is_candidate(cfg, nodes, &clobbers, &reader) {
                s += 1;
                continue 'outer;
            }
        }

        // Check that there is really only one assignee of dst
        if count_writers(cfg, nodes, dst.0, dst.1, name, &mut HashSet::new()) != 1 {
            s += 1;
            continue 'outer;
        }

        // Do the replacement
        let src = src.clone();
        let name = name.clone();
        nodes[dst.0].code[dst.1].replace_name(&name, &src);
        nodes[node].code.remove(s);
        changed = true;
        
        // Revisit s
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
