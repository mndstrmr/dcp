use crate::{expr::{self, UnaryOp}, mir::{MirVisitorMut, self}, lir};

fn collapse_cmp_in(sexpr: &mut expr::Expr) {
    match sexpr {
        expr::Expr::Name(_) | expr::Expr::Num(_) | expr::Expr::Bool(_) | expr::Expr::Func(_) => {},
        expr::Expr::Unary { expr, op } if op.is_cmp() => {
            if let expr::Expr::Binary { op: expr::BinaryOp::Cmp, lhs, rhs } = expr.as_mut() {
                collapse_cmp_in(lhs);
                collapse_cmp_in(rhs);
                *sexpr = expr::Expr::Binary { op: op.cmp_op_to_binaryop(), lhs: lhs.clone(), rhs: rhs.clone() };
            } else {
                collapse_cmp_in(expr);
            }
        },
        expr::Expr::Unary { expr, .. } => collapse_cmp_in(expr),
        expr::Expr::Binary { lhs, rhs, .. } => {
            collapse_cmp_in(lhs);
            collapse_cmp_in(rhs);
        }
        expr::Expr::Deref { ptr, .. } => collapse_cmp_in(ptr),
        expr::Expr::Ref(value) => collapse_cmp_in(value),
        expr::Expr::Call { func, args } => {
            collapse_cmp_in(func);
            for arg in args {
                collapse_cmp_in(arg);
            }
        }
    }
}

struct CollapseCmpVisitor;

impl MirVisitorMut for CollapseCmpVisitor {
    fn visit_expr(&mut self, expr: &mut expr::Expr) {
        collapse_cmp_in(expr);
    }
}

pub fn collapse_cmp(code: &mut mir::MirFunc) {
    CollapseCmpVisitor.visit_block(&mut code.code)    
}

fn reduce_binops_in(sexpr: &mut expr::Expr) {
    use expr::{Expr::*, BinaryOp::*};
    match sexpr {
        expr::Expr::Name(_) | expr::Expr::Num(_) | expr::Expr::Bool(_) | expr::Expr::Func(_) => {},
        expr::Expr::Unary { expr, op: UnaryOp::Not } => {
            reduce_binops_in(expr.as_mut());
            *sexpr = expr.neg();
        }
        expr::Expr::Unary { expr, .. } => {
            reduce_binops_in(expr.as_mut());
        }
        expr::Expr::Binary { lhs, rhs, op } => {
            reduce_binops_in(lhs.as_mut());
            reduce_binops_in(rhs.as_mut());

            macro_rules! x {
                ($lhs:pat, $op:pat, $rhs:pat $(if $g:expr)* => $x:expr) => {
                    #[allow(irrefutable_let_patterns)]
                    if
                        let $lhs = lhs.as_mut() &&
                        let $op = op &&
                        let $rhs = rhs.as_mut()
                        $(
                            && $g
                        )*
                    { $x; return; }
                };

                (!($lhs2:pat, $op2:pat, $rhs2:pat), $op:pat, $rhs:pat $(if $g:expr)* => $x:expr) => {
                    if
                        let Binary { op: op2, lhs: lhs2, rhs: rhs2 } = lhs.as_mut() &&
                        let $lhs2 = lhs2.as_mut() &&
                        let $op2 = op2 &&
                        let $rhs2 = rhs2.as_mut() &&
                        let $op = op &&
                        let $rhs = rhs.as_mut()
                        $(
                            && $g
                        )*
                    { $x; return; }
                };

                ($lhs:pat, $op:pat, ! ($lhs2:pat, $op2:pat, $rhs2:pat) $(if $g:expr)* => $x:expr) => {
                    #[allow(irrefutable_let_patterns)]
                    if
                        let $lhs = lhs.as_mut() &&
                        let $op = op &&
                        let Binary { op: op2, lhs: lhs2, rhs: rhs2 } = rhs.as_mut() &&
                        let $lhs2 = lhs2.as_mut() &&
                        let $op2 = op2 &&
                        let $rhs2 = rhs2.as_mut()
                        $(
                            && $g
                        )*
                    { $x; return; }
                };
            }

            x!(Num(n1), Add, Num(n2) => *sexpr = Num(*n1 + *n2));
            x!(lhs, Add, Num(0) => *sexpr = lhs.take());
            x!(lhs, Sub, Num(0) => *sexpr = lhs.take());
            x!(lhs, Mul, Num(1) => *sexpr = lhs.take());
            x!(_, Mul, Num(0) => *sexpr = Num(0));
            x!(!(lhs2, Add, Num(n)), Add, Num(n2) => *sexpr = expr::Expr::Binary {
                op: expr::BinaryOp::Add,
                lhs: Box::new(lhs2.take()),
                rhs: Box::new(expr::Expr::Num(*n + *n2))
            });
            x!(!(lhs2, Add, Num(n)), Sub, Num(n2) => *sexpr = expr::Expr::Binary {
                op: expr::BinaryOp::Add,
                lhs: Box::new(lhs2.take()),
                rhs: Box::new(expr::Expr::Num(*n - *n2))
            });
            x!(_, Sub, Num(n2) if *n2 < 0 => {
                *rhs = Box::new(Num(-*n2));
                *op = Add;
            });
            x!(Num(1), And, ! (lhs, op, rhs) if op.is_logical() => {
                *sexpr = expr::Expr::Binary {
                    op: *op,
                    lhs: Box::new(lhs.take()),
                    rhs: Box::new(rhs.take())
                }
            });
            x!(! (lhs, op, rhs), And, Num(1) if op.is_logical() => {
                *sexpr = expr::Expr::Binary {
                    op: *op,
                    lhs: Box::new(lhs.take()),
                    rhs: Box::new(rhs.take())
                }
            });
            x!(Unary { expr: lhs2, op: UnaryOp::Not }, Or, Unary { expr: rhs2, op: UnaryOp::Not } => {
                *sexpr = expr::Expr::Binary {
                    op: And,
                    lhs: Box::new(lhs2.take()),
                    rhs: Box::new(rhs2.take())
                }
            });
        }
        expr::Expr::Deref { ptr, .. } => reduce_binops_in(ptr.as_mut()),
        expr::Expr::Ref(value) => reduce_binops_in(value.as_mut()),
        expr::Expr::Call { func, args } => {
            reduce_binops_in(func.as_mut());
            for arg in args {
                reduce_binops_in(arg);
            }
        }
    }
}

struct ReduceBinOpVisitor;

impl MirVisitorMut for ReduceBinOpVisitor {
    fn visit_expr(&mut self, expr: &mut expr::Expr) {
        reduce_binops_in(expr);
    }
}

pub fn reduce_binops(code: &mut mir::MirFunc) {
    ReduceBinOpVisitor.visit_block(&mut code.code)
}

pub fn reduce_binops_lir(blocks: &mut [lir::LirNode]) {
    for block in blocks {
        for stmt in &mut block.code {
            match stmt {
                lir::Lir::Assign { src, dst } => {
                    reduce_binops_in(src);
                    reduce_binops_in(dst);
                }
                lir::Lir::Return(expr) => reduce_binops_in(expr),
                lir::Lir::Do(expr) => reduce_binops_in(expr),
                lir::Lir::Branch { cond: Some(cond), .. } => reduce_binops_in(cond),
                lir::Lir::Branch { .. } | lir::Lir::Label(_) => {}
            }
        }
    }
}
