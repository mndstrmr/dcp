use std::fmt::Display;

use wasmparser::Operator;

use crate::{dataflow::Abi, lir, expr, ty};

pub fn abi() -> Abi {
    Abi {
        args: vec!["l0", "l1", "l2", "l3", "l4", "l5", "l6", "l7"],
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

    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
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

    pub fn pop(&mut self) -> lir::Label {
        let (_, end, _) = self.blocks.pop().expect("Empty block stack");
        lir::Label(end)
    }

    pub fn branch_target_rel(&self, rel: usize) -> lir::Label {
        let (start, end, loops) = self.blocks[self.blocks.len() - 1 - rel];
        if loops { lir::Label(start) } else { lir::Label(end) }
    }

    pub fn ret(&self) -> lir::Label {
        lir::Label(self.blocks[0].1)
    }
}

pub enum TranslationError<'a> {
    UnknownInstruction(wasmparser::Operator<'a>),
    Decode,
    BadFunctionIndex
}

impl<'a> Display for TranslationError<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TranslationError::UnknownInstruction(insn) => write!(f, "do not know how to translate {:?}", insn),
            TranslationError::Decode => write!(f, "malformed code"),
            TranslationError::BadFunctionIndex => write!(f, "function indexed a type which does not exist")
        }
    }
}

pub fn to_lir<'a>(function: &'a wasmparser::FunctionBody, func_types: &[wasmparser::FuncType], raw_types: &[wasmparser::FuncType]) -> Result<lir::LirFunc, TranslationError<'a>> {
    let mut block = lir::LirFuncBuilder::new();

    let mut blocks = BlockStack::new();
    let mut stack = StackNaming::new();

    for insn in function.get_operators_reader().expect("Could not make operators reader") {
        let insn = match insn {
            Ok(insn) => insn,
            Err(_) => return Err(TranslationError::Decode)
        };
        // println!("{:?}", insn);
        gen_insn(insn, &mut block, &mut blocks, &mut stack, func_types, raw_types)?;
    }

    if stack.is_empty() {
        block.push(lir::Lir::Return(expr::Expr::Num(0)));
    } else {
        block.push(lir::Lir::Return(stack.pop().expr()));
    }

    Ok(block.block())
}

fn gen_insn<'a>(
    insn: wasmparser::Operator<'a>,
    block: &mut lir::LirFuncBuilder,
    blocks: &mut BlockStack, stack: &mut StackNaming,
    func_types: &[wasmparser::FuncType],
    raw_types: &[wasmparser::FuncType]
) -> Result<(), TranslationError<'a>> {
    match insn {
        Operator::End => {
            let end = blocks.pop();
            block.push(lir::Lir::Label(end));
            Ok(())
        }
        Operator::LocalGet { local_index } => {
            let dst = stack.push();
            block.push(lir::Lir::Assign { src: local_ref(local_index as usize), dst: dst.expr() });
            Ok(())
        }
        Operator::LocalSet { local_index } => {
            let src = stack.pop();
            block.push(lir::Lir::Assign { src: src.expr(), dst: local_ref(local_index as usize) });
            Ok(())
        }
        Operator::LocalTee { local_index } => {
            let src = stack.peek();
            block.push(lir::Lir::Assign { src: src.expr(), dst: local_ref(local_index as usize) });
            Ok(())
        }
        Operator::I32Const { value } => {
            let dst = stack.push();
            block.push(lir::Lir::Assign { src: expr::Expr::Num(value as i64), dst: dst.expr() });
            Ok(())
        }
        Operator::I64Const { value } => {
            let dst = stack.push();
            block.push(lir::Lir::Assign { src: expr::Expr::Num(value), dst: dst.expr() });
            Ok(())
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
            Ok(())
        }
        Operator::I32Add | Operator::I64Add | Operator::F32Add | Operator::F64Add |
        Operator::I32And | Operator::I64And |
        Operator::I32Or | Operator::I64Or |
        Operator::I32Shl | Operator::I64Shl |
        Operator::I32ShrU | Operator::I64ShrU |
        Operator::I32Xor | Operator::I64Xor |
        Operator::I32Sub | Operator::I64Sub | Operator::F32Sub | Operator::F64Sub => {
            let src2 = stack.pop();
            let src1 = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Binary {
                    op: match insn {
                        Operator::I32Add | Operator::I64Add | Operator::F32Add | Operator::F64Add => expr::BinaryOp::Add,
                        Operator::I32And | Operator::I64And => expr::BinaryOp::And,
                        Operator::I32Or | Operator::I64Or => expr::BinaryOp::Or,
                        Operator::I32Sub | Operator::I64Sub | Operator::F32Sub | Operator::F64Sub => expr::BinaryOp::Sub,
                        Operator::I32Shl | Operator::I64Shl => expr::BinaryOp::Shl,
                        Operator::I32ShrU | Operator::I64ShrU => expr::BinaryOp::Shr,
                        Operator::I32Xor | Operator::I64Xor => expr::BinaryOp::Xor,
                        _ => unreachable!()
                    },
                    lhs: src1.bexpr(),
                    rhs: src2.bexpr(),
                },
                dst: dst.expr()
            });
            Ok(())
        }
        Operator::I32Rotr | Operator::I64Rotr |
        Operator::I32Rotl | Operator::I64Rotl => {
            let src2 = stack.pop();
            let src1 = stack.pop();
            let dst = stack.push();

            block.push(lir::Lir::Assign {
                dst: dst.expr(),
                src: expr::Expr::Call {
                    func: Box::new(expr::Expr::BuiltIn(match insn {
                        Operator::I32Rotr | Operator::I64Rotr => expr::BuiltIn::Rotr,
                        Operator::I32Rotl | Operator::I64Rotl => expr::BuiltIn::Rotl,
                        _ => unreachable!()
                    })),
                    args: vec![src1.expr(), src2.expr()]
                }
            });
            Ok(())
        }
        Operator::I32Ctz | Operator::I64Ctz |
        Operator::I32Clz | Operator::I64Clz => {
            let src1 = stack.pop();
            let dst = stack.push();

            block.push(lir::Lir::Assign {
                dst: dst.expr(),
                src: expr::Expr::Call {
                    func: Box::new(expr::Expr::BuiltIn(match insn {
                        Operator::I32Ctz | Operator::I64Ctz => expr::BuiltIn::Ctz,
                        Operator::I32Clz | Operator::I64Clz => expr::BuiltIn::Clz,
                        _ => unreachable!()
                    })),
                    args: vec![src1.expr()]
                }
            });
            Ok(())
        }
        Operator::GlobalGet { global_index } => {
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Name(match global_index {
                    0 => "sp".to_string(),
                    _ => format!("g{global_index}").to_string()
                }),
                dst: dst.expr()
            });
            Ok(())
        }
        Operator::GlobalSet { global_index } => {
            let src = stack.pop();
            block.push(lir::Lir::Assign {
                src: src.expr(),
                dst: expr::Expr::Name(match global_index {
                    0 => "sp".to_string(),
                    _ => format!("g{global_index}").to_string()
                }) // FIXME: Mark as being global somehow
            });
            Ok(())
        }
        Operator::I64Store { memarg: wasmparser::MemArg { offset, .. } } |
        Operator::I32Store { memarg: wasmparser::MemArg { offset, .. } } |
        Operator::I32Store8 { memarg: wasmparser::MemArg { offset, .. } } |
        Operator::I32Store16 { memarg: wasmparser::MemArg { offset, .. } } => {
            let src = stack.pop();
            let addr = stack.pop();
            block.push(lir::Lir::Assign {
                dst: expr::Expr::Deref {
                    ptr: Box::new(expr::Expr::Binary {
                        op: expr::BinaryOp::Add,
                        lhs: addr.bexpr(),
                        rhs: Box::new(expr::Expr::Num(offset as i64)),
                    }),
                    size: match insn {
                        Operator::I64Store { .. } => ty::Size::Size64,
                        Operator::I32Store { .. } => ty::Size::Size32,
                        Operator::I32Store8 { .. } => ty::Size::Size8,
                        Operator::I32Store16 { .. } => ty::Size::Size16,
                        _ => unreachable!()
                    }
                },
                src: src.expr()
            });
            Ok(())
        }
        Operator::I64Load { memarg: wasmparser::MemArg { offset, .. } } |
        Operator::I32Load { memarg: wasmparser::MemArg { offset, .. } } |
        Operator::I32Load8U { memarg: wasmparser::MemArg { offset, .. } } |
        Operator::I32Load16U { memarg: wasmparser::MemArg { offset, .. } } => {
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
                    size: match insn {
                        Operator::I64Load { .. } => ty::Size::Size64,
                        Operator::I32Load { .. } => ty::Size::Size32,
                        Operator::I32Load8U { .. } => ty::Size::Size8,
                        Operator::I32Load16U { .. } => ty::Size::Size16,
                        _ => unreachable!()
                    }
                }
            });
            Ok(())
        }
        Operator::Return => {
            block.push(lir::Lir::Branch { cond: None, target: blocks.ret() });
            block.push(lir::Lir::Label(blocks.tmp_label()));
            Ok(())
        }
        Operator::Block { blockty: wasmparser::BlockType::Empty } => {
            let start = blocks.push_block(false);
            block.push(lir::Lir::Label(start));
            Ok(())
        }
        Operator::Loop { blockty: wasmparser::BlockType::Empty } => {
            let start = blocks.push_block(true);
            block.push(lir::Lir::Label(start));
            Ok(())
        }
        Operator::I32LtS | Operator::I32LeS |
        Operator::I32LtU | Operator::I32LeU |
        Operator::I32GtS | Operator::I32GeS |
        Operator::I32GtU | Operator::I32GeU |
        Operator::I32Eq | Operator::I32Ne => {
            let src2 = stack.pop();
            let src1 = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                src: expr::Expr::Binary {
                    op: match insn {
                        Operator::I32LtS => expr::BinaryOp::Lt,
                        Operator::I32LtU => expr::BinaryOp::Lt,
                        Operator::I32LeS => expr::BinaryOp::Le,
                        Operator::I32LeU => expr::BinaryOp::Le,
                        Operator::I32GtS => expr::BinaryOp::Gt,
                        Operator::I32GtU => expr::BinaryOp::Gt,
                        Operator::I32GeS => expr::BinaryOp::Ge,
                        Operator::I32GeU => expr::BinaryOp::Ge,
                        Operator::I32Eq => expr::BinaryOp::Eq,
                        Operator::I32Ne => expr::BinaryOp::Ne,
                        _ => unreachable!()
                    },
                    lhs: src1.bexpr(),
                    rhs: src2.bexpr(),
                },
                dst: dst.expr()
            });
            Ok(())
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
            Ok(())
        }
        Operator::BrIf { relative_depth } => {
            let target = blocks.branch_target_rel(relative_depth as usize);
            let src = stack.pop();
            block.push(lir::Lir::Branch {
                cond: Some(src.expr()),
                target
            });
            block.push(lir::Lir::Label(blocks.tmp_label()));
            Ok(())
        }
        Operator::Br { relative_depth } => {
            let target = blocks.branch_target_rel(relative_depth as usize);
            block.push(lir::Lir::Branch {
                cond: None,
                target
            });
            block.push(lir::Lir::Label(blocks.tmp_label()));
            Ok(())
        }
        Operator::Call { function_index } => {
            if function_index as usize >= func_types.len() {
                return Err(TranslationError::BadFunctionIndex)
            }

            let mut args = Vec::new();
            for _ in func_types[function_index as usize].params() {
                args.push(stack.pop().expr());
            }

            if func_types[function_index as usize].results().len() == 0 {
                block.push(lir::Lir::Do(expr::Expr::Call {
                    func: Box::new(expr::Expr::Func(expr::FuncId(function_index as usize))),
                    args
                }));
            } else {
                let res = stack.push();
                assert_eq!(func_types[function_index as usize].results().len(), 1);
                block.push(lir::Lir::Assign {
                    dst: res.expr(),
                    src: expr::Expr::Call {
                        func: Box::new(expr::Expr::Func(expr::FuncId(function_index as usize))),
                        args
                    }
                });
            }

            Ok(())
        }
        Operator::Drop => {
            let res = stack.pop();
            // Small optimisation: if we drop something immediately after making it, get rid of the assignment
            if let Some(lir::Lir::Assign { dst: expr::Expr::Name(nm), src }) = block.last() && nm == &res.name() {
                let new = lir::Lir::Do(src.take());
                block.pop();
                block.push(new);
                Ok(())
            } else {
                Ok(())
            }
        }
        // FIXME: Add real support for this
        Operator::CallIndirect { type_index, table_index, .. } => {
            let mut args = Vec::new();
            for _ in raw_types[type_index as usize].params() {
                args.push(stack.pop().expr());
            }

            if raw_types[type_index as usize].results().len() == 0 {
                block.push(lir::Lir::Do(expr::Expr::Call {
                    func: Box::new(expr::Expr::Num(table_index as i64)),
                    args
                }));
            } else {
                let res = stack.push();
                assert_eq!(raw_types[type_index as usize].results().len(), 1);
                block.push(lir::Lir::Assign {
                    dst: res.expr(),
                    src: expr::Expr::Call {
                        func: Box::new(expr::Expr::Num(table_index as i64)),
                        args
                    }
                });
            }

            Ok(())
        }
        Operator::I32WrapI64 => {
            let src = stack.pop();
            let dst = stack.push();
            block.push(lir::Lir::Assign {
                dst: dst.expr(),
                src: expr::Expr::Binary {
                    op: expr::BinaryOp::And,
                    lhs: src.bexpr(),
                    rhs: Box::new(expr::Expr::Num(0xffffffff))
                }
            });
            Ok(())
        },
        // Types currently don't exist, so this is I guess meaningless
        Operator::I64Extend32S | Operator::I64ExtendI32S | Operator::I64ExtendI32U |
        Operator::I64Extend16S => Ok(()),
        Operator::Unreachable => {
            // FIXME: Add something here
            Ok(())
        },
        _ => Err(TranslationError::UnknownInstruction(insn))
    }
}
