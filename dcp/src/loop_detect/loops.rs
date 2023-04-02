use std::collections::{HashMap, HashSet};

use crate::{mir::{self, MirVisitorMut}, lir, expr};

fn insert_loops_without(code: &mut Vec<mir::Mir>, ignore: &mut HashSet<lir::Label>) {
    let mut loops: HashMap<lir::Label, usize> = HashMap::new();
    let mut i = code.len();

    'outer: while i > 0 {
        i -= 1;

        match &mut code[i] {
            mir::Mir::If { true_then, false_then, .. } => {
                insert_loops_without(true_then, ignore);
                insert_loops_without(false_then, ignore);
            }
            mir::Mir::Loop { code } | mir::Mir::While { code, .. } => {
                insert_loops_without(code, ignore);
            }
            mir::Mir::For { inc, code, .. } => {
                insert_loops_without(code, ignore);
                insert_loops_without(inc, ignore);
            }
            mir::Mir::Assign { .. } | mir::Mir::Break | mir::Mir::Continue | mir::Mir::Return(_) |
            mir::Mir::Branch { .. } | mir::Mir::Label(_) => {}
        }

        for defined in mir::defined_labels(&code[i..i + 1]) {
            if let Some(end) = loops.get(&defined) && !ignore.contains(&defined) {
                let mut chunk = code.drain(i..=*end).collect::<Vec<_>>();
                chunk.push(mir::Mir::Break);
                code.insert(i, mir::Mir::Loop { code: chunk });

                ignore.insert(defined);
                
                i = code.len();
                loops.drain();
                
                continue 'outer;
            }
        }

        for label in mir::used_labels(&code[i..i + 1]) {
            loops.entry(label).or_insert(i);
        }
    }
}

pub fn insert_loops(code: &mut Vec<mir::Mir>) {
    insert_loops_without(code, &mut HashSet::new())
}

pub fn gotos_to_loop_continues(code: &mut Vec<mir::Mir>) {
    struct GotoToContinueVisitor {
        loop_start: HashSet<lir::Label>
    }

    impl MirVisitorMut for GotoToContinueVisitor {
        fn visit_loop(&mut self, code: &mut Vec<mir::Mir>) -> mir::MVMAction {
            let mut labels = HashSet::new();
            let mut i = 0;
            while i < code.len() {
                let mir::Mir::Label(label) = &code[i] else {
                    break
                };

                labels.insert(*label);
                i += 1;
            }

            std::mem::swap(&mut labels, &mut self.loop_start);
            self.visit_block(code);
            self.loop_start = labels;
            
            mir::MVMAction::Keep
        }

        fn visit_branch(&mut self, cond: Option<&mut expr::Expr>, target: lir::Label) -> mir::MVMAction {
            if !self.loop_start.contains(&target) {
                return mir::MVMAction::Keep
            }

            if let Some(cond) = cond {
                mir::MVMAction::ReplaceSkip(mir::Mir::If { true_then: vec![mir::Mir::Continue], false_then: vec![], cond: cond.take() })
            } else {
                mir::MVMAction::ReplaceSkip(mir::Mir::Continue)
            }
        }
    }

    GotoToContinueVisitor { loop_start: HashSet::new() }.visit_block(code)
}


pub fn step_back_breaks(code: &mut Vec<mir::Mir>) {
    struct BreakStepBackVisitor;

    impl MirVisitorMut for BreakStepBackVisitor {
        fn pre_block_visit(&mut self, code: &mut Vec<mir::Mir>) {
            let mut i = 0;
            while i < code.len() {
                let Some(mir::Mir::Break) = code.get(i + 1) else {
                    i += 1;
                    continue
                };

                let Some(mir::Mir::If { true_then, false_then, .. }) = code.get_mut(i) else {
                    i += 1;
                    continue
                };
            
                if !true_then.last().map_or(false, mir::Mir::terminating) {
                    true_then.push(mir::Mir::Break);
                }

                if !false_then.last().map_or(false, mir::Mir::terminating) {
                    false_then.push(mir::Mir::Break);
                }

                code.remove(i + 1);
                i += 1;
            }
        }
    }

    BreakStepBackVisitor.visit_block(code)
}

pub fn gotos_to_loop_breaks(code: &mut Vec<mir::Mir>) {
    struct GotoToLoopBreak {
        end: HashSet<lir::Label>
    }

    impl MirVisitorMut for GotoToLoopBreak {
        fn visit_block(&mut self, code: &mut Vec<mir::Mir>) {
            let mut i = 0;
            while i < code.len() {
                if let mir::Mir::Loop { .. } = &mut code[i] {
                    let mut labels = HashSet::new();
                    let mut j = i + 1;
                    while j < code.len() {
                        let mir::Mir::Label(label) = &code[j] else {
                            break
                        };

                        labels.insert(*label);
                        j += 1;
                    }

                    std::mem::swap(&mut labels, &mut self.end);
                    self.visit(&mut code[i]);
                    self.end = labels;
                } else {
                    match self.visit(&mut code[i]) {
                        mir::MVMAction::ReplaceSkip(new) => code[i] = new,
                        _ => {}
                    }
                }

                i += 1;
            }
        }

        fn visit_branch(&mut self, cond: Option<&mut expr::Expr>, target: lir::Label) -> mir::MVMAction {
            if !self.end.contains(&target) {
                return mir::MVMAction::Keep
            }

            if let Some(cond) = cond {
                mir::MVMAction::ReplaceSkip(mir::Mir::If { true_then: vec![mir::Mir::Break], false_then: vec![], cond: cond.take() })
            } else {
                mir::MVMAction::ReplaceSkip(mir::Mir::Break)
            }
        }
    }

    GotoToLoopBreak { end: HashSet::new() }.visit_block(code)
}

pub fn final_continues(code: &mut Vec<mir::Mir>) {
    final_continues_with(code, false)
}

fn final_continues_with(code: &mut Vec<mir::Mir>, mut is_end: bool) {
    let mut i = code.len();
    while i > 0 {
        i -= 1;

        match &mut code[i] {
            mir::Mir::Continue if is_end => {
                code.remove(i);
            }
            mir::Mir::Loop { code } | mir::Mir::While { code, .. } | mir::Mir::For { code, .. } => {
                final_continues_with(code, true);
                is_end = false;
            }
            mir::Mir::If { true_then, false_then, .. } => {
                final_continues_with(true_then, is_end);
                final_continues_with(false_then, is_end);
                is_end = false;
            }
            mir::Mir::Assign { .. } | mir::Mir::Branch { .. } | mir::Mir::Return(_) |
            mir::Mir::Label(_) | mir::Mir::Break | mir::Mir::Continue => {
                is_end = false;
            }
        }
    }
}
