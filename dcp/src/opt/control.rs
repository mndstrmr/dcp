use std::collections::HashSet;

use crate::{mir::{Mir, MirVisitorMut, MVMAction, self, MirFunc}, lir};

struct TrimLabelVisitor {
    used: HashSet<lir::Label>
}

impl MirVisitorMut for TrimLabelVisitor {
    fn visit_label(&mut self, label: lir::Label) -> MVMAction {
        if self.used.contains(&label) {
            MVMAction::Keep
        } else {
            MVMAction::Remove
        }
    }
}

pub fn trim_labels(block: &mut MirFunc) {
    let mut visitor = TrimLabelVisitor { used: mir::used_labels(&mut block.code) };
    visitor.visit_block(&mut block.code);
}

struct ControlFlowCompressVisitor;

impl MirVisitorMut for ControlFlowCompressVisitor {
    fn pre_block_visit(&mut self, code: &mut Vec<Mir>) {
        let mut i = 0;
        'outer: while i < code.len() {
            let Mir::Branch { target, .. } = &code[i] else {
                i += 1;
                continue
            };


            let mut j = i + 1;
            while j < code.len() {
                let Mir::Label(label) = &code[j] else {
                    break
                };

                if label == target {
                    code.remove(i);
                    i = 0;
                    continue 'outer
                }

                j += 1;
            }

            i += 1;
        }
    }
}

pub fn compress_control_flow(block: &mut MirFunc) {
    ControlFlowCompressVisitor.visit_block(&mut block.code)
}

struct UnreachableControlFlow;

impl MirVisitorMut for UnreachableControlFlow {
    fn pre_block_visit(&mut self, code: &mut Vec<Mir>) {
        for (s, stmt) in code.iter_mut().enumerate() {
            if stmt.terminating() {
                code.drain(s + 1..);
                break;
            }
        }
    }
}

pub fn elim_unreachable(block: &mut MirFunc) {
    UnreachableControlFlow.visit_block(&mut block.code)
}

fn cull_fallthrough_jumps_with_end_scope(code: &mut Vec<Mir>, end: Option<&HashSet<lir::Label>>) {
    if let Some(end) = end {
        while let Some(Mir::Branch { target, .. }) = code.last() && end.contains(target) {
            code.pop();
        }
    }

    let mut i = 0;
    while i < code.len() {
        match &mut code[i] {
            Mir::If { .. } => {
                let mut new =
                    if i == code.len() - 1 {
                        end.map_or_else(HashSet::new, HashSet::clone)
                    } else {
                        HashSet::new()
                    };

                let mut j = i + 1;
                while let Some(Mir::Label(label)) = code.get(j) {
                    new.insert(*label);
                    j += 1;
                }
        
                let Mir::If { true_then, false_then, .. } = &mut code[i] else {
                    unreachable!()
                };
                
                cull_fallthrough_jumps_with_end_scope(true_then, Some(&new));
                cull_fallthrough_jumps_with_end_scope(false_then, Some(&new));
            }
            Mir::Loop { code } | Mir::While { code, .. } => {
                cull_fallthrough_jumps_with_end_scope(code, None);
            }
            Mir::For { inc, code, .. } => {
                cull_fallthrough_jumps_with_end_scope(inc, None);
                cull_fallthrough_jumps_with_end_scope(code, None);
            }
            Mir::Assign { .. } |  Mir::Branch { .. } | Mir::Return(_) |
            Mir::Label(_) | Mir::Break | Mir::Continue | Mir::Do(_) => {}
        }

        i += 1;
    }
}

pub fn cull_fallthrough_jumps(block: &mut MirFunc) {
    cull_fallthrough_jumps_with_end_scope(&mut block.code, None);
}
