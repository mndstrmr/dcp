use std::collections::HashSet;

use crate::{cfg, lir, Abi, expr};

#[derive(PartialEq)]
struct Loc {
    node: usize,
    stmt: usize
}

fn find_single_write_for<'a>(graph: &cfg::ControlFlowGraph, code: &'a [lir::Lir], node: usize, nodes: &'a Vec<lir::LirNode>, name: &str, visited: &mut HashSet<usize>) -> Option<(&'a expr::Expr, Loc)> {
    if !visited.insert(node) {
        return None;
    }

    let mut s = code.len();
    while s > 0 {
        s -= 1;

        if let lir::Lir::Assign { dst: expr::Expr::Name(nm), src } = &code[s] && *nm == name {
            return Some((src, Loc { node, stmt: s }))
        }
    }

    let mut write: Option<(&expr::Expr, Loc)> = None;
    for incoming in graph.incoming_for(node) {
        if let Some((new_write, loc)) = find_single_write_for(graph, &nodes[*incoming].code, *incoming, nodes, name, visited) {
            if let Some(write) = write && write.1 != loc {
                return None;
            }
            write = Some((new_write, loc));
        }
    }
    write
}

fn is_clobbered<'a>(graph: &cfg::ControlFlowGraph, code: &'a [lir::Lir], node: usize, nodes: &'a Vec<lir::LirNode>, name: &str, until_write: &str, visited: &mut HashSet<usize>) -> bool {
    if !visited.insert(node) {
        return false;
    }

    let mut s = code.len();
    while s > 0 {
        s -= 1;

        if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = &code[s] {
            if *nm == until_write {
                return false
            }

            if *nm == name {
                return true
            }
        }
    }

    for incoming in graph.incoming_for(node) {
        if is_clobbered(graph, &nodes[*incoming].code, *incoming, nodes, name, until_write, visited) {
            return true;
        }
    }
    false
}

fn elim_name_in(graph: &cfg::ControlFlowGraph, node: usize, nodes: &mut Vec<lir::LirNode>, _abi: &Abi, name: &str) {
    let mut s = 0;
    'outer: while s < nodes[node].code.len() {
        let stmt = &nodes[node].code[s];

        if stmt.count_reads(name) == 0 {
            s += 1;
            continue;
        }

        let Some((value, _)) = find_single_write_for(graph, &nodes[node].code[0..s], node, nodes, name, &mut HashSet::new()) else {
            s += 1;
            continue;
        };
        
        for dep in value.read_names_rhs() {
            if is_clobbered(graph, &nodes[node].code[0..s], node, nodes, dep, name, &mut HashSet::new()) {
                s += 1;
                continue 'outer;
            }
        }

        let clone = value.clone();
        nodes[node].code[s].replace_name(name, &clone);

        s += 1;
    }
}

pub fn elim_name(graph: &cfg::ControlFlowGraph, nodes: &mut Vec<lir::LirNode>, abi: &Abi, name: &str) {
    for n in 0..nodes.len() {
        elim_name_in(graph, n, nodes, abi, name);
    }
}
