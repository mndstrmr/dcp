use std::collections::HashSet;

use crate::{expr, lir};

#[derive(Clone, Debug)]
pub enum Mir {
    Assign {
        src: expr::Expr,
        dst: expr::Expr
    },
    Return(expr::Expr),
    Branch {
        cond: Option<expr::Expr>,
        target: lir::Label
    },
    Label(lir::Label),
    If {
        cond: expr::Expr,
        true_then: Vec<Mir>,
        false_then: Vec<Mir>,
    },
    Loop {
        code: Vec<Mir>,
    },
    While {
        guard: expr::Expr,
        code: Vec<Mir>,
    },
    For {
        guard: expr::Expr,
        inc: Vec<Mir>,
        code: Vec<Mir>,
    },
    Break,
    Continue
}

impl Mir {
    pub fn terminating(&self) -> bool {
        match self {
            Mir::Branch { cond: None, .. } | Mir::Break | Mir::Continue | Mir::Return(_) => true,
            _ => false
        }
    }
}

pub struct MirBlock {
    pub code: Vec<Mir>
}

impl MirBlock {
    pub fn new() -> MirBlock {
        MirBlock {
            code: Vec::new()
        }
    }
}

impl std::fmt::Display for MirBlock {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "block {{")?;
        for stmt in &self.code {
            f.write_str(&format!("\n{}", stmt).replace('\n', crate::NEWLINE_INDENT))?;
        }
        writeln!(f, "\n}}")?;

        Ok(())
    }
}

impl From<lir::Lir> for Mir {
    fn from(value: lir::Lir) -> Self {
        match value {
            lir::Lir::Assign { src, dst } => Mir::Assign { src, dst},
            lir::Lir::Branch { cond, target } => Mir::Branch { cond, target },
            lir::Lir::Return(expr) => Mir::Return(expr),
            lir::Lir::Label(label) => Mir::Label(label)
        }
    }
}

impl std::fmt::Display for Mir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mir::Assign { src, dst } => write!(f, "{dst} = {src}"),
            Mir::Return(expr) => write!(f, "return {expr}"),
            Mir::Branch { cond: Some(cond), target } => write!(f, "ifgoto {cond} #{target}"),
            Mir::Branch { cond: None, target } => write!(f, "goto #{target}"),
            Mir::If { cond, true_then, false_then } => {
                write!(f, "if {} {{", cond)?;
                for stmt in true_then {
                    f.write_str(&format!("\n{}", stmt).replace('\n', crate::NEWLINE_INDENT))?;
                }
                write!(f, "\n}}")?;

                if !false_then.is_empty() {
                    write!(f, " else {{")?;
                    for stmt in false_then {
                        f.write_str(&format!("\n{}", stmt).replace('\n', crate::NEWLINE_INDENT))?;
                    }
                    write!(f, "\n}}")?;
                }

                Ok(())
            }
            Mir::For { guard, inc, code } => {
                // FIXME: Multistatement increment
                write!(f, "for {}; {} {{", guard, inc.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(";"))?;
                for stmt in code {
                    f.write_str(&format!("\n{}", stmt).replace('\n', crate::NEWLINE_INDENT))?;
                }
                write!(f, "\n}}")?;

                Ok(())
            }
            Mir::While { guard, code } => {
                // FIXME: Multistatement increment
                write!(f, "while {} {{", guard)?;
                for stmt in code {
                    f.write_str(&format!("\n{}", stmt).replace('\n', crate::NEWLINE_INDENT))?;
                }
                write!(f, "\n}}")?;

                Ok(())
            }
            Mir::Loop { code } => {
                write!(f, "loop {{")?;
                for stmt in code {
                    f.write_str(&format!("\n{}", stmt).replace('\n', crate::NEWLINE_INDENT))?;
                }
                write!(f, "\n}}")
            }
            Mir::Break => write!(f, "break"),
            Mir::Continue => write!(f, "continue"),
            Mir::Label(label) => write!(f, "{label}:")
        }
    }
}

impl Mir {
    pub fn append_used_labels(&self, labels: &mut HashSet<lir::Label>) {
        match self {
            Mir::Break | Mir::Continue | Mir::Return(_) | Mir::Assign { .. } | Mir::Label(_) => {},
            Mir::Branch { target, .. } => {
                labels.insert(*target);
            }
            Mir::If { true_then, false_then, .. } => {
                append_used_labels(true_then, labels);
                append_used_labels(false_then, labels);
            }
            Mir::Loop { code } => {
                append_used_labels(code, labels);
            }
            Mir::While { code, .. } => {
                append_used_labels(code, labels);
            }
            Mir::For { inc, code, .. } => {
                append_used_labels(inc, labels);
                append_used_labels(code, labels);
            }
        }
    }
}

fn append_used_labels(code: &[Mir], labels: &mut HashSet<lir::Label>) {
    for stmt in code {
        stmt.append_used_labels(labels);
    }
}

pub fn used_labels(code: &[Mir]) -> HashSet<lir::Label> {
    let mut labels = HashSet::new();
    append_used_labels(code, &mut labels);
    labels
}

pub fn defined_labels(code: &[Mir]) -> HashSet<lir::Label> {
    let mut labels = HashSet::new();
    append_labels(code, &mut labels);
    labels
}

pub fn append_labels(code: &[Mir], labels: &mut HashSet<lir::Label>) {
    for stmt in code {
        match stmt {
            Mir::Label(label) => {
                labels.insert(*label);
            }
            Mir::For { inc, code, .. } => {
                append_labels(inc, labels);
                append_labels(code, labels);
            }
            Mir::Loop { code } => append_labels(code, labels),
            Mir::While { code, .. } => append_labels(code, labels),
            Mir::If { true_then, false_then, .. } => {
                append_labels(true_then, labels);
                append_labels(false_then, labels);
            }
            Mir::Assign { .. } | Mir::Return(_) | Mir::Branch { .. } | Mir::Break | Mir::Continue => {}
        }
    }
}

fn trim_labels_in(code: &mut Vec<Mir>, used: &HashSet<lir::Label>) {
    let mut i = 0;
    while i < code.len() {
        i += 1;
        match &mut code[i - 1] {
            Mir::Break | Mir::Continue | Mir::Return(_) | Mir::Assign { .. } | Mir::Branch { .. } => {},
            Mir::Label(label) if !used.contains(label) => {
                code.remove(i - 1);
                i -= 1;
            }
            Mir::Label(_) => {}
            Mir::If { true_then, false_then, .. } => {
                trim_labels_in(true_then, used);
                trim_labels_in(false_then, used);
            }
            Mir::Loop { code } => {
                trim_labels_in(code, used);
            }
            Mir::While { code, .. } => {
                trim_labels_in(code, used);
            }
            Mir::For { inc, code, .. } => {
                trim_labels_in(inc, used);
                trim_labels_in(code, used);
            }
        }
    }
}

pub fn trim_labels(code: &mut Vec<Mir>) {
    trim_labels_in(code, &used_labels(code));
}

pub fn compress_control_flow(code: &mut Vec<Mir>) {
    let mut i = 0;
    while i < code.len() {
        i += 1;
        match &mut code[i - 1] {
            Mir::Break | Mir::Continue | Mir::Return(_) | Mir::Assign { .. } | Mir::Label(_) => {},
            Mir::Branch { target, .. } => {
                let target = *target;

                let mut j = i;
                while j < code.len() {
                    match &code[j] {
                        Mir::Label(label) if *label == target => {
                            code.remove(i - 1);
                            i = 0; // Redo
                            break
                        }
                        Mir::Label(_) => j += 1,
                        _ => break
                    }
                }
            }
            Mir::If { true_then, false_then, .. } => {
                compress_control_flow(true_then);
                compress_control_flow(false_then);
            }
            Mir::Loop { code } => {
                compress_control_flow(code);
            }
            Mir::While { code, .. } => {
                compress_control_flow(code);
            }
            Mir::For { inc, code, .. } => {
                compress_control_flow(inc);
                compress_control_flow(code);
            }
        }
    }
}

fn cull_fallthrough_jumps_with_end_scope(code: &mut Vec<Mir>, end: &HashSet<lir::Label>) {
    while let Some(Mir::Branch { target, .. }) = code.last() && end.contains(target) {
        code.pop();
    }

    let mut i = 0;
    while i < code.len() {
        match &mut code[i] {
            Mir::If { .. } => {
                let mut new = end.clone();

                let mut j = i + 1;
                while let Some(Mir::Label(label)) = code.get(j) {
                    new.insert(*label);
                    j += 1;
                }
        
                let Mir::If { true_then, false_then, .. } = &mut code[i] else {
                    unreachable!()
                };
                
                cull_fallthrough_jumps_with_end_scope(true_then, &new);
                cull_fallthrough_jumps_with_end_scope(false_then, &new);
            }
            Mir::Loop { code } | Mir::While { code, .. } => {
                cull_fallthrough_jumps_with_end_scope(code, &HashSet::new());
            }
            Mir::For { inc, code, .. } => {
                cull_fallthrough_jumps_with_end_scope(inc, &HashSet::new());
                cull_fallthrough_jumps_with_end_scope(code, &HashSet::new());
            }
            Mir::Assign { .. } |  Mir::Branch { .. } | Mir::Return(_) |
            Mir::Label(_) | Mir::Break | Mir::Continue => {}
        }

        i += 1;
    }
}

pub fn cull_fallthrough_jumps(code: &mut Vec<Mir>) {
    cull_fallthrough_jumps_with_end_scope(code, &HashSet::new());
}

pub fn collapse_cmp(code: &mut Vec<Mir>) {
    for stmt in code {
        match stmt {
            Mir::Assign { src, dst } => {
                src.collapse_cmp();
                dst.collapse_cmp();
            }
            Mir::Branch { cond: Some(cond), .. } => cond.collapse_cmp(),
            Mir::Return(x) => x.collapse_cmp(),
            Mir::If { cond, true_then, false_then } => {
                cond.collapse_cmp();
                collapse_cmp(true_then);
                collapse_cmp(false_then);
            }
            Mir::Loop { code } => collapse_cmp(code),
            Mir::While { guard, code } => {
                guard.collapse_cmp();
                collapse_cmp(code);
            }
            Mir::For { guard, inc, code } => {
                guard.collapse_cmp();
                collapse_cmp(inc);
                collapse_cmp(code);
            }
            Mir::Break | Mir::Continue | Mir::Label(_) | Mir::Branch { cond: None, .. } => {}
        }
    }
}
