use std::collections::HashSet;

use crate::{expr, lir};

#[derive(Clone, Debug)]
pub enum Mir {
    Assign {
        src: expr::Expr,
        dst: expr::Expr
    },
    Return(expr::Expr),
    Do(expr::Expr),
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

pub struct MirLocal {
    pub offset: u64,
    pub size: u64,
    pub name: String
}

pub struct MirStackFrame {
    size: u64,
    locals: Vec<MirLocal>
}

impl MirStackFrame {
    pub fn new() -> MirStackFrame {
        MirStackFrame {
            size: 0,
            locals: vec![]
        }
    }

    pub fn get_at(&self, offset: u64) -> Option<&MirLocal> {
        match self.locals.binary_search_by(|x| x.offset.cmp(&offset)) {
            Ok(idx) => self.locals.get(idx),
            Err(_) => None
        }
    }

    pub fn get_by_name(&self, name: &str) -> Option<&MirLocal> {
        self.locals.iter().find(|x| x.name == name)
    }

    pub fn get_mut_by_name(&mut self, name: &str) -> Option<&mut MirLocal> {
        self.locals.iter_mut().find(|x| x.name == name)
    }

    pub fn insert(&mut self, local: MirLocal) {
        match self.locals.binary_search_by(|x| x.offset.cmp(&local.offset)) {
            Ok(_) => panic!("Already exists"),
            Err(idx) => {
                self.locals.insert(idx, local);
            }
        }
    }
}

pub struct MirFunc {
    pub args: Vec<&'static str>,
    pub results: Vec<&'static str>,
    pub code: Vec<Mir>,
    pub stack_frame: MirStackFrame
}

impl MirFunc {
    pub fn new(args: Vec<&'static str>, results: Vec<&'static str>, code: Vec<Mir>, stack_frame: MirStackFrame) -> MirFunc {
        MirFunc {
            args, results,
            code,
            stack_frame
        }
    }
}

impl std::fmt::Display for MirFunc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "func {{")?;
        write!(f, "{}frame {} {{", crate::NEWLINE_INDENT, self.stack_frame.size)?;
        for local in &self.stack_frame.locals {
            write!(f, "\n{}{}var {}: {} bytes @ base - {}", crate::INDENT, crate::INDENT, local.name, local.size, local.offset)?;
        }
        write!(f, "{}}}", crate::NEWLINE_INDENT)?;

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
            lir::Lir::Do(expr) => Mir::Do(expr),
            lir::Lir::Label(label) => Mir::Label(label)
        }
    }
}

impl std::fmt::Display for Mir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mir::Assign { src, dst } => write!(f, "{dst} = {src}"),
            Mir::Return(expr) => write!(f, "return {expr}"),
            Mir::Do(expr) => write!(f, "{expr}"),
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

pub trait MirVisitor {
    fn visit_block(&mut self, code: &[Mir]) {
        for stmt in code {
            self.visit(stmt);
        }
    }

    fn visit(&mut self, stmt: &Mir) {
        match stmt {
            Mir::Break => self.visit_break(),
            Mir::Continue => self.visit_continue(),
            Mir::Return(expr) => self.visit_return(expr),
            Mir::Do(expr) => self.visit_do(expr),
            Mir::Assign { src, dst } => self.visit_assign(dst, src),
            Mir::Label(label) => self.visit_label(*label),
            Mir::Branch { cond, target } => self.visit_branch(cond.as_ref(), *target),
            Mir::If { cond, true_then, false_then } => self.visit_if(cond, true_then, false_then),
            Mir::Loop { code } => self.visit_loop(code),
            Mir::While { guard, code } => self.visit_while(guard, code),
            Mir::For { guard, inc, code } => self.visit_for(guard, inc, code),
        }
    }

    fn visit_expr(&mut self, _expr: &expr::Expr) {}

    fn visit_break(&mut self) {}
    fn visit_continue(&mut self) {}
    fn visit_return(&mut self, expr: &expr::Expr) {
        self.visit_expr(expr);
    }
    fn visit_do(&mut self, expr: &expr::Expr) {
        self.visit_expr(expr);
    }
    fn visit_assign(&mut self, dst: &expr::Expr, src: &expr::Expr) {
        self.visit_expr(dst);
        self.visit_expr(src);
    }
    fn visit_label(&mut self, _label: lir::Label) {}
    fn visit_branch(&mut self, cond: Option<&expr::Expr>, _target: lir::Label) {
        if let Some(cond) = cond {
            self.visit_expr(cond);
        }
    }
    
    fn visit_if(&mut self, cond: &expr::Expr, true_then: &[Mir], false_then: &[Mir]) {
        self.visit_expr(cond);
        self.visit_block(true_then);
        self.visit_block(false_then);
    }

    fn visit_loop(&mut self, code: &[Mir]) {
        self.visit_block(code);
    }

    fn visit_while(&mut self, guard: &expr::Expr, code: &[Mir]) {
        self.visit_expr(guard);
        self.visit_block(code);
    }

    fn visit_for(&mut self, guard: &expr::Expr, inc: &[Mir], code: &[Mir]) {
        self.visit_expr(guard);
        self.visit_block(inc);
        self.visit_block(code);
    }
}

pub enum MVMAction {
    Keep,
    Remove,
    Replace(Mir),
    ReplaceSkip(Mir),
    ReplaceMany(Vec<Mir>)
}

pub trait MirVisitorMut {
    fn pre_block_visit(&mut self, _code: &mut Vec<Mir>) {}

    fn visit_block(&mut self, code: &mut Vec<Mir>) {
        self.pre_block_visit(code);

        let mut i = 0;
        while i < code.len() {
            match self.visit(&mut code[i]) {
                MVMAction::Keep => i += 1,
                MVMAction::Remove => {
                    code.remove(i);
                }
                MVMAction::Replace(new) => {
                    code[i] = new;
                }
                MVMAction::ReplaceSkip(new) => {
                    code[i] = new;
                    i += 1;
                }
                MVMAction::ReplaceMany(new) => {
                    code.remove(i);
                    for (j, x) in new.into_iter().enumerate() {
                        code.insert(i + j, x);
                    }
                }
            }
        }
    }

    fn visit(&mut self, stmt: &mut Mir) -> MVMAction {
        match stmt {
            Mir::Break => self.visit_break(),
            Mir::Continue => self.visit_continue(),
            Mir::Return(expr) => self.visit_return(expr),
            Mir::Do(expr) => self.visit_do(expr),
            Mir::Assign { src, dst } => self.visit_assign(dst, src),
            Mir::Label(label) => self.visit_label(*label),
            Mir::Branch { cond, target } => self.visit_branch(cond.as_mut(), *target),
            Mir::If { cond, true_then, false_then } => self.visit_if(cond, true_then, false_then),
            Mir::Loop { code } => self.visit_loop(code),
            Mir::While { guard, code } => self.visit_while(guard, code),
            Mir::For { guard, inc, code } => self.visit_for(guard, inc, code),
        }
    }

    fn visit_expr(&mut self, _expr: &mut expr::Expr) {}

    fn visit_break(&mut self) -> MVMAction { MVMAction::Keep }
    fn visit_continue(&mut self) -> MVMAction { MVMAction::Keep }
    fn visit_return(&mut self, expr: &mut expr::Expr) -> MVMAction {
        self.visit_expr(expr);
        MVMAction::Keep
    }
    fn visit_do(&mut self, expr: &mut expr::Expr) -> MVMAction {
        self.visit_expr(expr);
        MVMAction::Keep
    }
    fn visit_assign(&mut self, dst: &mut expr::Expr, src: &mut expr::Expr) -> MVMAction {
        self.visit_expr(dst);
        self.visit_expr(src);
        MVMAction::Keep
    }
    fn visit_label(&mut self, _label: lir::Label) -> MVMAction { MVMAction::Keep }
    fn visit_branch(&mut self, cond: Option<&mut expr::Expr>, _target: lir::Label) -> MVMAction {
        if let Some(cond) = cond {
            self.visit_expr(cond);
        }
        MVMAction::Keep
    }
    
    fn visit_if(&mut self, cond: &mut expr::Expr, true_then: &mut Vec<Mir>, false_then: &mut Vec<Mir>) -> MVMAction {
        self.visit_expr(cond);
        self.visit_block(true_then);
        self.visit_block(false_then);
        MVMAction::Keep
    }

    fn visit_loop(&mut self, code: &mut Vec<Mir>) -> MVMAction {
        self.visit_block(code);
        MVMAction::Keep
    }

    fn visit_while(&mut self, guard: &mut expr::Expr, code: &mut Vec<Mir>) -> MVMAction {
        self.visit_expr(guard);
        self.visit_block(code);
        MVMAction::Keep
    }

    fn visit_for(&mut self, guard: &mut expr::Expr, inc: &mut Vec<Mir>, code: &mut Vec<Mir>) -> MVMAction {
        self.visit_expr(guard);
        self.visit_block(inc);
        self.visit_block(code);
        MVMAction::Keep
    }
}

pub fn used_labels(code: &[Mir]) -> HashSet<lir::Label> {
    struct UsedLabelVisitor {
        labels: HashSet<lir::Label>
    }

    impl MirVisitor for UsedLabelVisitor {
        fn visit_branch(&mut self, _cond: Option<&expr::Expr>, target: lir::Label) {
            self.labels.insert(target);
        }
    }

    let mut visitor = UsedLabelVisitor { labels: HashSet::new() };
    visitor.visit_block(code);
    visitor.labels
}

pub fn defined_labels(code: &[Mir]) -> HashSet<lir::Label> {
    struct DefLabelVisitor {
        labels: HashSet<lir::Label>
    }

    impl MirVisitor for DefLabelVisitor {
        fn visit_label(&mut self, label: lir::Label) {
            self.labels.insert(label);
        }
    }

    let mut visitor = DefLabelVisitor { labels: HashSet::new() };
    visitor.visit_block(code);
    visitor.labels
}

pub fn contains_continue(code: &[Mir]) -> bool {
    struct ContainsContinue(bool);
    
    impl MirVisitor for ContainsContinue {
        fn visit_block(&mut self, code: &[Mir]) {
            let mut i = 0;
            while i < code.len() && !self.0 {
                self.visit(&code[i]);
                i += 1;
            }
        }

        fn visit_continue(&mut self) {
            self.0 = true;
        }

        fn visit_loop(&mut self, _: &[Mir]) {}
        fn visit_while(&mut self, _: &expr::Expr, _: &[Mir]) {}
        fn visit_for(&mut self, _: &expr::Expr, _: &[Mir], _: &[Mir]) {}
    }

    let mut visitor = ContainsContinue(false);
    visitor.visit_block(code);
    visitor.0
}

pub fn break_count(code: &[Mir]) -> usize {
    struct BreakCount(usize);
    
    impl MirVisitor for BreakCount {
        fn visit_block(&mut self, code: &[Mir]) {
            let mut i = 0;
            while i < code.len() {
                self.visit(&code[i]);
                i += 1;
            }
        }

        fn visit_break(&mut self) {
            self.0 += 1;
        }

        fn visit_loop(&mut self, _: &[Mir]) {}
        fn visit_while(&mut self, _: &expr::Expr, _: &[Mir]) {}
        fn visit_for(&mut self, _: &expr::Expr, _: &[Mir], _: &[Mir]) {}
    }

    let mut visitor = BreakCount(0);
    visitor.visit_block(code);
    visitor.0
}
