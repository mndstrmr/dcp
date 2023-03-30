use crate::expr;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Label(pub usize);

impl std::fmt::Display for Label {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub enum Lir {
    Branch {
        cond: Option<expr::Expr>,
        target: Label
    },
    Assign {
        src: expr::Expr,
        dst: expr::Expr
    },
    Label(Label),
    Return(expr::Expr)
}

impl Lir {
    pub fn count_reads(&self, name: &str) -> usize {
        match self {
            Lir::Return(expr) => expr.count_reads(name),
            Lir::Assign { dst: expr::Expr::Name(_), src } => src.count_reads(name),
            Lir::Assign { src, dst } => src.count_reads(name) + dst.count_reads(name),
            Lir::Branch { cond: Some(cond), .. } => cond.count_reads(name),
            Lir::Branch { .. } => 0,
            Lir::Label(_) => 0
        }
    }

    pub fn replace_name(&mut self, name: &str, expr: &expr::Expr) {
        match self {
            Lir::Return(ret) => ret.replace_name(name, expr),
            Lir::Assign { dst: expr::Expr::Name(_), src } => src.replace_name(name, expr),
            Lir::Assign { src, dst } => {
                src.replace_name(name, expr);
                dst.replace_name(name, expr);
            },
            Lir::Branch { cond: Some(cond), .. } => cond.replace_name(name, expr),
            Lir::Branch { .. } => (),
            Lir::Label(_) => ()
        }
    }
}

impl std::fmt::Display for Lir {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Lir::Branch { cond: Some(cond), target } => write!(f, "ifgoto {cond} {target}"),
            Lir::Branch { cond: None, target } => write!(f, "goto {target}"),
            Lir::Return(expr) => write!(f, "return {expr}"),
            Lir::Label(label) => write!(f, "{label}:"),
            Lir::Assign { src, dst } => write!(f, "{dst} = {src}")
        }
    }
}

pub struct LabelAllocator {
    next: usize
}

impl LabelAllocator {
    pub fn new() -> LabelAllocator {
        LabelAllocator {
            next: 0
        }
    }

    pub fn next(&mut self) -> Label {
        self.next += 1;
        Label(self.next - 1)
    }
}

pub type Index = usize;

pub struct LirFuncBuilder {
    code: Vec<Lir>,
    label_alloc: LabelAllocator
}

impl LirFuncBuilder {
    pub fn new() -> LirFuncBuilder {
        LirFuncBuilder {
            code: Vec::new(),
            label_alloc: LabelAllocator::new()
        }
    }

    pub fn new_label(&mut self) -> Label {
        self.label_alloc.next()
    }

    pub fn push(&mut self, code: Lir) {
        self.code.push(code);
    }

    pub fn block(self) -> LirFunc {
        LirFunc {
            code: self.code,
        }
    }
}

#[derive(Debug)]
pub struct LirNode {
    pub code: Vec<Lir>,
}

#[derive(Debug)]
pub struct LirFunc {
    code: Vec<Lir>,
}

impl LirFunc {
    pub fn new() -> LirFunc {
        LirFunc {
            code: Vec::new(),
        }
    }

    pub fn len(&self) -> usize {
        self.code.len()
    }

    pub fn get_mut(&mut self) -> &mut Vec<Lir> {
        &mut self.code
    }

    pub fn get(&self) -> &[Lir] {
        &self.code
    }

    pub fn at(&self, idx: usize) -> Option<&Lir> {
        self.code.get(idx)
    }
}

impl std::fmt::Display for LirFunc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for stmt in &self.code {
            writeln!(f, "    {}", stmt)?;
        }

        Ok(())
    }
}
