use std::collections::HashMap;

use capstone::{
    arch::{
        arm64::{Arm64CC, Arm64Insn, Arm64OpMem, Arm64Operand, Arm64OperandType, Arm64Reg},
        ArchOperand,
    },
    prelude::*,
};

use crate::{expr, lir, ty};

const CMP: &'static str = "cmp";

pub const X: &[&'static str] = &[
    "x0", "x1", "x2", "x3", "x4", "x5", "x6", "x7", "x8", "x9", "x10", "x11", "x12", "x13", "x14",
    "x15", "x16", "x17", "x18", "x19", "x20", "x21", "x22", "x23", "x24", "x25", "x26", "x27",
    "x28", "fp", "lr", "sp",
];

fn name(reg: RegId) -> expr::Expr {
    expr::Expr::Name(match reg.0 as u32 {
        Arm64Reg::ARM64_REG_WZR | Arm64Reg::ARM64_REG_XZR => panic!("Zero reg"),

        Arm64Reg::ARM64_REG_FP => X[29].to_string(),
        Arm64Reg::ARM64_REG_LR => X[30].to_string(),
        Arm64Reg::ARM64_REG_SP => X[31].to_string(),

        x @ 216..=244 => X[x as usize - 216].to_string(),
        x @ 185..=213 => X[x as usize - 185].to_string(),
        _ => panic!("bad reg {:?}", reg),
    })
}

fn reg_size(reg: RegId) -> ty::Size {
    match reg.0 as u32 {
        Arm64Reg::ARM64_REG_WZR => ty::Size::Size32,
        Arm64Reg::ARM64_REG_XZR => ty::Size::Size64,

        Arm64Reg::ARM64_REG_FP => ty::Size::Size64,
        Arm64Reg::ARM64_REG_LR => ty::Size::Size64,
        Arm64Reg::ARM64_REG_SP => ty::Size::Size64,

        216..=244 => ty::Size::Size64,
        185..=213 => ty::Size::Size32,
        _ => panic!("bad reg {:?}", reg),
    }
}

fn cc_to_lir(cc: Arm64CC) -> Option<expr::UnaryOp> {
    match cc {
        Arm64CC::ARM64_CC_INVALID => None,
        Arm64CC::ARM64_CC_EQ => Some(expr::UnaryOp::CmpEq),
        Arm64CC::ARM64_CC_NE => Some(expr::UnaryOp::CmpNe),
        Arm64CC::ARM64_CC_LT => Some(expr::UnaryOp::CmpLt),
        Arm64CC::ARM64_CC_GT => Some(expr::UnaryOp::CmpGt),
        Arm64CC::ARM64_CC_LE => Some(expr::UnaryOp::CmpLe),
        Arm64CC::ARM64_CC_GE => Some(expr::UnaryOp::CmpGe),
        
        Arm64CC::ARM64_CC_LO => Some(expr::UnaryOp::CmpLt),
        Arm64CC::ARM64_CC_HI => Some(expr::UnaryOp::CmpGt),

        _ => todo!()
    }
}

fn op_to_mem_addr(op: &ArchOperand) -> expr::Expr {
    let op = match op {
        ArchOperand::Arm64Operand(op) => op,
        _ => panic!("not arm64?")
    };

    match op.op_type {
        Arm64OperandType::Mem(mem) => mem_to_lir_addr(mem),
        _ => todo!("Not a mem op: {:?}", op)
    }
}

fn op_reg_size(op: &ArchOperand) -> ty::Size {
    let op = match op {
        ArchOperand::Arm64Operand(op) => op,
        _ => panic!("not arm64?")
    };

    match op.op_type {
        Arm64OperandType::Reg(reg) => reg_size(reg),
        _ => todo!("Operand: {:?}", op)
    }
}

fn op_to_non_mem(op: &ArchOperand) -> expr::Expr {
    let op = match op {
        ArchOperand::Arm64Operand(op) => op,
        _ => panic!("not arm64?")
    };

    match op.op_type {
        Arm64OperandType::Reg(reg) =>
            if reg.0 == Arm64Reg::ARM64_REG_WZR as u16 || reg.0 == Arm64Reg::ARM64_REG_XZR as u16 {
                expr::Expr::Num(0)
            } else {
                name(reg)
            }
        Arm64OperandType::Imm(imm) => expr::Expr::Num(imm),
        Arm64OperandType::Mem(_) => panic!("Memory operand"),
        _ => todo!("Operand: {:?}", op)
    }
}

fn op_to_expr(op: &ArchOperand, size: ty::Size) -> expr::Expr {
    let op = match op {
        ArchOperand::Arm64Operand(op) => op,
        _ => panic!("not arm64?")
    };

    match op.op_type {
        Arm64OperandType::Reg(reg) =>
            if reg.0 == Arm64Reg::ARM64_REG_WZR as u16 || reg.0 == Arm64Reg::ARM64_REG_XZR as u16 {
                expr::Expr::Num(0)
            } else {
                name(reg)
            }
        Arm64OperandType::Imm(imm) => expr::Expr::Num(imm),
        Arm64OperandType::Mem(mem) => expr::Expr::Deref {
            ptr: Box::new(mem_to_lir_addr(mem)),
            size
        },
        _ => todo!("Operand: {:?}", op)
    }
}

fn mem_to_lir_addr(mem: Arm64OpMem) -> expr::Expr {
    assert_eq!(mem.index().0, 0);

    let mut expr = None;
    if mem.base().0 != 0 {
        expr = Some(name(mem.base()));
    }

    if mem.disp() != 0 {
        if let Some(expr_) = expr {
            expr = Some(expr::Expr::Binary {
                op: expr::BinaryOp::Add,
                lhs: Box::new(expr_),
                rhs: Box::new(expr::Expr::Num(mem.disp() as i64))
            });
        } else {
            expr = Some(expr::Expr::Num(mem.disp() as i64));
        }
    }

    match expr {
        None => expr::Expr::Num(0),
        Some(expr) => expr,
    }
}

pub fn to_lir(data: &[u8], base: u64, addr_to_func: &HashMap<u64, expr::FuncId>) -> Result<lir::LirFunc, String> {
    let cs = Capstone::new()
        .arm64()
        .mode(arch::arm64::ArchMode::Arm)
        .detail(true)
        .build()
        .expect("Could not build cs object");

    let insns = cs.disasm_all(data, base).expect("Could not disassemble");

    let mut block = lir::LirFuncBuilder::new();
    let mut addr_to_label = HashMap::new();

    for insn in insns.as_ref() {
        let detail = cs
            .insn_detail(insn)
            .expect("Could not object cs instruction detail");
        let arch_detail = detail.arch_detail();
        let ops = arch_detail.operands();
        let arch_detail = arch_detail.arm64().unwrap();

        if let Some(label) = addr_to_label.get(&insn.address()) {
            block.push(lir::Lir::Label(*label));
        } else {
            let label = block.new_label();
            block.push(lir::Lir::Label(label));
            addr_to_label.insert(insn.address(), label);
        }

        match Arm64Insn::from(insn.id().0) {
            Arm64Insn::ARM64_INS_SUB => {
                let dst = op_to_non_mem(&ops[0]);
                let src1 = op_to_non_mem(&ops[1]);
                let src2 = op_to_non_mem(&ops[2]);
                block.push(lir::Lir::Assign {
                    dst,
                    src: expr::Expr::Binary {
                        op: expr::BinaryOp::Sub,
                        lhs: Box::new(src1),
                        rhs: Box::new(src2),
                    },
                });
            }
            Arm64Insn::ARM64_INS_ADD => {
                let dst = op_to_non_mem(&ops[0]);
                let src1 = op_to_non_mem(&ops[1]);
                let src2 = op_to_non_mem(&ops[2]);
                block.push(lir::Lir::Assign {
                    dst,
                    src: expr::Expr::Binary {
                        op: expr::BinaryOp::Add,
                        lhs: Box::new(src1),
                        rhs: Box::new(src2),
                    },
                });
            }
            Arm64Insn::ARM64_INS_MOV | Arm64Insn::ARM64_INS_LDR | Arm64Insn::ARM64_INS_LDUR => {
                let dst = op_to_non_mem(&ops[0]);
                let src = op_to_expr(&ops[1], op_reg_size(&ops[0]));
                block.push(lir::Lir::Assign {
                    dst, src
                });
            }
            Arm64Insn::ARM64_INS_STR | Arm64Insn::ARM64_INS_STUR => {
                let dst = op_to_expr(&ops[1], op_reg_size(&ops[0]));
                let src = op_to_non_mem(&ops[0]);
                block.push(lir::Lir::Assign {
                    dst, src
                });
            }
            Arm64Insn::ARM64_INS_CMP => {
                let src1 = op_to_non_mem(&ops[0]);
                let src2 = op_to_non_mem(&ops[1]);

                block.push(lir::Lir::Assign {
                    dst: expr::Expr::Name(CMP.to_string()),
                    src: expr::Expr::Binary {
                        op: expr::BinaryOp::Cmp,
                        lhs: Box::new(src1.clone()),
                        rhs: Box::new(src2.clone()),
                    },
                });
            }
            Arm64Insn::ARM64_INS_SUBS => {
                let dst = op_to_non_mem(&ops[0]);
                let src1 = op_to_non_mem(&ops[1]);
                let src2 = op_to_non_mem(&ops[2]);

                block.push(lir::Lir::Assign {
                    dst: expr::Expr::Name(CMP.to_string()),
                    src: expr::Expr::Binary {
                        op: expr::BinaryOp::Cmp,
                        lhs: Box::new(src1.clone()),
                        rhs: Box::new(src2.clone()),
                    },
                });

                block.push(lir::Lir::Assign {
                    dst,
                    src: expr::Expr::Binary {
                        op: expr::BinaryOp::Sub,
                        lhs: Box::new(src1.clone()),
                        rhs: Box::new(src2.clone()),
                    },
                });
            }
            Arm64Insn::ARM64_INS_RET => {
                block.push(lir::Lir::Return(expr::Expr::Name(X[0].to_string())));
            }
            Arm64Insn::ARM64_INS_CSEL => {
                let cond = match cc_to_lir(arch_detail.cc()) {
                    None => None,
                    Some(op) => Some(expr::Expr::Unary {
                        op,
                        expr: Box::new(expr::Expr::Name(CMP.to_string())),
                    }),
                };

                let dst = op_to_non_mem(&ops[0]);
                let src1 = op_to_non_mem(&ops[1]);
                let src2 = op_to_non_mem(&ops[2]);

                let label1 = block.new_label();
                let label2 = block.new_label();
                let label3 = block.new_label();

                block.push(lir::Lir::Branch {
                    cond,
                    target: label2.clone(),
                });
                block.push(lir::Lir::Label(label1));
                block.push(lir::Lir::Assign {
                    dst: dst.clone(),
                    src: src2,
                });
                block.push(lir::Lir::Branch {
                    cond: None,
                    target: label3.clone(),
                });
                block.push(lir::Lir::Label(label2));
                block.push(lir::Lir::Assign {
                    dst,
                    src: src1,
                });
                block.push(lir::Lir::Label(label3));
            }
            Arm64Insn::ARM64_INS_CSET => {
                let cond = match cc_to_lir(arch_detail.cc()) {
                    None => expr::Expr::Num(1),
                    Some(op) => expr::Expr::Unary {
                        op,
                        expr: Box::new(expr::Expr::Name(CMP.to_string())),
                    },
                };

                let dst = op_to_non_mem(&ops[0]);

                block.push(lir::Lir::Assign {
                    dst: dst.clone(),
                    src: cond,
                });
            }
            Arm64Insn::ARM64_INS_BL => {
                let addr = match &ops[0] {
                    ArchOperand::Arm64Operand(Arm64Operand {
                        op_type: Arm64OperandType::Imm(val),
                        ..
                    }) =>
                        match addr_to_func.get(&(*val as u64)) {
                            Some(func) => expr::Expr::Func(*func),
                            None => expr::Expr::Num(*val),
                        },
                    _ => panic!("Branch operand type"),
                };

                block.push(lir::Lir::Assign {
                    dst: expr::Expr::Name(X[0].to_string()),
                    src: expr::Expr::Call {
                        func: Box::new(addr),
                        args: vec![],
                    },
                });
            }
            Arm64Insn::ARM64_INS_B => {
                let cond = match cc_to_lir(arch_detail.cc()) {
                    None => None,
                    Some(op) => Some(expr::Expr::Unary {
                        op,
                        expr: Box::new(expr::Expr::Name(CMP.to_string())),
                    }),
                };

                let target = match &ops[0] {
                    ArchOperand::Arm64Operand(Arm64Operand {
                        op_type: Arm64OperandType::Imm(val),
                        ..
                    }) => {
                        if let Some(label) = addr_to_label.get(&(*val as u64)) {
                            *label
                        } else {
                            let label = block.new_label();
                            addr_to_label.insert(*val as u64, label);
                            label
                        }
                    }
                    _ => panic!("Branch operand type"),
                };

                block.push(lir::Lir::Branch {
                    cond,
                    target,
                });
            }
            Arm64Insn::ARM64_INS_TBNZ => {
                let src = op_to_non_mem(&ops[0]);

                let target = match &ops[2] {
                    ArchOperand::Arm64Operand(Arm64Operand {
                        op_type: Arm64OperandType::Imm(val),
                        ..
                    }) => {
                        if let Some(label) = addr_to_label.get(&(*val as u64)) {
                            *label
                        } else {
                            let label = block.new_label();
                            addr_to_label.insert(*val as u64, label);
                            label
                        }
                    }
                    _ => panic!("Branch operand type"),
                };

                block.push(lir::Lir::Branch {
                    cond: Some(src),
                    target,
                });
            }
            // FIXME: This is wrong. What if it updates itself?
            Arm64Insn::ARM64_INS_STP => {
                let src1 = op_to_non_mem(&ops[0]);
                let src2 = op_to_non_mem(&ops[1]);
                let dest = op_to_mem_addr(&ops[2]);
                let size = op_reg_size(&ops[0]);

                block.push(lir::Lir::Assign {
                    dst: expr::Expr::Deref {
                        ptr: Box::new(dest.clone()),
                        size
                    },
                    src: src1,
                });

                block.push(lir::Lir::Assign {
                    dst: expr::Expr::Deref {
                        ptr: Box::new(expr::Expr::Binary {
                            op: expr::BinaryOp::Add,
                            lhs: Box::new(dest),
                            rhs: Box::new(expr::Expr::Num(8)),
                        }),
                        size
                    },
                    src: src2,
                });
            }
            Arm64Insn::ARM64_INS_LDP => {
                let src1 = op_to_non_mem(&ops[0]);
                let src2 = op_to_non_mem(&ops[1]);
                let dest = op_to_mem_addr(&ops[2]);
                let size = op_reg_size(&ops[0]);

                block.push(lir::Lir::Assign {
                    dst: src1,
                    src: expr::Expr::Deref {
                        ptr: Box::new(dest.clone()),
                        size
                    },
                });

                block.push(lir::Lir::Assign {
                    dst: src2,
                    src: expr::Expr::Deref {
                        ptr: Box::new(expr::Expr::Binary {
                            op: expr::BinaryOp::Add,
                            lhs: Box::new(dest),
                            rhs: Box::new(expr::Expr::Num(8)),
                        }),
                        size
                    },
                });
            }
            _ => todo!(
                "Unimplented instruction: {} {}",
                insn.mnemonic().unwrap(),
                insn.op_str().unwrap()
            ),
        }
    }

    Ok(block.block())
}
