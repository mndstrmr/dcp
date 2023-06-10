use std::collections::{HashMap, HashSet};

use crate::{mir::{self, MirVisitorMut, MirFunc}, lir, expr};

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
            mir::Mir::Branch { .. } | mir::Mir::Label(_) | mir::Mir::Do(_) => {}
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

pub fn insert_loops(code: &mut MirFunc) {
    insert_loops_without(&mut code.code, &mut HashSet::new())
}

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

pub fn gotos_to_loop_continues(code: &mut MirFunc) {
    GotoToContinueVisitor { loop_start: HashSet::new() }.visit_block(&mut code.code)
}


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

pub fn step_back_breaks(code: &mut mir::MirFunc) {
    BreakStepBackVisitor.visit_block(&mut code.code)
}

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

pub fn gotos_to_loop_breaks(code: &mut mir::MirFunc) {
    GotoToLoopBreak { end: HashSet::new() }.visit_block(&mut code.code)
}

struct LoopStartLabelSwap;

impl MirVisitorMut for LoopStartLabelSwap {
    fn visit_loop(&mut self, code: &mut Vec<mir::Mir>) -> mir::MVMAction {
        let mut i = 0;
        while let Some(mir::Mir::Label(_)) = code.get(i) {
            i += 1;
        }

        let start: Vec<_> = code.drain(0..i).collect();
        code.extend(start);

        mir::MVMAction::Keep
    }
}

pub fn loop_start_label_swap(code: &mut mir::MirFunc) {
    LoopStartLabelSwap.visit_block(&mut code.code)
}

struct InfLoopUnreachable;

impl MirVisitorMut for InfLoopUnreachable {
    fn visit_block(&mut self, code: &mut Vec<mir::Mir>) {
        let mut i = 0;
        while i < code.len() {
            self.visit(&mut code[i]);

            if let mir::Mir::Loop { code: inner } = &mut code[i] {
                if mir::break_count(inner) > 0 {
                    i += 1;
                    continue;
                }

                let mut j = i + 1;
                while j < code.len() {
                    if let mir::Mir::Label(_) = &code[j] {
                        break
                    };

                    j += 1;
                }

                code.drain(i + 1..j);
                i += 1;
            } else {
                i += 1;
            }
        }
    }
}

pub fn inf_loops_unreachable(code: &mut mir::MirFunc) {
    InfLoopUnreachable.visit_block(&mut code.code)
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
            mir::Mir::Label(_) | mir::Mir::Break | mir::Mir::Continue | mir::Mir::Do(_) => {
                is_end = false;
            }
        }
    }
}

pub fn final_continues(code: &mut mir::MirFunc) {
    final_continues_with(&mut code.code, false)
}


struct TerminatingToBreak;

impl MirVisitorMut for TerminatingToBreak {
    fn visit_loop(&mut self, code: &mut Vec<mir::Mir>) -> mir::MVMAction {
        self.visit_block(code);

        let break_count = mir::break_count(&code);

        let Some(mir::Mir::If { true_then, false_then, cond }) = code.first_mut() else {
            return mir::MVMAction::Keep
        };

        if true_then.len() <= 1 || !false_then.is_empty() {
            return mir::MVMAction::Keep
        }

        match true_then.last().unwrap() {
            mir::Mir::Break => {
                if break_count != 1 {
                    return mir::MVMAction::Keep
                }

                let mut replacement: Vec<_> = true_then.drain(..true_then.len() - 1).collect();
                let mut new_body = vec![
                    mir::Mir::If {
                        cond: cond.take(),
                        true_then: vec![mir::Mir::Break],
                        false_then: vec![]
                    }
                ];
                new_body.extend(code.drain(1..));
                replacement.insert(0, mir::Mir::Loop { code: new_body });
                mir::MVMAction::ReplaceMany(replacement)
            }
            mir::Mir::Return(_) => {
                if break_count != 0 {
                    return mir::MVMAction::Keep
                }

                let mut replacement: Vec<_> = true_then.drain(..).collect();
                let mut new_body = vec![
                    mir::Mir::If {
                        cond: cond.take(),
                        true_then: vec![mir::Mir::Break],
                        false_then: vec![]
                    }
                ];
                new_body.extend(code.drain(1..));
                replacement.insert(0, mir::Mir::Loop { code: new_body });
                mir::MVMAction::ReplaceMany(replacement)
            },
            _ => mir::MVMAction::Keep
        }
    }
}

pub fn terminating_to_break(code: &mut mir::MirFunc) {
    TerminatingToBreak.visit_block(&mut code.code)
}
