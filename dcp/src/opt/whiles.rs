use crate::mir::{self, MirVisitorMut};

struct LoopsToWhileVisitor;

impl MirVisitorMut for LoopsToWhileVisitor {
    fn visit_loop(&mut self, code: &mut Vec<mir::Mir>) -> mir::MVMAction {
        if
            let Some(mir::Mir::If { cond, true_then, false_then }) = code.first_mut() &&
            true_then.len() == 1 &&
            let Some(mir::Mir::Break) = true_then.first()
        {
            let guard = cond.neg();

            let mut new_code: Vec<_> = false_then.drain(..).collect();
            new_code.extend(code.drain(1..));

            mir::MVMAction::Replace(mir::Mir::While {
                guard,
                code: new_code
            })
        } else if
            let Some(mir::Mir::If { cond, true_then, false_then }) = code.first_mut() &&
            false_then.len() == 1 &&
            let Some(mir::Mir::Break) = false_then.first()
        {
            let guard = cond.take();

            let mut new_code: Vec<_> = true_then.drain(..).collect();
            new_code.extend(code.drain(1..));

            mir::MVMAction::Replace(mir::Mir::While {
                guard,
                code: new_code
            })
        } else {
            self.visit_block(code);
            mir::MVMAction::Keep
        }
    }
}

pub fn loops_to_whiles(code: &mut mir::MirFunc) {
    LoopsToWhileVisitor.visit_block(&mut code.code)
}
