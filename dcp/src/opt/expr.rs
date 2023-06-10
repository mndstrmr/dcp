use crate::{expr, mir::{MirVisitorMut, self}, lir};

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
    match sexpr {
        expr::Expr::Name(_) | expr::Expr::Num(_) | expr::Expr::Bool(_) | expr::Expr::Func(_) => {},
        expr::Expr::Unary { expr, .. } => reduce_binops_in(expr.as_mut()),
        expr::Expr::Binary { lhs, rhs, op } => {
            reduce_binops_in(lhs.as_mut());
            reduce_binops_in(rhs.as_mut());

            if let expr::Expr::Num(n) = rhs.as_ref() &&
                let expr::Expr::Binary { op: op2, lhs: lhs2, rhs: rhs2 } = lhs.as_mut() &&
                let expr::Expr::Num(n2) = rhs2.as_ref() {
                match (op, op2) {
                    (expr::BinaryOp::Add, expr::BinaryOp::Add) =>
                        *sexpr = expr::Expr::Binary {
                            op: expr::BinaryOp::Add,
                            lhs: Box::new(lhs2.take()),
                            rhs: Box::new(expr::Expr::Num(n + n2))
                        },
                    (expr::BinaryOp::Add, expr::BinaryOp::Sub) =>
                        *sexpr = expr::Expr::Binary {
                            op: expr::BinaryOp::Sub,
                            lhs: Box::new(lhs2.take()),
                            rhs: Box::new(expr::Expr::Num(n2 - n))
                        },
                    (expr::BinaryOp::And, op2) if op2.is_logical() && *n != 0 =>
                        *sexpr = lhs.take(),
                    _ => {}
                }
        } else if
                let expr::Expr::Num(n1) = rhs.as_ref() &&
                let expr::Expr::Num(n2) = lhs.as_ref() {
            match op {
                expr::BinaryOp::Add => *sexpr = expr::Expr::Num(n1 + n2),
                _ => {}
            }
        } else if let expr::Expr::Num(n) = rhs.as_ref() &&
                *n < 0 && *op == expr::BinaryOp::Sub {
                *rhs = Box::new(expr::Expr::Num(-n));
                *op = expr::BinaryOp::Add;
            } else if let expr::Expr::Num(n) = rhs.as_ref() && *n == 0 {
                match op {
                    expr::BinaryOp::Add => *sexpr = lhs.take(),
                    expr::BinaryOp::Sub => *sexpr = lhs.take(),
                    expr::BinaryOp::Mul => *sexpr = expr::Expr::Num(0),
                    _ => {}
                };
            } else if let expr::Expr::Num(1) = rhs.as_ref() &&
                    let expr::BinaryOp::And = op &&
                    let expr::Expr::Binary { op: op2, .. } = lhs.as_ref() &&
                    op2.is_logical() {
                *sexpr = lhs.take();
            }
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
