use crate::{mir::{self, MirVisitor, MirVisitorMut}, expr};

fn num_to_name(mut num: usize) -> String {
    const CHARSET: &[u8] = b"abcdefghijklmnopqrstuvwxyz";

    let mut name = String::new();
    loop {
        name.insert(0, CHARSET[num % 26] as char);
        num /= 26;
        if num == 0 {
            break name
        }
        num -= 1;
    }
}

struct NameGen {
    idx: usize
}

impl NameGen {
    fn new() -> NameGen {
        NameGen {
            idx: 0
        }
    }

    fn get(&mut self) -> String {
        self.idx += 1;
        num_to_name(self.idx - 1)
    }
}

fn populate_stack_frame(function: &mut mir::MirFunc, base_reg: &str) {
    struct NameGenMirVisitor<'a> {
        name_gen: NameGen,
        stack_frame: &'a mut mir::MirStackFrame,
        base_reg: &'a str
    }

    impl<'a> MirVisitor for NameGenMirVisitor<'a> {
        fn visit_expr(&mut self, expr: &expr::Expr) {
            match expr {
                expr::Expr::Binary { op, lhs, rhs } => {
                    if
                        *op == expr::BinaryOp::Add &&
                        let expr::Expr::Name(name) = lhs.as_ref() &&
                        *name == self.base_reg && 
                        let expr::Expr::Num(offset) = rhs.as_ref() &&
                        *offset >= 0 {
                        
                        if let None = self.stack_frame.get_at(*offset as u64) {
                            let name = self.name_gen.get();
                            self.stack_frame.insert(mir::MirLocal { name, offset: *offset as u64, size: 0 })
                        }
                    }

                    self.visit_expr(lhs);
                    self.visit_expr(rhs);
                }
                expr::Expr::Call { func, args } => {
                    self.visit_expr(func);
                    for arg in args {
                        self.visit_expr(arg);
                    }
                }
                expr::Expr::Deref { ptr, .. } => self.visit_expr(ptr),
                expr::Expr::Ref(value) => self.visit_expr(value),
                expr::Expr::Unary { expr, .. } => self.visit_expr(expr),
                expr::Expr::Name(_) | expr::Expr::Bool(_) |  expr::Expr::Num(_) | expr::Expr::Func(_) => {}
            }
        }
    }

    NameGenMirVisitor {
        base_reg,
        name_gen: NameGen::new(),
        stack_frame: &mut function.stack_frame
    }.visit_block(&function.code);
}

fn rename(function: &mut mir::MirFunc, base_reg: &str) {
    struct RenameMirVisitor<'a> {
        stack_frame: &'a mut mir::MirStackFrame,
        base_reg: &'a str
    }

    impl<'a> MirVisitorMut for RenameMirVisitor<'a> {
        fn visit_expr(&mut self, expr: &mut expr::Expr) {
            match expr {
                expr::Expr::Binary { op, lhs, rhs } => {
                    self.visit_expr(lhs);
                    self.visit_expr(rhs);
                    
                    if
                        *op == expr::BinaryOp::Add &&
                        let expr::Expr::Name(name) = lhs.as_ref() &&
                        *name == self.base_reg && 
                        let expr::Expr::Num(offset) = rhs.as_ref() &&
                        *offset >= 0 &&
                        let Some(local) = self.stack_frame.get_at(*offset as u64) {
                        *expr = expr::Expr::Ref(Box::new(expr::Expr::Name(local.name.to_string())));
                    }
                }
                expr::Expr::Call { func, args } => {
                    self.visit_expr(func);
                    for arg in args {
                        self.visit_expr(arg);
                    }
                }
                expr::Expr::Deref { ptr, size } => {
                    self.visit_expr(ptr);
                    if
                        let expr::Expr::Ref(inner) = ptr.as_ref() &&
                        let expr::Expr::Name(name) = inner.as_ref() &&
                        let Some(local) = self.stack_frame.get_mut_by_name(name) {
                        
                        if local.size == 0 {
                            local.size = size.byte_count() as u64;
                            *expr = expr::Expr::Name(name.clone());
                        } else if local.size == size.byte_count() as u64 {
                            *expr = expr::Expr::Name(name.clone());
                        }
                    }
                }
                expr::Expr::Ref(value) => {
                    self.visit_expr(value);
                }
                expr::Expr::Unary { expr, .. } => self.visit_expr(expr),
                expr::Expr::Name(_) | expr::Expr::Bool(_) |  expr::Expr::Num(_) | expr::Expr::Func(_) => {}
            }
        }
    }

    RenameMirVisitor {
        base_reg,
        stack_frame: &mut function.stack_frame
    }.visit_block(&mut function.code)
}

pub fn name_locals(function: &mut mir::MirFunc, base_reg: &str) {
    populate_stack_frame(function, base_reg);
    rename(function, base_reg);
}
