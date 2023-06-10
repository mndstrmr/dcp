use crate::ty;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    CmpEq, CmpNe, CmpLt, CmpLe, CmpGt, CmpGe,
}

impl std::fmt::Display for UnaryOp {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Eq, Ne, Lt, Le, Gt, Ge,
    Add, Sub, Mul, Div,
    And, Or, Shl, Shr,
    Cmp
}

impl BinaryOp {
    pub fn is_logical(&self) -> bool {
        match self {
            BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge |
            BinaryOp::And | BinaryOp::Or => true,
            _ => false
        }
    }
}

impl std::fmt::Display for BinaryOp {
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
            BinaryOp::Shl => "<<",
            BinaryOp::Shr => ">>",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
            BinaryOp::Cmp => "cmp",
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FuncId(pub usize);

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Name(String), // FIXME: Intern this or something some day
    Num(i64),
    Func(FuncId),
    Bool(bool),
    Deref {
        ptr: Box<Expr>,
        size: ty::Size
    },
    Ref(Box<Expr>),
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

impl Expr {
    pub fn fmt_with_precedence(&self, f: &mut std::fmt::Formatter<'_>, prec: usize) -> std::fmt::Result {
        const REF: usize = 5;
        const FUNC: usize = 15;
        const UNARY: usize = 10;
        const BINARY: usize = 4;

        match self {
            Expr::Name(name) => write!(f, "{}", name),
            Expr::Num(x) => {
                // Power of two, or one less than power of 2
                if *x >= 4096 || (*x > 32 && ((x & (x - 1)) == 0 || ((x + 1) & x) == 0)) {
                    write!(f, "0x{:x}", x)
                } else {
                    write!(f, "{}", x)
                }
            },
            Expr::Func(idx) => write!(f, "fn{}", idx.0),
            Expr::Bool(b) => write!(f, "{}", b),

            Expr::Deref { ptr, size } => {
                if prec >= REF {
                    write!(f, "(*{size} ")?;
                    ptr.fmt_with_precedence(f, REF)?;
                    write!(f, ")")
                } else {
                    write!(f, "*{size} ")?;
                    ptr.fmt_with_precedence(f, REF)
                }
            },
            Expr::Ref(value) => {
                if prec >= REF {
                    write!(f, "(&")?;
                    value.fmt_with_precedence(f, REF)?;
                    write!(f, ")")
                } else {
                    write!(f, "&")?;
                    value.fmt_with_precedence(f, REF)
                }
            }
            Expr::Unary { op, expr } if op.is_cmp() => {
                if prec >= UNARY {
                    write!(f, "(")?;
                    expr.fmt_with_precedence(f, UNARY)?;
                    write!(f, ".{})", op)
                } else {
                    expr.fmt_with_precedence(f, UNARY)?;
                    write!(f, ".{}", op)
                }
            }
            Expr::Unary { op, expr } => {
                if prec >= UNARY {
                    write!(f, "({}", op)?;
                    expr.fmt_with_precedence(f, UNARY)?;
                    write!(f, ")")
                } else {
                    write!(f, "{}", op)?;
                    expr.fmt_with_precedence(f, UNARY)
                }
            }
            Expr::Binary { op, lhs, rhs } => {
                if prec >= BINARY {
                    write!(f, "(")?;
                    lhs.fmt_with_precedence(f, BINARY)?;
                    write!(f, " {} ", op)?;
                    rhs.fmt_with_precedence(f, BINARY)?;
                    write!(f, ")")
                } else {
                    lhs.fmt_with_precedence(f, BINARY)?;
                    write!(f, " {} ", op)?;
                    rhs.fmt_with_precedence(f, BINARY)
                }
            },
            Expr::Call { func, args } => {
                if prec >= FUNC {
                    write!(f, "(")?;
                    func.fmt_with_precedence(f, FUNC)?;
                    write!(f, "({}))", args.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(", "))
                } else {
                    func.fmt_with_precedence(f, FUNC)?;
                    write!(f, "({})", args.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(", "))
                }
            },
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_with_precedence(f, 0)
    }
}

impl Expr {
    pub fn take(&mut self) -> Expr {
        std::mem::replace(self, Expr::Bool(false))
    }

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

    pub fn has_side_effects(&self) -> bool {
        match self {
            Expr::Name(_) => false,
            Expr::Func(_) => false,
            Expr::Binary { lhs, rhs, .. } => lhs.has_side_effects() || rhs.has_side_effects(),
            Expr::Unary { expr, .. } => expr.has_side_effects(),
            Expr::Bool(_) => false,
            Expr::Num(_) => false,
            Expr::Deref { ptr, .. } => ptr.has_side_effects(),
            Expr::Ref(value) => value.has_side_effects(),
            Expr::Call { .. } => true
        }
    }

    pub fn count_reads(&self, name: &str) -> usize {
        match self {
            Expr::Name(nm) => (*nm == name) as usize,
            Expr::Func(_) => 0,
            Expr::Binary { lhs, rhs, .. } => lhs.count_reads(name) + rhs.count_reads(name),
            Expr::Unary { expr, .. } => expr.count_reads(name),
            Expr::Bool(_) => 0,
            Expr::Num(_) => 0,
            Expr::Deref { ptr, .. } => ptr.count_reads(name),
            Expr::Ref(value) => value.count_reads(name),
            Expr::Call { func, args } => args.iter().fold(func.count_reads(name), |prev, x| prev + x.count_reads(name))
        }
    }

    pub fn read_names_rhs(&self) -> Vec<&str> {
        let mut vec = Vec::new();
        self.append_read_names_rhs(&mut vec);
        vec
    }

    pub fn read_names_lhs(&self) -> Vec<&str> {
        let mut vec = Vec::new();
        self.append_read_names_lhs(&mut vec);
        vec
    }

    fn append_read_names_rhs<'a>(&'a self, names: &mut Vec<&'a str>) {
        match self {
            Expr::Name(name) => names.push(name.as_str()),
            Expr::Deref { ptr, .. } => ptr.append_read_names_rhs(names),
            Expr::Ref(value) => value.append_read_names_rhs(names),
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
            Expr::Num(_) | Expr::Bool(_) | Expr::Func(_) => {}
        }
    }

    fn append_read_names_lhs<'a>(&'a self, names: &mut Vec<&'a str>) {
        if let Expr::Name(_) = self {
            return;
        }

        self.append_read_names_rhs(names)
    }

    pub fn replace_name(&mut self, name: &str, expr: &Expr) {
        match self {
            Expr::Name(name_) if *name_ == name => *self = expr.clone(),
            Expr::Bool(_) | Expr::Num(_) | Expr::Name(_) | Expr::Func(_) => (),
            Expr::Binary { lhs, rhs, .. } => {
                lhs.replace_name(name, expr);
                rhs.replace_name(name, expr);
            }
            Expr::Unary { expr: uexpr, .. } => uexpr.replace_name(name, expr),
            Expr::Deref { ptr, .. } => ptr.replace_name(name, expr),
            Expr::Ref(value) => value.replace_name(name, expr),
            Expr::Call { func, args } => {
                func.replace_name(name, expr);
                for arg in args {
                    arg.replace_name(name, expr);
                }
            }
        }
    }
}
