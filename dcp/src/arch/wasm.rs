use wasmparser::Operator;

use crate::{dataflow::Abi, lir, expr, ty};

pub fn abi() -> Abi {
    Abi {
        args: vec![],
        global: vec!["sp"],
        base_reg: None,
        callee_saved: vec![],
        eliminate: vec![]
    }
}

fn local_name(idx: usize) -> String {
    format!("l{idx}")
}

fn local_ref(idx: usize) -> expr::Expr {
    expr::Expr::Name(local_name(idx))
}

struct StackNaming {
    names: Vec<usize>,
    next: usize
}

struct StackName(usize);

impl StackName {
    pub fn name(&self) -> String {
        format!("s{}", self.0)
    }

    pub fn expr(&self) -> expr::Expr {
        expr::Expr::Name(self.name())
    }

    pub fn bexpr(&self) -> Box<expr::Expr> {
        Box::new(expr::Expr::Name(self.name()))
    }
}

impl StackNaming {
    pub fn new() -> StackNaming {
        StackNaming {
            names: vec![],
            next: 0
        }
    }

    pub fn push(&mut self) -> StackName {
        self.names.push(self.next);
        self.next += 1;
        StackName(self.next - 1)
    }

    pub fn pop(&mut self) -> StackName {
        StackName(self.names.pop().unwrap())
    }

    pub fn peek(&self) -> StackName {
        StackName(self.names.last().cloned().unwrap())
    }
}

struct BlockStack {
    blocks: Vec<(usize, usize, bool)>,
    next: usize
}

impl BlockStack {
    pub fn new() -> BlockStack {
        BlockStack {
            blocks: vec![(0, 1, false)],
            next: 2
        }
    }

    pub fn tmp_label(&mut self) -> lir::Label {
        self.next += 1;
        lir::Label(self.next - 1)
    }

    pub fn push_block(&mut self, loops: bool) -> lir::Label {
        self.blocks.push((self.next, self.next + 1, loops));
        self.next += 2;
        lir::Label(self.next - 2)
    }

    pub fn pop(&mut self) -> (lir::Label, lir::Label, bool) {
        let (start, end, loops) = self.blocks.pop().expect("Empty block stack");
        (lir::Label(start), lir::Label(end), loops)
    }

    pub fn branch_target_rel(&self, rel: usize) -> lir::Label {
        let (start, end, loops) = self.blocks[self.blocks.len() - 1 - rel];
        if loops { lir::Label(start) } else { lir::Label(end) }
    }

    pub fn ret(&self) -> lir::Label {
        lir::Label(self.blocks[0].1)
    }
}

pub fn to_lir(function: &wasmparser::FunctionBody, func_types: &[wasmparser::FuncType]) -> Result<lir::LirFunc, String> {
    let mut block = lir::LirFuncBuilder::new();

    let mut blocks = BlockStack::new();
    let mut stack = StackNaming::new();

    for insn in function.get_operators_reader().expect("Could not make operators reader") {
        println!("{:?}", insn);
        gen_insn(insn.expect("Could not decode instruction"), &mut block, &mut blocks, &mut stack, func_types);
    }

    block.push(lir::Lir::Return(stack.pop().expr()));

    Ok(block.block())
}

fn gen_insn(
    insn: wasmparser::Operator,
    block: &mut lir::LirFuncBuilder,
    blocks: &mut BlockStack, stack: &mut StackNaming,
    func_types: &[wasmparser::FuncType]
) {
    match insn {
        Operator::End => {
            let (start, end, loops) = blocks.pop();
            if loops {
                block.push(lir::Lir::Branch { cond: None, target: start });
            }
            block.push(lir::Lir::Label(end));
        }
        Operator::LocalGet { local_index } => {
            let dst = stack.push();
            block.push(lir::Lir::Assign { src: local_ref(local_index as usize), dst: dst.expr() });
        }
        Operator::LocalSet { local_index } => {
            let src = stack.pop();
            block.push(lir::Lir::Assign { src: src.expr(), dst: local_ref(local_index as usize) });
        }
        Operator::LocalTee { local_index } => {
            let src = stack.peek();
            block.push(lir::Lir::Assign { src: src.expr(), dst: local_ref(local_index as usize) });
        }
        Operator::I32Const { value } => {
            let dst = stack.push();
            block.push(lir::Lir::Assign { src: expr::Expr::Num(value as i64), dst: dst.expr() });
        }
        Operator::I64Const { value } => {
            let dst = stack.push();
            block.push(lir::Lir::Assign { src: expr::Expr::Num(value), dst: dst.expr() });
        }
        Operator::Select => {
            let i = stack.pop();
            let v2 = stack.pop();
            let v1 = stack.pop();
            let dst = stack.push();

            let end = blocks.tmp_label();
            let step = blocks.tmp_label();

            block.push(lir::Lir::Branch {
                cond: Some(i.expr()),
                target: step
            });
            block.push(lir::Lir::Label(blocks.tmp_label()));
            block.push(lir::Lir::Assign { src: v2.expr(), dst: dst.expr() });
            block.push(lir::Lir::Branch {
                cond: None,
                target: end
            });
            block.push(lir::Lir::Label(step));
            block.push(lir::Lir::Assign { src: v1.expr(), dst: dst.expr() });
            block.push(lir::Lir::Label(end));
        }
        Operator::I32Add | Operator::I64Add | Operator::F32Add | Operator::F64Add => {
            let src2 = stack.pop();
            let src1 = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Binary {
                    op: expr::BinaryOp::Add,
                    lhs: src1.bexpr(),
                    rhs: src2.bexpr(),
                },
                dst: dst.expr()
            });
        }
        Operator::I32And | Operator::I64And => {
            let src2 = stack.pop();
            let src1 = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Binary {
                    op: expr::BinaryOp::And,
                    lhs: src1.bexpr(),
                    rhs: src2.bexpr(),
                },
                dst: dst.expr()
            });
        }
        Operator::I32Sub | Operator::I64Sub | Operator::F32Sub | Operator::F64Sub => {
            let src2 = stack.pop();
            let src1 = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Binary {
                    op: expr::BinaryOp::Sub,
                    lhs: src1.bexpr(),
                    rhs: src2.bexpr(),
                },
                dst: dst.expr()
            });
        }
        Operator::GlobalGet { global_index: 0 } => {
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Name("sp".to_string()),
                dst: dst.expr()
            });
        }
        Operator::GlobalSet { global_index: 0 } => {
            let src = stack.pop();
            block.push(lir::Lir::Assign {
                src: src.expr(),
                dst: expr::Expr::Name("sp".to_string())
            });
        }
        Operator::I32Store { memarg: wasmparser::MemArg { offset, .. } } => {
            let src = stack.pop();
            let addr = stack.pop();
            block.push(lir::Lir::Assign {
                dst: expr::Expr::Deref {
                    ptr: Box::new(expr::Expr::Binary {
                        op: expr::BinaryOp::Add,
                        lhs: addr.bexpr(),
                        rhs: Box::new(expr::Expr::Num(offset as i64)),
                    }),
                    size: ty::Size::Size32
                },
                src: src.expr()
            });
        }
        Operator::I32Load { memarg: wasmparser::MemArg { offset, .. } } => {
            let addr = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                dst: dst.expr(),
                src: expr::Expr::Deref {
                    ptr: Box::new(expr::Expr::Binary {
                        op: expr::BinaryOp::Add,
                        lhs: addr.bexpr(),
                        rhs: Box::new(expr::Expr::Num(offset as i64)),
                    }),
                    size: ty::Size::Size32
                }
            });
        }
        Operator::Return => {
            block.push(lir::Lir::Branch { cond: None, target: blocks.ret() });
            block.push(lir::Lir::Label(blocks.tmp_label()));
        }
        Operator::Block { blockty: wasmparser::BlockType::Empty } => {
            let start = blocks.push_block(false);
            block.push(lir::Lir::Label(start));
        }
        Operator::Loop { blockty: wasmparser::BlockType::Empty } => {
            let start = blocks.push_block(true);
            block.push(lir::Lir::Label(start));
        }
        Operator::I32LtS => {
            let src2 = stack.pop();
            let src1 = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Binary {
                    op: expr::BinaryOp::Lt,
                    lhs: src1.bexpr(),
                    rhs: src2.bexpr(),
                },
                dst: dst.expr()
            });
        }
        Operator::I32Eqz => {
            let src = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Unary {
                    op: expr::UnaryOp::Not,
                    expr: src.bexpr(),
                },
                dst: dst.expr()
            });
        }
        Operator::BrIf { relative_depth } => {
            let target = blocks.branch_target_rel(relative_depth as usize);
            let src = stack.pop();
            block.push(lir::Lir::Branch {
                cond: Some(src.expr()),
                target
            });
            block.push(lir::Lir::Label(blocks.tmp_label()));
        }
        Operator::Br { relative_depth } => {
            let target = blocks.branch_target_rel(relative_depth as usize);
            block.push(lir::Lir::Branch {
                cond: None,
                target
            });
            block.push(lir::Lir::Label(blocks.tmp_label()));
        }
        Operator::Call { function_index } => {
            assert_eq!(func_types[function_index as usize].results().len(), 1);
            let mut args = Vec::new();
            for _ in func_types[function_index as usize].params() {
                args.push(stack.pop().expr());
            }
            let res = stack.push();
            block.push(lir::Lir::Assign {
                dst: res.expr(),
                src: expr::Expr::Call {
                    func: Box::new(expr::Expr::Func(expr::FuncId(function_index as usize))),
                    args
                }
            });
        }
        _ => todo!()
    }
}
