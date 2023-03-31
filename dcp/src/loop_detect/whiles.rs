use crate::mir::{self, MirVisitorMut};

pub fn loops_to_whiles(code: &mut Vec<mir::Mir>) {
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
            } else {
                self.visit_block(code);
                mir::MVMAction::Keep
            }
        }
    }

    LoopsToWhileVisitor.visit_block(code)
}
