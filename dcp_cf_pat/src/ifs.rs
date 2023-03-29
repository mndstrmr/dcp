fn goto_to_ifs(code: &mut ir::Block, scope: &ir::ScopeLabels) {
    let mut iter = ir::BlindBlockIter::new();

    while let Some(stmt) = iter.next(code) {
        if
            let ir::Stmt::Branch { cond: Some(cond), target } = stmt &&
            let Some(target_idx) = code.find_label_flat(target, Some(scope)) &&
            target_idx > iter.offset() {

            let cond = cond.neg();

            let mut body = ir::Block::new_from(
                code.get_mut().drain(iter.offset() + 1..target_idx).collect()
            );

            let new_scope = ir::ScopeLabels::new(
                code.labels_back_from(iter.offset(), scope),
                code.labels_forward_from(iter.offset(), scope),
            );

            goto_to_ifs(&mut body, &new_scope);

            code.get_mut()[iter.offset()] = ir::Stmt::If {
                cond,
                true_then: body,
                false_then: ir::Block::empty()
            };
        }
    }
}

fn gotos_to_elses(code: &mut ir::Block, scope: &ir::ScopeLabels) -> bool {
    let mut iter = ir::BlindBlockIter::new();
    let mut changed = false;

    while let Some(stmt) = iter.next(code) {
        if let ir::Stmt::If { true_then, .. } = stmt {
            if
                let Some(ir::Stmt::Branch { cond: None, target }) = true_then.non_nop_last() &&
                let Some(mut target_idx) = code.find_label_flat(target, Some(scope)) &&
                target_idx > iter.offset() {

                target_idx = code.label_block_start(target_idx);

                let else_block = code.get_mut().drain(iter.offset() + 1..target_idx).collect::<Vec<_>>();

                match &mut code.get_mut()[iter.offset()] {
                    ir::Stmt::If { true_then, false_then, cond } => {
                        true_then.pop_non_nop_last();
                        if true_then.non_nop_len() == 0 {
                            *cond = cond.neg();
                            *true_then.get_mut() = else_block;
                        } else {
                            *false_then.get_mut() = else_block;
                        }
                    },
                    _ => unreachable!()
                }

                changed = true;
            }

            let scope = ir::ScopeLabels::new(
                code.labels_back_from(iter.offset(), scope),
                code.labels_forward_from(iter.offset(), scope),
            );

            match &mut code.get_mut()[iter.offset()] {
                ir::Stmt::If { true_then, false_then, .. } => {
                    while gotos_to_elses(true_then, &scope) {}
                    while gotos_to_elses(false_then, &scope) {}
                },
                _ => unreachable!()
            }
        }
    }

    changed
}

pub fn insert_ifs(code: &mut ir::Block) {
    goto_to_ifs(code, &ir::ScopeLabels::empty());
    while gotos_to_elses(code, &ir::ScopeLabels::empty()) {}
}
