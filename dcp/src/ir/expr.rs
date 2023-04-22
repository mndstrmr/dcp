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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BinaryOp {
    Eq, Ne, Lt, Le, Gt, Ge,
    Add, Sub, Mul, Div,
    And, Or,
    Cmp
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


impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Expr::Name(name) => write!(f, "{}", name),
            Expr::Num(num) => write!(f, "{}", num),
            Expr::Func(idx) => write!(f, "fn{}", idx.0),
            Expr::Bool(b) => write!(f, "{}", b),
            Expr::Deref { ptr, size } => write!(f, "*{size} {}", ptr),
            Expr::Ref(value) => write!(f, "&{value}"),
            Expr::Unary { op, expr } if op.is_cmp() => write!(f, "{}.{}", expr, op),
            Expr::Unary { op, expr } => write!(f, "{}{}", op, expr),
            Expr::Binary { op, lhs, rhs } => write!(f, "({} {} {})", lhs, op, rhs),
            Expr::Call { func, args } => write!(f, "{}({})", func, args.iter().map(|x| x.to_string()).collect::<Vec<String>>().join(", ")),
        }
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

    pub fn append_read_names_rhs<'a>(&'a self, names: &mut Vec<&'a str>) {
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

    pub fn append_read_names_lhs<'a>(&'a self, names: &mut Vec<&'a str>) {
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

    pub fn collapse_cmp(&mut self) {
        match self {
            Expr::Name(_) | Expr::Num(_) | Expr::Bool(_) | Expr::Func(_) => {},
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
            Expr::Deref { ptr, .. } => ptr.collapse_cmp(),
            Expr::Ref(value) => value.collapse_cmp(),
            Expr::Call { func, args } => {
                func.collapse_cmp();
                for arg in args {
                    arg.collapse_cmp();
                }
            }
        }
    }

    pub fn reduce_binops(&mut self) {
        match self {
            Expr::Name(_) | Expr::Num(_) | Expr::Bool(_) | Expr::Func(_) => {},
            Expr::Unary { expr, .. } => expr.reduce_binops(),
            Expr::Binary { lhs, rhs, op } => {
                lhs.reduce_binops();
                rhs.reduce_binops();

                if let Expr::Num(n) = rhs.as_ref() &&
                    let Expr::Binary { op: op2, lhs: lhs2, rhs: rhs2 } = lhs.as_mut() &&
                    let Expr::Num(n2) = rhs2.as_ref() {
                    match (op, op2) {
                        (BinaryOp::Add, BinaryOp::Add) =>
                            *self = Expr::Binary {
                                op: BinaryOp::Add,
                                lhs: Box::new(lhs2.take()),
                                rhs: Box::new(Expr::Num(n + n2))
                            },
                        _ => {}
                    }
                }
            }
            Expr::Deref { ptr, .. } => ptr.reduce_binops(),
            Expr::Ref(value) => value.reduce_binops(),
            Expr::Call { func, args } => {
                func.reduce_binops();
                for arg in args {
                    arg.reduce_binops();
                }
            }
        }
    }
}
