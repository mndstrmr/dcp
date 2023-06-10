use crate::{mir, expr, lir, dataflow::Abi};

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

// FIXME: I have not proved that sp (or whatever) doesn't change (which it does)...
fn append_expr_to_frame(expr: &mut expr::Expr, base_reg: &str, stack_frame: &mut mir::MirStackFrame, name_gen: &mut NameGen) {
    match expr {
        expr::Expr::Binary { op, lhs, rhs } => {
            append_expr_to_frame(lhs, base_reg, stack_frame, name_gen);
            append_expr_to_frame(rhs, base_reg, stack_frame, name_gen);

            if
                *op == expr::BinaryOp::Sub &&
                let expr::Expr::Name(name) = lhs.as_ref() &&
                *name == base_reg && 
                let expr::Expr::Num(offset) = rhs.as_ref() &&
                *offset >= 0 {
                
                if let Some(local) = stack_frame.get_at(*offset as u64) {
                    *expr = expr::Expr::Ref(Box::new(expr::Expr::Name(local.name.to_string())));
                } else {
                    let name = name_gen.get();
                    stack_frame.insert(mir::MirLocal { name: name.clone(), offset: *offset as u64, size: 0 });
                    *expr = expr::Expr::Ref(Box::new(expr::Expr::Name(name)));
                }
            }
        }
        expr::Expr::Call { func, args } => {
            append_expr_to_frame(func, base_reg, stack_frame, name_gen);
            for arg in args {
                append_expr_to_frame(arg, base_reg, stack_frame, name_gen);
            }
        }
        expr::Expr::Deref { ptr, size } => {
            append_expr_to_frame(ptr, base_reg, stack_frame, name_gen);

            if
                let expr::Expr::Ref(inner) = ptr.as_ref() &&
                let expr::Expr::Name(name) = inner.as_ref() &&
                let Some(local) = stack_frame.get_mut_by_name(name) {
                
                if local.size == 0 {
                    local.size = size.byte_count() as u64;
                    *expr = expr::Expr::Name(name.clone());
                } else if local.size == size.byte_count() as u64 {
                    *expr = expr::Expr::Name(name.clone());
                }
            }
        }
        expr::Expr::Ref(value) => append_expr_to_frame(value, base_reg, stack_frame, name_gen),
        expr::Expr::Unary { expr, .. } => append_expr_to_frame(expr, base_reg, stack_frame, name_gen),
        expr::Expr::Name(_) | expr::Expr::Bool(_) |  expr::Expr::Num(_) | expr::Expr::Func(_) => {}
    }
}

pub fn mem_to_name(nodes: &mut Vec<lir::LirNode>, abi: &Abi) -> mir::MirStackFrame {
    let mut stack_frame = mir::MirStackFrame::new();

    let Some(base_reg) = abi.base_reg else {
        return stack_frame;
    };

    let mut name_gen = NameGen::new();

    for node in nodes {
        for stmt in &mut node.code {
            match stmt {
                lir::Lir::Return(expr) => append_expr_to_frame(expr, base_reg, &mut stack_frame, &mut name_gen),
                lir::Lir::Do(expr) => append_expr_to_frame(expr, base_reg, &mut stack_frame, &mut name_gen),
                lir::Lir::Assign { src, dst } => {
                    append_expr_to_frame(src, base_reg, &mut stack_frame, &mut name_gen);
                    append_expr_to_frame(dst, base_reg, &mut stack_frame, &mut name_gen);
                }
                lir::Lir::Branch { cond: Some(cond), .. } => append_expr_to_frame(cond, base_reg, &mut stack_frame, &mut name_gen),
                lir::Lir::Branch { .. } | lir::Lir::Label(_) => {}
            }
        }
    }

    stack_frame
}
