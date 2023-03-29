#![feature(let_chains)]

use std::rc::Rc;

pub fn elim_consecutive_jump_labels(block: &mut ir::Block) {
    let mut i = 0;

    while i < block.len() {
        let ir::Stmt::Branch { target, .. } = block.at(i) else {
            i += 1;
            continue;
        };

        let mut j = i + 1;
        while j < block.len() {
            match block.at(j) {
                ir::Stmt::Label(label) => {
                    if let Some(label) = label.upgrade() && Rc::ptr_eq(&label, target) {
                        block.get_mut().remove(i);
                        break;
                    }

                    j += 1;
                }
                ir::Stmt::Nop => j += 1,
                _ => break
            }
        }

        // Safe even if we removed the branch since we would just hit the label anyway
        i += 1;
    }
}

pub fn elim_consecutive_control_flow(block: &mut ir::Block) {
    let mut i = 0;

    while i < block.len() {
        match block.at_mut(i) {
            ir::Stmt::Branch { cond: None, .. } | ir::Stmt::Continue | ir::Stmt::Break | ir::Stmt::Return(_) => {
                let mut j = i + 1;
                while j < block.len() {
                    if let ir::Stmt::Label(label) = block.at(j) && label.upgrade().is_some() {
                        break;
                    }
                    j += 1;
                }
                block.get_mut().drain(i + 1..j);
                i += 1;
            }
            ir::Stmt::If { true_then, false_then, .. } => {
                elim_consecutive_control_flow(true_then);
                elim_consecutive_control_flow(false_then);
                i += 1;
            }
            ir::Stmt::Loop { code } => {
                elim_consecutive_control_flow(code);
                i += 1;
            }
            _ => i += 1
        }
    }
}

pub fn loop_break_early(block: &mut ir::Block) {
    let mut i = 0;

    while i < block.len() {
        if let ir::Stmt::If { true_then, false_then, cond } = block.at_mut(i) {
            if let Some(ir::Stmt::Break) = false_then.non_nop_last() {
                *cond = cond.neg();

                let true_code = true_then.get_mut().drain(..).collect::<Vec<_>>();
                let false_code = false_then.get_mut().drain(..).collect::<Vec<_>>();

                true_then.get_mut().extend(false_code);
                false_then.get_mut().extend(true_code);
            }
        }

        if let ir::Stmt::If { true_then, false_then, .. } = block.at_mut(i) {
            if let Some(ir::Stmt::Break) = true_then.non_nop_last() {
                let false_then = false_then.get_mut().drain(..).collect::<Vec<_>>();
                let after = block.get_mut().drain(i + 1..).collect::<Vec<_>>();
                block.get_mut().extend(false_then);
                block.get_mut().extend(after);
            }
        }

        match block.at_mut(i) {
            ir::Stmt::If { true_then, false_then, .. } => {
                loop_break_early(true_then);
                loop_break_early(false_then);
                i += 1;
            }
            ir::Stmt::Loop { code } => {
                loop_break_early(code);
                i += 1;
            }
            _ => i += 1
        }
    }
}


pub fn elim_loop_final_continue(block: &mut ir::Block) {
    let mut i = 0;

    while i < block.len() {
        match block.at_mut(i) {
            ir::Stmt::If { true_then, false_then, .. } => {
                elim_loop_final_continue(true_then);
                elim_loop_final_continue(false_then);
                i += 1;
            }
            ir::Stmt::Loop { code } => {
                if let Some(ir::Stmt::Continue) = code.non_nop_last() {
                    code.pop_non_nop_last();
                }

                elim_loop_final_continue(code);
                i += 1;
            }
            _ => i += 1
        }
    }
}


pub fn collapse_cmp(block: &mut ir::Block) {
    for stmt in block.get_mut() {
        match stmt {
            ir::Stmt::Assign { lhs, rhs } => {
                lhs.collapse_cmp();
                rhs.collapse_cmp();
            }
            ir::Stmt::Branch { cond: Some(cond), .. } => cond.collapse_cmp(),
            ir::Stmt::If { cond, true_then, false_then } => {
                cond.collapse_cmp();
                collapse_cmp(true_then);
                collapse_cmp(false_then);
            }
            ir::Stmt::Loop { code } => collapse_cmp(code),
            ir::Stmt::For { inc: _inc, guard, code } => {
                // FIXME: collapse inc
                guard.collapse_cmp();
                collapse_cmp(code);
            }
            ir::Stmt::Return(expr) => expr.collapse_cmp(),
            _ => {}
        }
    }
}


pub fn elim_nop_assignments(block: &mut ir::Block) {
    let mut i = 0;
    while i < block.len() {
        i += 1;
        match block.at_mut(i - 1) {
            ir::Stmt::Assign { lhs, rhs } if lhs == rhs => {
                i -= 1;
                block.get_mut().remove(i);
            }
            ir::Stmt::For { code, .. } => elim_nop_assignments(code),
            ir::Stmt::If { true_then, false_then, .. } => {
                elim_nop_assignments(true_then);
                elim_nop_assignments(false_then);
            }
            ir::Stmt::Loop { code } => elim_nop_assignments(code),
            _ => {}
        }
    }
}
