use std::collections::HashSet;

use crate::{cfg, lir, expr, dataflow::Abi};

#[derive(PartialEq, Hash, Eq, PartialOrd, Ord, Clone, Copy, Debug)]
pub struct Loc {
    pub node: usize,
    pub stmt: usize
}

fn assignments(nodes: &Vec<lir::LirNode>, name: &str, assignments: &mut HashSet<Loc>) {
    for (n, node) in nodes.iter().enumerate() {
        for (s, stmt) in node.code.iter().enumerate() {
            if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = stmt && nm == name {
                assignments.insert(Loc { node: n, stmt: s });
            }
        }
    }
}

fn append_reads_before_writes(graph: &cfg::ControlFlowGraph, loc: Loc, nodes: &Vec<lir::LirNode>, name: &str, uses: &mut HashSet<Loc>, visited: &mut HashSet<usize>, abi: &Abi) {
    if !visited.insert(loc.node) {
        return;
    }
    
    for (s, stmt) in nodes[loc.node].code[loc.stmt..].iter().enumerate() {
        if stmt.count_reads(name) > 0 {
            uses.insert(Loc { node: loc.node, stmt: s + loc.stmt });
        } else if let lir::Lir::Return(_) = stmt && abi.callee_saved.contains(&name) {
            uses.insert(Loc { node: loc.node, stmt: s + loc.stmt });
        }

        if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = stmt && nm == name {
            return;
        }
    }

    for outgoing in graph.outgoing_for(loc.node) {
        append_reads_before_writes(graph, Loc { node: *outgoing, stmt: 0 }, nodes, name, uses, visited, abi);
    }
}

fn all_writes_before_match(graph: &cfg::ControlFlowGraph, loc: Loc, nodes: &Vec<lir::LirNode>, name: &str, target: Loc, visited: &mut HashSet<usize>, abi: &Abi) -> bool {
    if !visited.insert(loc.node) {
        return true;
    }

    let mut s = loc.stmt;
    while s > 0 {
        s -= 1;

        if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = &nodes[loc.node].code[s] && nm == name {
            return Loc { stmt: s, node: loc.node } == target;
        }
    }

    for incoming in graph.incoming_for(loc.node) {
        if !all_writes_before_match(graph, Loc { node: *incoming, stmt: nodes[*incoming].code.len() }, nodes, name, target, visited, abi) {
            return false;
        }
    }
    true
}

pub fn ssaify(graph: &cfg::ControlFlowGraph, nodes: &mut Vec<lir::LirNode>, name: &str, abi: &Abi) -> HashSet<Loc> {
    let mut ssa_names = HashSet::new();

    let mut assignment_set = HashSet::new();
    assignments(nodes, name, &mut assignment_set);

    let mut ssa_count = 0;

    for assignment in assignment_set {
        let mut reads = HashSet::new();
        append_reads_before_writes(graph, Loc { node: assignment.node, stmt: assignment.stmt + 1 }, nodes, name, &mut reads, &mut HashSet::new(), abi);

        let mut all = true;
        for read in &reads {
            if !all_writes_before_match(graph, *read, nodes, name, assignment, &mut HashSet::new(), abi) {
                all = false;
            }
        }

        if all {
            let new_name = format!("{name}{ssa_count}");
            ssa_count += 1;
            ssa_names.insert(assignment);

            let lir::Lir::Assign { dst, .. } = &mut nodes[assignment.node].code[assignment.stmt] else {
                unreachable!()
            };

            let new_node = &expr::Expr::Name(new_name.clone());
            *dst = new_node.clone();

            for read in reads {
                nodes[read.node].code[read.stmt].replace_name(name, new_node);

                if let lir::Lir::Return(_) = &nodes[read.node].code[read.stmt] && abi.callee_saved.contains(&name) {
                    // This is a return, so it is the last item in the block, so inserting an element will not change any locs
                    nodes[read.node].code.insert(read.stmt, lir::Lir::Assign { src: new_node.clone(), dst: expr::Expr::Name(name.to_string()) })
                }
            }
        }
    }

    ssa_names
}
