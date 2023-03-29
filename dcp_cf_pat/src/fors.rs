fn loop_contains_continue(block: &ir::Block) -> bool {
    for stmt in block.get() {
        match stmt {
            ir::Stmt::Continue => return true,
            ir::Stmt::If { true_then, false_then, .. } => if loop_contains_continue(true_then) || loop_contains_continue(false_then) { return true },
            ir::Stmt::Loop { .. } | ir::Stmt::For { .. } => {},
            _ => {}
        }
    }

    false
}

pub fn loops_to_fors(block: &mut ir::Block) {
    let mut i = 0;
    while i < block.len() {
        match block.at_mut(i) {
            ir::Stmt::Loop { code } => {
                if
                    let Some(ir::Stmt::If { true_then, false_then, .. }) = code.non_nop_first() &&
                    false_then.is_empty() && true_then.non_nop_len() == 1 &&
                    let Some(ir::Stmt::Break) = true_then.non_nop_first() &&
                    !loop_contains_continue(code) &&
                    let Some(ir::Stmt::Assign { .. }) = code.non_nop_last()
                {
                    let Some(ir::Stmt::If { cond, .. }) = code.pop_non_nop_first() else {
                        unreachable!()
                    };

                    let inc = Box::new(code.pop_non_nop_last().unwrap());

                    let new_code = code.get_mut().drain(1..).collect::<Vec<_>>();

                    *block.at_mut(i) = ir::Stmt::For {
                        inc,
                        guard: cond.neg(),
                        code: ir::Block::new_from(new_code)
                    };
                } else {
                    i += 1;
                }
            }
            _ => i += 1
        }
    }
}

fn gotos_to_for_continue_with(block: &mut ir::Block, end_labels: &[String]) {
    for stmt in block.get_mut() {
        match stmt {
            ir::Stmt::Branch { target, .. } if end_labels.contains(&target.0) => {
                *stmt = ir::Stmt::Continue;
            }
            ir::Stmt::If { true_then, false_then, .. } => {
                gotos_to_for_continue_with(true_then, end_labels);
                gotos_to_for_continue_with(false_then, end_labels);
            }
            ir::Stmt::Loop { code } => gotos_to_for_continue_with(code, &[]),
            ir::Stmt::For { code, .. } =>
                gotos_to_for_continue_with(
                    code, 
                    &code.labels_back_from(code.len() - 1, &ir::ScopeLabels::empty()).iter()
                        .filter_map(|x| x.upgrade().map(|x| x.0.clone()))
                        .collect::<Vec<_>>()
                ),
            _ => {}
        }
    }
}

pub fn gotos_to_for_continue(block: &mut ir::Block) {
    gotos_to_for_continue_with(block, &[])
}
