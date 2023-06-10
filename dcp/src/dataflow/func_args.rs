use std::collections::HashSet;

use crate::{cfg, lir, expr, dataflow::Abi};

use super::ReadWrite;

pub struct GlobalSig {
    pub args: Vec<&'static str>
}

pub struct GlobalCfgNode {
    pub local_cfg: cfg::ControlFlowGraph,
    pub local_lirnodes: Vec<lir::LirNode>,
    pub args: Vec<&'static str>
}

impl GlobalCfgNode {
    pub fn new(local_cfg: cfg::ControlFlowGraph, local_lirnodes: Vec<lir::LirNode>) -> GlobalCfgNode {
        GlobalCfgNode {
            local_cfg,
            local_lirnodes,
            args: Vec::new()
        }
    }

    pub fn split(self) -> ((cfg::ControlFlowGraph, Vec<lir::LirNode>), GlobalSig) {
        ((self.local_cfg, self.local_lirnodes), GlobalSig { args: self.args })
    }
}

fn stmt_reads_or_writes_recursive(global_nodes: &mut [GlobalCfgNode], visited: &mut HashSet<usize>, global_node: usize, node: usize, stmti: usize, name: &str, abi: &Abi) -> ReadWrite {
    let stmt = &global_nodes[global_node].local_lirnodes[node].code[stmti];
    
    if stmt.count_reads(name) > 0 {
        return ReadWrite::Reads;
    }

    if let lir::Lir::Return(_) = stmt && abi.callee_saved.contains(&name) {
        return ReadWrite::Reads;
    }

    if
        let lir::Lir::Assign { src: expr::Expr::Call { func, .. }, .. } = stmt &&
        let expr::Expr::Call { func, .. } = func.as_ref() &&
        let expr::Expr::Func(funcid) = func.as_ref() {
        if global_nodes[funcid.0].args.contains(&name) {
            return ReadWrite::Reads;
        } else if visited.insert(funcid.0) {
            let id = funcid.0;
            visit_func(global_nodes, visited, id, abi);

            if global_nodes[id].args.contains(&name) {
                return ReadWrite::Reads;
            }
        }

        return ReadWrite::Writes;
    }

    if let lir::Lir::Assign { dst: expr::Expr::Name(nm), .. } = stmt && *nm == name {
        return ReadWrite::Writes;
    }

    ReadWrite::Neither
}

fn reads_before_writes_recursive(global_nodes: &mut [GlobalCfgNode], global_visited: &mut HashSet<usize>, global_node: usize, name: &str, abi: &Abi) -> bool {
    let mut to_visit = vec![0];
    let mut visited = HashSet::new();

    'outer: while let Some(node) = to_visit.pop() {
        for i in 0..global_nodes[global_node].local_lirnodes[node].code.len() {
            match stmt_reads_or_writes_recursive(global_nodes, global_visited, global_node, node, i, name, abi) {
                ReadWrite::Reads => return true,
                ReadWrite::Writes => continue 'outer,
                ReadWrite::Neither => {}
            }
        }

        for neighbour in global_nodes[global_node].local_cfg.outgoing_for(node) {
            if visited.insert(*neighbour) {
                to_visit.push(*neighbour);
            }
        }
    }

    false
}

fn visit_func(global_nodes: &mut [GlobalCfgNode], global_visited: &mut HashSet<usize>, global_node: usize, abi: &Abi) -> bool {
    let mut changed = false;
    // for arg in &abi.args {
    for a in global_nodes[global_node].args.len()..abi.args.len() {
        let arg = abi.args[a];
        if reads_before_writes_recursive(global_nodes, global_visited, global_node, arg, abi) {
            global_nodes[global_node].args.push(arg);
            changed = true;
        } else {
            break;
        }
    }
    changed
}

pub fn func_args(funcs: &mut [GlobalCfgNode], abi: &Abi) {
    let mut changed = true;

    while changed {
        changed = false;

        let mut visited = HashSet::new();
        for i in 0..funcs.len() {
            changed = changed || visit_func(funcs, &mut visited, i, abi);
        }
    }
}

fn insert_func_args_in_expr(sigs: &[GlobalSig], expr: &mut expr::Expr) {
    match expr {
        expr::Expr::Binary { lhs, rhs, .. } => {
            insert_func_args_in_expr(sigs, lhs);
            insert_func_args_in_expr(sigs, rhs);
        }
        expr::Expr::Unary { expr, .. } => {
            insert_func_args_in_expr(sigs, expr);
        }
        expr::Expr::Call { func, args } => {
            if let expr::Expr::Func(funcid) = func.as_ref() && let Some(sig) = sigs.get(funcid.0) {
                args.extend(sig.args.iter().cloned().map(str::to_string).map(expr::Expr::Name));
            } else {
                insert_func_args_in_expr(sigs, func);
                for arg in args {
                    insert_func_args_in_expr(sigs, arg);
                }
            }
        }
        expr::Expr::Bool(_) | expr::Expr::Name(_) | expr::Expr::Num(_) | expr::Expr::Func(_) => {},
        expr::Expr::Deref { ptr, .. } => {
            insert_func_args_in_expr(sigs, ptr);
        }
        expr::Expr::Ref(value) => {
            insert_func_args_in_expr(sigs, value);
        }
    }
}

fn insert_func_args_in(sigs: &[GlobalSig], node: usize, nodes: &mut Vec<lir::LirNode>) {
    for stmt in &mut nodes[node].code {
        match stmt {
            lir::Lir::Assign { src, dst } => {
                insert_func_args_in_expr(sigs, src);
                insert_func_args_in_expr(sigs, dst);
            }
            lir::Lir::Branch { cond: Some(cond), .. } => {
                insert_func_args_in_expr(sigs, cond);
            }
            lir::Lir::Return(expr) => insert_func_args_in_expr(sigs, expr),
            lir::Lir::Do(expr) => insert_func_args_in_expr(sigs, expr),
            lir::Lir::Label(_) | lir::Lir::Branch { .. } => {}
        }
    }
}

pub fn insert_func_args(sigs: &[GlobalSig], nodes: &mut Vec<lir::LirNode>) {
    let mut i = 0;
    while i < nodes.len() {
        insert_func_args_in(sigs, i, nodes);
        i += 1;
    }
}
