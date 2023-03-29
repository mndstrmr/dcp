use std::{rc::Rc, collections::HashSet};

fn backtrack(code: &ir::Block, visited: &mut HashSet<*const ir::Label>, target: *const ir::Label, mut idx: usize) -> Option<usize> {
    while idx > 0 {
        idx -= 1;

        match code.at(idx) {
            ir::Stmt::Label(label) => {
                visited.remove(&label.as_ptr());

                if label.as_ptr() == target {
                    return Some(idx);
                }
            }
            ir::Stmt::If { true_then, false_then, .. } => {
                if let Some(_) = backtrack(true_then, visited, target, true_then.len()) {
                    return Some(idx);
                }
                
                if let Some(_) = backtrack(false_then, visited, target, false_then.len()) {
                    return Some(idx);
                }
            }
            ir::Stmt::Loop { code } => {
                if let Some(_) = backtrack(code, visited, target, code.len()) {
                    return Some(idx);
                }
            }
            _ => {}
        }
    }
    
    None
}

fn gotos_to_loops_once(code: &mut ir::Block, visited: &mut HashSet<*const ir::Label>, completed: &mut HashSet<*const ir::Label>) -> Option<*const ir::Label> {
    let mut iter = ir::BlindBlockIter::new();

    while let Some(stmt) = iter.next(code) {
        macro_rules! bt {
            ($ptr:expr) => {
                match backtrack(code, visited, $ptr, iter.offset()) {
                    Some(idx) => {
                        let mut body = code.get_mut().drain(idx..iter.offset() + 1).collect::<Vec<_>>();
                        body.push(ir::Stmt::Break);
                        let loop_ = ir::Stmt::Loop {
                            code: ir::Block::new_from(body)
                        };
                        code.get_mut().insert(idx, loop_);
                        completed.insert($ptr);
                        iter.seek(idx);
                    }
                    None => return Some($ptr)
                }
            };
        }

        match stmt {
            ir::Stmt::Label(label) => {
                if let Some(label) = label.upgrade() {
                    visited.insert(Rc::as_ptr(&label));
                }
            }
            ir::Stmt::Branch { target: btarget, .. } => {
                let ptr = Rc::as_ptr(btarget);
                if completed.contains(&ptr) || !visited.contains(&ptr) { continue; }
                
                bt!(ptr);
            }
            ir::Stmt::If { .. } => {
                let cont = match code.at_mut(iter.offset()) {
                    ir::Stmt::If { true_then, .. } => {
                        if let Some(ptr) = gotos_to_loops_once(true_then, visited, completed) {
                            bt!(ptr);
                            false
                        } else {
                            true
                        }
                    }
                    _ => unreachable!()
                };

                if cont {
                    match code.at_mut(iter.offset()) {
                        ir::Stmt::If { false_then, .. } => {
                            if let Some(ptr) = gotos_to_loops_once(false_then, visited, completed) {
                                bt!(ptr);
                            }
                        }
                        _ => unreachable!()
                    }
                }
            }
            ir::Stmt::Loop { .. } => {
                match code.at_mut(iter.offset()) {
                    ir::Stmt::Loop { code: lcode } => {
                        if let Some(ptr) = gotos_to_loops_once(lcode, visited, completed) {
                            bt!(ptr);
                        }
                    }
                    _ => unreachable!()
                }
            }
            _ => {}
        }
    }

    None
}

fn gotos_to_loops(code: &mut ir::Block) {
    assert!(gotos_to_loops_once(code, &mut HashSet::new(), &mut HashSet::new()).is_none());
}

fn gotos_to_break_cont(code: &mut ir::Block, loop_scope: Option<&ir::ScopeLabels>, parent: &ir::ScopeLabels) {
    let mut iter = ir::BlindBlockIter::new();

    while let Some(stmt) = iter.next(code) {
        match stmt {
            ir::Stmt::If { .. } => {
                let scope = ir::ScopeLabels::new(
                    code.labels_back_from(iter.offset(), parent),
                    code.labels_forward_from(iter.offset(), parent),
                );

                if let ir::Stmt::If { true_then, false_then, .. } = code.at_mut(iter.offset()) {
                    gotos_to_break_cont(true_then, loop_scope, &scope);
                    gotos_to_break_cont(false_then, loop_scope, &scope);
                }
            }
            ir::Stmt::Loop { .. } => {
                let mut scope = ir::ScopeLabels::new(
                    Vec::new(),
                    code.labels_forward_from(iter.offset(), parent),
                );

                if let ir::Stmt::Loop { code: lcode } = code.at_mut(iter.offset()) {
                    scope.append_start(lcode.labels_at(0, &parent).iter().map(|x| Rc::downgrade(x)));
                    gotos_to_break_cont(lcode, Some(&scope), &scope);
                }
            }
            ir::Stmt::Branch { target, cond: None } => {
                let target = target.clone();

                if let Some(loop_scope) = loop_scope {
                    if loop_scope.start_weak().iter().position(|x| x.as_ptr() == Rc::as_ptr(&target)).is_some() {
                        *code.at_mut(iter.offset()) = ir::Stmt::Continue;
                    }

                    if loop_scope.end_weak().iter().position(|x| x.as_ptr() == Rc::as_ptr(&target)).is_some() {
                        *code.at_mut(iter.offset()) = ir::Stmt::Break;
                    }
                }
            }
            ir::Stmt::Branch { target, cond: Some(cond) } => {
                let target = target.clone();
                let cond = cond.clone();

                if let Some(loop_scope) = loop_scope {
                    if loop_scope.start_weak().iter().position(|x| x.as_ptr() == Rc::as_ptr(&target)).is_some() {
                        *code.at_mut(iter.offset()) = ir::Stmt::If {
                            cond: cond.clone(),
                            true_then: ir::Block::new_from(vec![ir::Stmt::Continue]),
                            false_then: ir::Block::empty()
                        };
                    }

                    if loop_scope.end_weak().iter().position(|x| x.as_ptr() == Rc::as_ptr(&target)).is_some() {
                        *code.at_mut(iter.offset()) = ir::Stmt::If {
                            cond,
                            true_then: ir::Block::new_from(vec![ir::Stmt::Break]),
                            false_then: ir::Block::empty()
                        };
                    }
                }
            }
            _ => {}
        }
    }
}

fn step_back_if_break(code: &mut ir::Block) {
    /* 
    
    if x {
        A
    } else {
        B
    }
    break

    ==>

    if x {
        A
        break
    } else {
        B
        break
    }
    */

    let mut iter = ir::BlindMultiBlockIter::<2>::new();

    while let indices = iter.step(code) && indices.len() > 0 {
        if 
            indices.len() > 1 &&
            let ir::Stmt::If { .. } = code.at_mut(indices[0]) &&
            let ir::Stmt::Break = code.at_mut(indices[1])
        {
            code.get_mut().remove(indices[1]);

            if let ir::Stmt::If { true_then, false_then, .. } = code.at_mut(indices[0]) {
                true_then.add(ir::Stmt::Break);
                false_then.add(ir::Stmt::Break);
            }
        }

        match code.at_mut(indices[0]) {
            ir::Stmt::If { true_then, false_then, .. } => {
                step_back_if_break(true_then);
                step_back_if_break(false_then);
            }
            ir::Stmt::Loop { code } => {
                step_back_if_break(code);
            }
            _ => {}
        }
    }
}

pub fn insert_loops(code: &mut ir::Block) {
    gotos_to_loops(code);
    gotos_to_break_cont(code, None, &ir::ScopeLabels::empty());
    step_back_if_break(code);
}
