use crate::{mir::{self, MirVisitorMut}, expr};

struct WhileToForVisitor;

impl MirVisitorMut for WhileToForVisitor {
    fn visit_while(&mut self, guard: &mut expr::Expr, code: &mut Vec<mir::Mir>) -> mir::MVMAction {
        if let Some(mir::Mir::Assign { .. }) = code.last() && !mir::contains_continue(code) {
            let inc = code.pop().unwrap();
            let new_code = code.drain(..).collect();
            mir::MVMAction::Replace(mir::Mir::For {
                guard: guard.take(),
                inc: vec![inc],
                code: new_code
            })
        } else {
            self.visit_block(code);
            mir::MVMAction::Keep
        }
    }
}

pub fn whiles_to_fors(code: &mut mir::MirFunc) {
    WhileToForVisitor.visit_block(&mut code.code)
}
