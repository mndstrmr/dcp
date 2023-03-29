#![allow(dead_code)]

use std::{fmt::Display, rc::{Weak, Rc}};

const NEWLINE_INDENT: &'static str = "\n    ";

#[derive(Debug)]
pub struct Func {
    code: Block
}

impl Func {
    pub fn new() -> Func {
        Func {
            code: Block::empty()
        }
    }

    pub fn add(&mut self, stmt: Stmt) {
        self.code.add(stmt);
    }

    pub fn block(&self) -> &Block {
        &self.code
    }

    pub fn block_mut(&mut self) -> &mut Block {
        &mut self.code
    }

    pub fn take_block(self) -> Block {
        self.code
    }
}

impl Display for Func {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "func {{")?;
        for stmt in self.code.get() {
            if !stmt.invisible() {
                f.write_str(&format!("\n{}", stmt).replace('\n', NEWLINE_INDENT))?;
            }
        }
        writeln!(f, "\n}}")?;

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    CmpEq, CmpNe, CmpLt, CmpLe, CmpGt, CmpGe,
}

impl Display for UnaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            UnaryOp::Not => "!",
            UnaryOp::CmpEq => "eq",
            UnaryOp::CmpNe => "ne",
            UnaryOp::CmpLt => "lt",
            UnaryOp::CmpLe => "le",
            UnaryOp::CmpGt => "gt",
            UnaryOp::CmpGe => "ge",
        })
    }
}

impl UnaryOp {
    pub fn is_cmp(&self) -> bool {
        match self {
            UnaryOp::CmpEq | UnaryOp::CmpNe | UnaryOp::CmpLt |
            UnaryOp::CmpLe | UnaryOp::CmpGt | UnaryOp::CmpGe => true,
            _ => false
        }
    }

    pub fn cmp_op_to_binaryop(&self) -> BinaryOp {
        match self {
            UnaryOp::CmpEq => BinaryOp::Eq,
            UnaryOp::CmpNe => BinaryOp::Ne,
            UnaryOp::CmpLt => BinaryOp::Lt,
            UnaryOp::CmpLe => BinaryOp::Le,
            UnaryOp::CmpGt => BinaryOp::Gt,
            UnaryOp::CmpGe => BinaryOp::Ge,
            _ => panic!("Not a cmpop")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Eq, Ne, Lt, Le, Gt, Ge,
    Add, Sub, Mul, Div,
    And, Or,
    Cmp
}

impl Display for BinaryOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            BinaryOp::Eq => "==",
            BinaryOp::Ne => "!=",
            BinaryOp::Lt => "<",
            BinaryOp::Gt => ">",
            BinaryOp::Le => "<=",
            BinaryOp::Ge => ">=",
            BinaryOp::Add => "+",
            BinaryOp::Sub => "-",
            BinaryOp::Mul => "*",
            BinaryOp::Div => "/",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::Cmp => "cmp",
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Name(String),
    Num(i64),
    Bool(bool),
    Deref(Box<Expr>),
    Call {
        func: Box<Expr>,
        args: Vec<Expr>
    },
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
    }
}

impl Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Name(name) => write!(f, "{}", name),
            Expr::Num(num) => write!(f, "{}", num),
            Expr::Bool(b) => write!(f, "{}", b),
            Expr::Deref(expr) => write!(f, "*{}", expr),
            Expr::Unary { op, expr } if op.is_cmp() => write!(f, "{}.{}", expr, op),
            Expr::Unary { op, expr } => write!(f, "{}{}", op, expr),
            Expr::Binary { op, lhs, rhs } => write!(f, "({} {} {})", lhs, op, rhs),
            Expr::Call { func, args } => write!(f, "{}({})", func, args.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(", ")),
        }
    }
}

impl Expr {
    pub fn neg(&self) -> Expr {
        match self {
            Expr::Bool(val) => Expr::Bool(!val),
            
            Expr::Unary { op: UnaryOp::Not, expr } => expr.as_ref().clone(),
            Expr::Unary { op: UnaryOp::CmpEq, expr } => Expr::Unary { op: UnaryOp::CmpNe, expr: expr.clone() },
            Expr::Unary { op: UnaryOp::CmpNe, expr } => Expr::Unary { op: UnaryOp::CmpEq, expr: expr.clone() },
            Expr::Unary { op: UnaryOp::CmpLt, expr } => Expr::Unary { op: UnaryOp::CmpGe, expr: expr.clone() },
            Expr::Unary { op: UnaryOp::CmpGe, expr } => Expr::Unary { op: UnaryOp::CmpLt, expr: expr.clone() },
            Expr::Unary { op: UnaryOp::CmpLe, expr } => Expr::Unary { op: UnaryOp::CmpGt, expr: expr.clone() },
            Expr::Unary { op: UnaryOp::CmpGt, expr } => Expr::Unary { op: UnaryOp::CmpLe, expr: expr.clone() },
            
            Expr::Binary { op: BinaryOp::Eq, lhs, rhs } => Expr::Binary { op: BinaryOp::Ne, lhs: lhs.clone(), rhs: rhs.clone() },
            Expr::Binary { op: BinaryOp::Ne, lhs, rhs } => Expr::Binary { op: BinaryOp::Eq, lhs: lhs.clone(), rhs: rhs.clone() },
            Expr::Binary { op: BinaryOp::Lt, lhs, rhs } => Expr::Binary { op: BinaryOp::Ge, lhs: lhs.clone(), rhs: rhs.clone() },
            Expr::Binary { op: BinaryOp::Ge, lhs, rhs } => Expr::Binary { op: BinaryOp::Lt, lhs: lhs.clone(), rhs: rhs.clone() },
            Expr::Binary { op: BinaryOp::Gt, lhs, rhs } => Expr::Binary { op: BinaryOp::Le, lhs: lhs.clone(), rhs: rhs.clone() },
            Expr::Binary { op: BinaryOp::Le, lhs, rhs } => Expr::Binary { op: BinaryOp::Gt, lhs: lhs.clone(), rhs: rhs.clone() },
            Expr::Binary { op: BinaryOp::And, lhs, rhs } => Expr::Binary { op: BinaryOp::Or, lhs: Box::new(lhs.neg()), rhs: Box::new(rhs.neg()) },
            Expr::Binary { op: BinaryOp::Or, lhs, rhs } => Expr::Binary { op: BinaryOp::And, lhs: Box::new(lhs.neg()), rhs: Box::new(rhs.neg()) },

            _ => Expr::Unary { op: UnaryOp::Not, expr: Box::new(self.clone()) }
        }
    }

    pub fn count_reads(&self, name: &str) -> usize {
        match self {
            Expr::Name(nm) => (nm == name) as usize,
            Expr::Binary { lhs, rhs, .. } => lhs.count_reads(name) + rhs.count_reads(name),
            Expr::Unary { expr, .. } => expr.count_reads(name),
            Expr::Bool(_) => 0,
            Expr::Num(_) => 0,
            Expr::Deref(expr) => expr.count_reads(name),
            Expr::Call { func, args } => args.iter().fold(func.count_reads(name), |prev, x| prev + x.count_reads(name))
        }
    }

    pub fn read_names_rhs(&self) -> Vec<String> {
        let mut vec = Vec::new();
        self.append_read_names_rhs(&mut vec);
        vec
    }

    pub fn append_read_names_rhs(&self, names: &mut Vec<String>) {
        match self {
            Expr::Name(name) => names.push(name.clone()),
            Expr::Deref(expr) => expr.append_read_names_rhs(names),
            Expr::Unary { expr, .. } => expr.append_read_names_rhs(names),
            Expr::Binary { lhs, rhs, .. } => {
                lhs.append_read_names_rhs(names);
                rhs.append_read_names_rhs(names)
            }
            Expr::Call { func, args } => {
                func.append_read_names_rhs(names);
                for arg in args {
                    arg.append_read_names_rhs(names);
                }
            }
            Expr::Num(_) | Expr::Bool(_) => {}
        }
    }

    pub fn append_read_names_lhs(&self, names: &mut Vec<String>) {
        if let Expr::Name(_) = self {
            return;
        }

        self.append_read_names_rhs(names)
    }

    pub fn replace_name(&mut self, name: &str, expr: &Expr) {
        match self {
            Expr::Name(name_) if name_ == name => *self = expr.clone(),
            Expr::Bool(_) | Expr::Num(_) | Expr::Name(_) => (),
            Expr::Binary { lhs, rhs, .. } => {
                lhs.replace_name(name, expr);
                rhs.replace_name(name, expr);
            }
            Expr::Unary { expr: uexpr, .. } => uexpr.replace_name(name, expr),
            Expr::Deref(dexpr) => dexpr.replace_name(name, expr),
            Expr::Call { func, args } => {
                func.replace_name(name, expr);
                for arg in args {
                    arg.replace_name(name, expr);
                }
            }
        }
    }

    pub fn collapse_cmp(&mut self) {
        match self {
            Expr::Name(_) => {},
            Expr::Num(_) | Expr::Bool(_) => {},
            Expr::Unary { expr, op } if op.is_cmp() => {
                if let Expr::Binary { op: BinaryOp::Cmp, lhs, rhs } = expr.as_mut() {
                    lhs.collapse_cmp();
                    rhs.collapse_cmp();
                    *self = Expr::Binary { op: op.cmp_op_to_binaryop(), lhs: lhs.clone(), rhs: rhs.clone() };
                } else {
                    expr.collapse_cmp();
                }
            },
            Expr::Unary { expr, .. } => expr.collapse_cmp(),
            Expr::Binary { lhs, rhs, .. } => {
                lhs.collapse_cmp();
                rhs.collapse_cmp();
            }
            Expr::Deref(x) => x.collapse_cmp(),
            Expr::Call { func, args } => {
                func.collapse_cmp();
                for arg in args {
                    arg.collapse_cmp();
                }
            }
        }
    }
}

pub struct BlindMultiBlockIter<const S: usize> {
    offset: usize,
}

impl<const S: usize> BlindMultiBlockIter<S> {
    pub fn new() -> BlindMultiBlockIter<S> {
        BlindMultiBlockIter {
            offset: 0,
        }
    }

    pub fn step(&mut self, block: &Block) -> Vec<usize> {
        let mut indices = Vec::new();
        let mut offset = 0;

        while let Some(stmt) = block.0.get(self.offset + offset) {
            offset += 1;
            if stmt.invisible() {
                continue;
            }
            
            indices.push(self.offset + offset - 1);

            if indices.len() >= S {
                break
            }
        }

        if let Some(first) = indices.first() {
            self.offset = first + 1;
        }

        indices
    }
}

pub struct BlindBlockIter {
    offset: usize
}

impl BlindBlockIter {
    pub fn new() -> BlindBlockIter {
        BlindBlockIter {
            offset: 0
        }
    }

    pub fn offset(&self) -> usize {
        self.offset - 1
    }

    pub fn next<'a>(&mut self, block: &'a Block) -> Option<&'a Stmt> {
        loop {
            let stmt = block.0.get(self.offset)?;
            self.offset += 1;

            if !stmt.invisible() {
                return Some(stmt);
            }
        }
    }

    pub fn seek(&mut self, idx: usize) {
        self.offset = idx;
    }
}

pub struct ScopeLabels {
    start: Vec<Weak<Label>>,
    end: Vec<Weak<Label>>,
}

impl ScopeLabels {
    pub fn empty() -> ScopeLabels {
        ScopeLabels {
            start: Vec::new(),
            end: Vec::new()
        }
    }

    pub fn new(start: Vec<Weak<Label>>, end: Vec<Weak<Label>>) -> ScopeLabels {
        ScopeLabels {
            start,
            end
        }
    }

    pub fn append_start(&mut self, iter: impl Iterator<Item = Weak<Label>>) {
        self.start.extend(iter);
    }

    pub fn append_end(&mut self, iter: impl Iterator<Item = Weak<Label>>) {
        self.end.extend(iter);
    }

    pub fn start(&self) -> impl Iterator<Item = Rc<Label>> + '_ {
        self.start.iter().filter_map(|x| x.upgrade())
    }

    pub fn start_weak(&self) -> &[Weak<Label>] {
        &self.start
    }

    pub fn end(&self) -> impl Iterator<Item = Rc<Label>> + '_ {
        self.end.iter().filter_map(|x| x.upgrade())
    }

    pub fn end_weak(&self) -> &[Weak<Label>] {
        &self.end
    }
}

#[derive(Clone, Debug)]
pub struct Block(Vec<Stmt>);

impl Block {
    pub fn empty() -> Block {
        Block(Vec::new())
    }

    pub fn new_from(stmts: Vec<Stmt>) -> Block {
        Block(stmts)
    }

    pub fn get(&self) -> &[Stmt] {
        &self.0
    }

    pub fn get_mut(&mut self) -> &mut Vec<Stmt> {
        &mut self.0
    }

    pub fn add(&mut self, stmt: Stmt) {
        self.0.push(stmt);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn at(&self, idx: usize) -> &Stmt {
        self.0.get(idx).unwrap()
    }

    pub fn at_mut(&mut self, idx: usize) -> &mut Stmt {
        self.0.get_mut(idx).unwrap()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn non_nop_len(&self) -> usize {
        let mut len = 0;
        for stmt in &self.0 {
            match stmt {
                Stmt::Label(label) => len += label.upgrade().is_some() as usize,
                Stmt::Nop => {},
                _ => len += 1
            }
        }
        len
    }

    pub fn pop_non_nop_last(&mut self) -> Option<Stmt> {
        if self.len() == 0 {
            return None;
        }

        let mut i = self.len();
        while i > 0 {
            match &self.0[i - 1] {
                Stmt::Label(_) | Stmt::Nop => i -= 1,
                _ => {
                    return Some(self.0.remove(i - 1));
                }
            }
        }

        None
    }

    pub fn non_nop_last(&self) -> Option<&Stmt> {
        if self.len() == 0 {
            return None
        }

        let mut i = self.len();
        while i > 0 {
            match &self.0[i - 1] {
                Stmt::Label(_) | Stmt::Nop => i -= 1,
                x => return Some(x)
            }
        }

        None
    }
    
    pub fn non_nop_first(&self) -> Option<&Stmt> {
        if self.len() == 0 {
            return None
        }

        let mut i = 0;
        while i < self.len() {
            match &self.0[i] {
                Stmt::Label(_) | Stmt::Nop => i += 1,
                x => return Some(x)
            }
        }

        None
    }

    pub fn pop_non_nop_first(&mut self) -> Option<Stmt> {
        if self.len() == 0 {
            return None
        }

        let mut i = 0;
        while i < self.len() {
            match &self.0[i] {
                Stmt::Label(_) | Stmt::Nop => i += 1,
                _ => {
                    return Some(self.0.remove(i))
                }
            }
        }

        None
    }

    pub fn iter_all(&self) -> impl Iterator<Item=(usize, &Stmt)> {
        self.0.iter().enumerate()
    }

    pub fn iter(&self) -> impl Iterator<Item=(usize, &Stmt)> {
        self.0.iter().enumerate().filter(|(_, x)| !x.invisible())
    }

    pub fn labels_at(&self, idx: usize, scope: &ScopeLabels) -> Vec<Rc<Label>> {
        let mut labels = Vec::new();

        let mut backward_idx = idx;
        loop {
            match self.at(backward_idx) {
                Stmt::Label(label) => {
                    match label.upgrade() {
                        Some(label) => labels.push(label),
                        None => {}
                    }
                }
                Stmt::Nop => {}
                _ => break
            }

            if backward_idx == 0 {
                labels.extend(scope.start());
                break;
            }

            backward_idx -= 1;
        }

        let mut forward_idx = idx;
        loop {
            forward_idx += 1;

            if forward_idx == self.0.len() {
                labels.extend(scope.end());
                break;
            }
            
            match self.at(forward_idx) {
                Stmt::Label(label) => {
                    match label.upgrade() {
                        Some(label) => labels.push(label),
                        None => {}
                    }
                }
                Stmt::Nop => {}
                _ => break
            }
        }

        labels
    }

    // Does not include self.0[idx]
    pub fn labels_back_from(&self, idx: usize, scope: &ScopeLabels) -> Vec<Weak<Label>> {
        let mut labels = Vec::new();

        let mut backward_idx = idx;
        loop {
            if backward_idx == 0 {
                labels.extend(scope.start_weak().iter().cloned());
                break;
            }

            backward_idx -= 1;

            match self.at(backward_idx) {
                Stmt::Label(label) => {
                    labels.push(label.clone());
                }
                Stmt::Nop => {}
                _ => break
            }
        }

        labels
    }

    // Does not include self.0[idx]
    pub fn labels_forward_from(&self, idx: usize, scope: &ScopeLabels) -> Vec<Weak<Label>> {
        let mut labels = Vec::new();

        let mut forward_idx = idx;
        loop {
            if forward_idx + 1 == self.0.len() {
                labels.extend(scope.end_weak().iter().cloned());
                break;
            }

            forward_idx += 1;
            
            match self.at(forward_idx) {
                Stmt::Label(label) => {
                    labels.push(label.clone());
                }
                Stmt::Nop => {}
                _ => break
            }
        }

        labels
    }

    pub fn find_label_flat(&self, label: &Rc<Label>, parent: Option<&ScopeLabels>) -> Option<usize> {
        if let Some(parent) = parent {
            if parent.start().find(|x| Rc::ptr_eq(x, label)).is_some() {
                return Some(0);
            }
    
            if parent.end().find(|x| Rc::ptr_eq(x, label)).is_some() {
                return Some(self.0.len());
            }
        }

        for (s, stmt) in self.0.iter().enumerate() {
            if let Stmt::Label(other) = stmt {
                if other.as_ptr() == Rc::as_ptr(label) {
                    return Some(s);
                }
            }
        }

        None
    }

    pub fn label_block_start(&self, mut idx: usize) -> usize {
        loop {
            if idx == 0 {
                return 0;
            }

            idx -= 1;
            
            match self.at(idx) {
                Stmt::Label(_) | Stmt::Nop => {}
                _ => return idx + 1
            }
        }
    }

    pub fn can_fallthrough_to(&self, mut idx: usize) -> bool {
        while idx > 0 {
            idx -= 1;

            match self.at(idx) {
               Stmt::Branch { cond: None, .. }  => return false,
               Stmt::Label(label) if label.strong_count() > 0  => return true,
               _ => {}
            }
        }

        true
    }

    pub fn take(self) -> Vec<Stmt> {
        self.0
    }
}

#[derive(Debug)]
pub struct Label(pub String);

#[derive(Clone, Debug)]
pub enum Stmt {
    Nop,
    Assign {
        lhs: Expr,
        rhs: Expr
    },
    Label(Weak<Label>),
    Branch {
        cond: Option<Expr>,
        target: Rc<Label>,
    },
    If {
        cond: Expr,
        true_then: Block,
        false_then: Block,
    },
    Loop {
        code: Block
    },
    For {
        inc: Box<Stmt>,
        guard: Expr,
        code: Block
    },
    Break,
    Continue,
    Return(Expr)
}

impl Stmt {
    pub fn invisible(&self) -> bool {
        match self {
            Stmt::Label(label) => label.upgrade().is_none(),
            Stmt::Nop => true,
            _ => false
        }
    }

    pub fn count_reads(&self, name: &str) -> usize {
        match self {
            Stmt::Nop | Stmt::Break | Stmt::Continue | Stmt::Label(_) => 0,
            Stmt::Return(expr) => expr.count_reads(name),
            Stmt::Assign { lhs: Expr::Name(_), rhs } => rhs.count_reads(name),
            Stmt::Assign { lhs, rhs } => lhs.count_reads(name) + rhs.count_reads(name),
            Stmt::Branch { cond: Some(cond), .. } => cond.count_reads(name),
            Stmt::Branch { .. } => 0,
            Stmt::If { .. } | Stmt::Loop { .. } | Stmt::For { .. } => panic!("Complex statements"),
        }
    }

    pub fn replace_name(&mut self, name: &str, expr: &Expr) {
        match self {
            Stmt::Nop | Stmt::Break | Stmt::Continue | Stmt::Label(_) => (),
            Stmt::Return(ret) => ret.replace_name(name, expr),
            Stmt::Assign { lhs: Expr::Name(_), rhs } => rhs.replace_name(name, expr),
            Stmt::Assign { lhs, rhs } => {
                lhs.replace_name(name, expr);
                rhs.replace_name(name, expr);
            },
            Stmt::Branch { cond: Some(cond), .. } => cond.replace_name(name, expr),
            Stmt::Branch { .. } => (),
            Stmt::If { .. } | Stmt::Loop { .. } | Stmt::For { .. } => panic!("Complex statements")
        }
    }
}

impl Display for Stmt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Stmt::Assign { lhs, rhs } => write!(f, "{} = {}", lhs, rhs),
            Stmt::Nop => Ok(()),
            Stmt::Label(label) =>
                match label.upgrade() {
                    Some(label) => write!(f, "{}:", label.0),
                    None => Ok(()),
                },
            Stmt::Branch { cond: Some(cond), target } => write!(f, "if {}, goto {}", cond, target.0),
            Stmt::Branch { cond: None, target } => write!(f, "goto {}", target.0),
            Stmt::If { cond, true_then, false_then } => {
                write!(f, "if {} {{", cond)?;
                for stmt in true_then.get() {
                    if !stmt.invisible() {
                        f.write_str(&format!("\n{}", stmt).replace('\n', NEWLINE_INDENT))?;
                    }
                }
                write!(f, "\n}}")?;

                if !false_then.is_empty() {
                    write!(f, " else {{")?;
                    for stmt in false_then.get() {
                        if !stmt.invisible() {
                            f.write_str(&format!("\n{}", stmt).replace('\n', NEWLINE_INDENT))?;
                        }
                    }
                    write!(f, "\n}}")?;
                }

                Ok(())
            }
            Stmt::For { guard, inc, code } => {
                write!(f, "for {}; {} {{", guard, inc)?;
                for stmt in code.get() {
                    if !stmt.invisible() {
                        f.write_str(&format!("\n{}", stmt).replace('\n', NEWLINE_INDENT))?;
                    }
                }
                write!(f, "\n}}")?;

                Ok(())
            }
            Stmt::Loop { code } => {
                write!(f, "loop {{")?;
                for stmt in code.get() {
                    if !stmt.invisible() {
                        f.write_str(&format!("\n{}", stmt).replace('\n', NEWLINE_INDENT))?;
                    }
                }
                write!(f, "\n}}")
            }
            Stmt::Break => write!(f, "break"),
            Stmt::Continue => write!(f, "continue"),
            Stmt::Return(expr) => write!(f, "return {expr}"),
        }
    }
}
