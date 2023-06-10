use std::collections::HashSet;

use crate::{lir, expr, Module, FunctionDefSet};

use super::ReadWrite;

fn stmt_reads_or_writes_recursive(module: &mut Module, defs: &FunctionDefSet, visited: &mut HashSet<expr::FuncId>, global_node: expr::FuncId, node: usize, stmti: usize, name: &str) -> ReadWrite {
    let stmt = &defs.find(global_node).unwrap().local_lirnodes[node].code[stmti];
    
    if stmt.count_reads(name) > 0 {
        return ReadWrite::Reads;
    }

    if let lir::Lir::Return(_) = stmt && module.abi.callee_saved.contains(&name) {
        return ReadWrite::Reads;
    }

    if
        let lir::Lir::Assign { src: expr::Expr::Call { func, .. }, .. } = stmt &&
        let expr::Expr::Call { func, .. } = func.as_ref() &&
        let expr::Expr::Func(funcid) = func.as_ref() {
        if module.functions[funcid.0].args.contains(&name) {
            return ReadWrite::Reads;
        } else if visited.insert(*funcid) {
            let funcid = *funcid;
            visit_func(module, defs, visited, funcid);

            if module.find_decl(funcid).unwrap().args.contains(&name) {
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

fn reads_before_writes_recursive(module: &mut Module, defs: &FunctionDefSet, global_visited: &mut HashSet<expr::FuncId>, global_node: expr::FuncId, name: &str) -> bool {
    let mut to_visit = vec![0];
    let mut visited = HashSet::new();

    'outer: while let Some(node) = to_visit.pop() {
        for i in 0..defs.find(global_node).unwrap().local_lirnodes[node].code.len() {
            match stmt_reads_or_writes_recursive(module, defs, global_visited, global_node, node, i, name) {
                ReadWrite::Reads => return true,
                ReadWrite::Writes => continue 'outer,
                ReadWrite::Neither => {}
            }
        }

        for neighbour in defs.find(global_node).unwrap().local_cfg.outgoing_for(node) {
            if visited.insert(*neighbour) {
                to_visit.push(*neighbour);
            }
        }
    }

    false
}

fn visit_func(module: &mut Module, defs: &FunctionDefSet, global_visited: &mut HashSet<expr::FuncId>, global_node: expr::FuncId) -> bool {
    let mut changed = false;
    for a in module.find_decl(global_node).unwrap().args.len()..module.abi.args.len() {
        let arg = module.abi.args[a];
        if reads_before_writes_recursive(module, defs, global_visited, global_node, arg) {
            module.find_decl_mut(global_node).unwrap().args.push(arg);
            changed = true;
        } else {
            break;
        }
    }
    changed
}

pub fn func_args(module: &mut Module, defs: &FunctionDefSet) {
    let mut changed = true;

    while changed {
        changed = false;

        let mut visited = HashSet::new();
        let mut i = 0;
        while i < module.functions.len() {
            changed = changed || visit_func(module, defs, &mut visited, module.functions[i].funcid);
            i += 1;
        }
    }
}

fn insert_func_args_in_expr(module: &Module, expr: &mut expr::Expr) {
    match expr {
        expr::Expr::Binary { lhs, rhs, .. } => {
            insert_func_args_in_expr(module, lhs);
            insert_func_args_in_expr(module, rhs);
        }
        expr::Expr::Unary { expr, .. } => {
            insert_func_args_in_expr(module, expr);
        }
        expr::Expr::Call { func, args } => {
            if let expr::Expr::Func(funcid) = func.as_ref() && let Some(sig) = module.find_decl(*funcid) {
                args.extend(sig.args.iter().cloned().map(str::to_string).map(expr::Expr::Name));
            } else {
                insert_func_args_in_expr(module, func);
                for arg in args {
                    insert_func_args_in_expr(module, arg);
                }
            }
        }
        expr::Expr::Bool(_) | expr::Expr::Name(_) | expr::Expr::Num(_) | expr::Expr::Func(_) => {},
        expr::Expr::Deref { ptr, .. } => {
            insert_func_args_in_expr(module, ptr);
        }
        expr::Expr::Ref(value) => {
            insert_func_args_in_expr(module, value);
        }
    }
}

fn insert_func_args_in(module: &Module, node: usize, nodes: &mut Vec<lir::LirNode>) {
    for stmt in &mut nodes[node].code {
        match stmt {
            lir::Lir::Assign { src, dst } => {
                insert_func_args_in_expr(module, src);
                insert_func_args_in_expr(module, dst);
            }
            lir::Lir::Branch { cond: Some(cond), .. } => {
                insert_func_args_in_expr(module, cond);
            }
            lir::Lir::Return(expr) => insert_func_args_in_expr(module, expr),
            lir::Lir::Do(expr) => insert_func_args_in_expr(module, expr),
            lir::Lir::Label(_) | lir::Lir::Branch { .. } => {}
        }
    }
}

pub fn insert_func_args(module: &Module, defs: &mut FunctionDefSet) {
    for function in &module.functions {
        let Some(def) = defs.find_mut(function.funcid) else {
            continue;
        };

        let mut i = 0;
        while i < def.local_lirnodes.len() {
            insert_func_args_in(module, i, &mut def.local_lirnodes);
            i += 1;
        }
    }
}
