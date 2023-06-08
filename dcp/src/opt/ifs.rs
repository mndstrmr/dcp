use crate::{mir::{Mir, MirVisitorMut, MVMAction, MirFunc}, expr};

struct TerminatingIfVisitor;

impl MirVisitorMut for TerminatingIfVisitor {
    fn visit_if(&mut self, cond: &mut expr::Expr, true_then: &mut Vec<Mir>, false_then: &mut Vec<Mir>) -> MVMAction {
        if true_then.last().map_or(false, Mir::terminating) && !false_then.is_empty() {
            let mut new_code = vec![
                Mir::If {
                    cond: cond.take(),
                    true_then: true_then.drain(..).collect(),
                    false_then: vec![]
                }
            ];
            new_code.extend(false_then.drain(..));
            MVMAction::ReplaceMany(new_code)
        } else if false_then.last().map_or(false, Mir::terminating) {
            let mut new_code = vec![
                Mir::If {
                    cond: cond.neg(),
                    true_then: false_then.drain(..).collect(),
                    false_then: vec![]
                }
            ];
            new_code.extend(true_then.drain(..));
            MVMAction::ReplaceMany(new_code)
        } else {
            self.visit_block(true_then);
            self.visit_block(false_then);
            MVMAction::Keep
        }
    }
}

pub fn inline_terminating_if(code: &mut MirFunc) {
    TerminatingIfVisitor.visit_block(&mut code.code)
}

struct FlipIfVisitor;

impl MirVisitorMut for FlipIfVisitor {
    fn visit_if(&mut self, cond: &mut expr::Expr, true_then: &mut Vec<Mir>, false_then: &mut Vec<Mir>) -> MVMAction {
        let mut is_inverse = false;
        while let expr::Expr::Unary { op: expr::UnaryOp::Not, expr } = cond {
            *cond = expr.take();
            is_inverse = !is_inverse;
        }

        if is_inverse {
            std::mem::swap(true_then, false_then);
        }

        MVMAction::Keep
    }
}

pub fn flip_negated_ifs(code: &mut MirFunc) {
    FlipIfVisitor.visit_block(&mut code.code)
}
