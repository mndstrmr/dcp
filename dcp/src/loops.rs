use std::collections::{HashMap, HashSet};

use crate::{mir, lir};

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
            mir::Mir::Assign { .. } | mir::Mir::Break | mir::Mir::Continue | mir::Mir::Return(_) | mir::Mir::Branch { .. } | mir::Mir::Label(_) => {}
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

fn gotos_to_loop_continues_with(code: &mut Vec<mir::Mir>, loop_start: &HashSet<lir::Label>) {
    for stmt in code {
        match stmt {
            mir::Mir::If { true_then, false_then, .. } => {
                gotos_to_loop_continues_with(true_then, loop_start);
                gotos_to_loop_continues_with(false_then, loop_start);
            }
            mir::Mir::Loop { code } => {
                let mut labels = HashSet::new();
                let mut i = 0;
                while i < code.len() {
                    let mir::Mir::Label(label) = &code[i] else {
                        break
                    };

                    labels.insert(*label);
                    i += 1;
                }

                gotos_to_loop_continues_with(code, &labels);
            }
            mir::Mir::While { code, .. } => gotos_to_loop_continues_with(code, &HashSet::new()),
            mir::Mir::For { inc, code, .. } => {
                gotos_to_loop_continues_with(inc, &HashSet::new());
                gotos_to_loop_continues_with(code, &HashSet::new());
            }
            mir::Mir::Branch { target, cond } if loop_start.contains(&target) => {
                if let Some(cond) = cond.take() {
                    *stmt = mir::Mir::If { true_then: vec![mir::Mir::Continue], false_then: vec![], cond };
                } else {
                    *stmt = mir::Mir::Continue;
                }
            }
            mir::Mir::Assign { .. } | mir::Mir::Break | mir::Mir::Continue | mir::Mir::Return(_) | mir::Mir::Branch { .. } | mir::Mir::Label(_) => {}
        }
    }
}

pub fn gotos_to_loop_continues(code: &mut Vec<mir::Mir>) {
    gotos_to_loop_continues_with(code, &HashSet::new())
}


pub fn step_back_breaks(code: &mut Vec<mir::Mir>) {
    let mut i = 0;
    while i < code.len() {
        match &mut code[i] {
            mir::Mir::If { true_then, false_then, .. } => {
                step_back_breaks(true_then);
                step_back_breaks(false_then);

                if let Some(mir::Mir::Break) = code.get(i + 1) {
                    let mir::Mir::If { true_then, false_then, .. } = &mut code[i] else {
                        unreachable!()
                    };

                    if !true_then.last().map_or(false, mir::Mir::terminating) {
                        true_then.push(mir::Mir::Break);
                    }

                    if !false_then.last().map_or(false, mir::Mir::terminating) {
                        false_then.push(mir::Mir::Break);
                    }

                    code.remove(i + 1);
                }
            }
            mir::Mir::Loop { code } | mir::Mir::While { code, .. } => {
                step_back_breaks(code);
            }
            mir::Mir::For { inc, code, .. } => {
                step_back_breaks(code);
                step_back_breaks(inc);
            }
            mir::Mir::Assign { .. } | mir::Mir::Break | mir::Mir::Continue | mir::Mir::Return(_) | mir::Mir::Branch { .. } | mir::Mir::Label(_) => {}
        }

        i += 1;
    }
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
            mir::Mir::Assign { .. } | mir::Mir::Branch { .. } | mir::Mir::Return(_) | mir::Mir::Label(_) | mir::Mir::Break | mir::Mir::Continue => {
                is_end = false;
            }
        }
    }
}
