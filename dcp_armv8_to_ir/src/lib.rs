use std::{collections::HashMap, rc::Rc};

use capstone::{prelude::*, arch::{ArchOperand, arm64::{Arm64Insn, Arm64Reg, Arm64CC, Arm64OperandType, Arm64Operand}}};

pub const ABI: abi::Abi = abi::Abi {
    callee_saved: &["lr", "sp"],
    args: &["w0", "w1", "w2", "w3", "w4", "w5", "w6", "w7"]
};

fn cc_to_ir(cc: Arm64CC) -> Option<ir::UnaryOp> {
    match cc {
        Arm64CC::ARM64_CC_INVALID => None,
        Arm64CC::ARM64_CC_EQ => Some(ir::UnaryOp::CmpEq),
        Arm64CC::ARM64_CC_NE => Some(ir::UnaryOp::CmpNe),
        Arm64CC::ARM64_CC_LT => Some(ir::UnaryOp::CmpLt),
        Arm64CC::ARM64_CC_GT => Some(ir::UnaryOp::CmpGt),
        Arm64CC::ARM64_CC_LE => Some(ir::UnaryOp::CmpLe),
        Arm64CC::ARM64_CC_GE => Some(ir::UnaryOp::CmpGe),
        
        Arm64CC::ARM64_CC_LO => Some(ir::UnaryOp::CmpLt),
        Arm64CC::ARM64_CC_HI => Some(ir::UnaryOp::CmpGt),

        _ => todo!()
    }
}

fn op_to_expr(cs: &Capstone, op: &ArchOperand) -> ir::Expr {
    let op = match op {
        ArchOperand::Arm64Operand(op) => op,
        _ => panic!("not arm64?")
    };

    match op.op_type {
        Arm64OperandType::Reg(reg) =>
            if reg.0 == Arm64Reg::ARM64_REG_WZR as u16 || reg.0 == Arm64Reg::ARM64_REG_XZR as u16 {
                ir::Expr::Num(0)
            } else {
                ir::Expr::Name(cs.reg_name(reg).unwrap())
            }
        Arm64OperandType::Imm(imm) => ir::Expr::Num(imm),
        Arm64OperandType::Mem(mem) => {
            assert_eq!(mem.index().0, 0);

            let mut expr = None;
            if mem.base().0 != 0 {
                expr = Some(ir::Expr::Name(cs.reg_name(mem.base()).unwrap()));
            }

            if mem.disp() != 0 {
                if let Some(expr_) = expr {
                    expr = Some(ir::Expr::Binary {
                        op: ir::BinaryOp::Add,
                        lhs: Box::new(expr_),
                        rhs: Box::new(ir::Expr::Num(mem.disp() as i64))
                    });
                } else {
                    expr = Some(ir::Expr::Num(mem.disp() as i64));
                }
            }

            match expr {
                None => ir::Expr::Deref(Box::new(ir::Expr::Num(0))),
                Some(expr) => ir::Expr::Deref(Box::new(expr)),
            }
        }
        _ => todo!("Operand: {:?}", op)
    }
}

fn op_to_mem_addr(cs: &Capstone, op: &ArchOperand) -> ir::Expr {
    let op = match op {
        ArchOperand::Arm64Operand(op) => op,
        _ => panic!("not arm64?")
    };

    match op.op_type {
        Arm64OperandType::Mem(mem) => {
            assert_eq!(mem.index().0, 0);

            let mut expr = None;
            if mem.base().0 != 0 {
                expr = Some(ir::Expr::Name(cs.reg_name(mem.base()).unwrap()));
            }

            if mem.disp() != 0 {
                if let Some(expr_) = expr {
                    expr = Some(ir::Expr::Binary {
                        op: ir::BinaryOp::Add,
                        lhs: Box::new(expr_),
                        rhs: Box::new(ir::Expr::Num(mem.disp() as i64))
                    });
                } else {
                    expr = Some(ir::Expr::Num(mem.disp() as i64));
                }
            }

            match expr {
                None => ir::Expr::Num(0),
                Some(expr) => expr,
            }
        }
        _ => todo!("Not a memory operand: {:?}", op)
    }
}

pub fn to_function(data: &[u8]) -> Result<ir::Func, String> {
    let cs = Capstone::new().arm64()
        .mode(arch::arm64::ArchMode::Arm)
        .detail(true)
        .build().expect("Could not build cs object");
    
    let insns = cs.disasm_all(data, 0).expect("Could not disassemble");

    let mut func = ir::Func::new();

    let tmp_label = Rc::new(ir::Label("<tmp>".to_string()));
    let mut labels = HashMap::new();
    let mut label_relocs = Vec::new();

    for insn in insns.as_ref() {
        let detail = cs.insn_detail(insn).expect("Could not object cs instruction detail");
        let arch_detail = detail.arch_detail();
        let ops = arch_detail.operands();
        let arch_detail = arch_detail.arm64().unwrap();

        let label = Rc::new(ir::Label(format!("@0x{:x}", insn.address())));
        labels.insert(insn.address(), label.clone());
        func.add(ir::Stmt::Label(Rc::downgrade(&label)));

        match Arm64Insn::from(insn.id().0) {
            Arm64Insn::ARM64_INS_SUB => {
                let dest = op_to_expr(&cs, &ops[0]);
                let src1 = op_to_expr(&cs, &ops[1]);
                let src2 = op_to_expr(&cs, &ops[2]);
                func.add(ir::Stmt::Assign {
                    lhs: dest,
                    rhs: ir::Expr::Binary {
                        op: ir::BinaryOp::Sub,
                        lhs: Box::new(src1),
                        rhs: Box::new(src2),
                    }
                });
            }
            Arm64Insn::ARM64_INS_ADD => {
                let dest = op_to_expr(&cs, &ops[0]);
                let src1 = op_to_expr(&cs, &ops[1]);
                let src2 = op_to_expr(&cs, &ops[2]);
                func.add(ir::Stmt::Assign {
                    lhs: dest,
                    rhs: ir::Expr::Binary {
                        op: ir::BinaryOp::Add,
                        lhs: Box::new(src1),
                        rhs: Box::new(src2),
                    }
                });
            }
            Arm64Insn::ARM64_INS_MOV | Arm64Insn::ARM64_INS_LDR | Arm64Insn::ARM64_INS_LDUR => {
                let dest = op_to_expr(&cs, &ops[0]);
                let src = op_to_expr(&cs, &ops[1]);
                func.add(ir::Stmt::Assign {
                    lhs: dest,
                    rhs: src
                });
            }
            Arm64Insn::ARM64_INS_STR | Arm64Insn::ARM64_INS_STUR => {
                let dest = op_to_expr(&cs, &ops[1]);
                let src = op_to_expr(&cs, &ops[0]);
                func.add(ir::Stmt::Assign {
                    lhs: dest,
                    rhs: src
                });
            }
            Arm64Insn::ARM64_INS_CMP => {
                let src1 = op_to_expr(&cs, &ops[0]);
                let src2 = op_to_expr(&cs, &ops[1]);
                
                func.add(ir::Stmt::Assign {
                    lhs: ir::Expr::Name("cmp".to_string()),
                    rhs: ir::Expr::Binary {
                        op: ir::BinaryOp::Cmp,
                        lhs: Box::new(src1.clone()),
                        rhs: Box::new(src2.clone()),
                    }
                });
            }
            Arm64Insn::ARM64_INS_SUBS => {
                let dest = op_to_expr(&cs, &ops[0]);
                let src1 = op_to_expr(&cs, &ops[1]);
                let src2 = op_to_expr(&cs, &ops[2]);

                func.add(ir::Stmt::Assign {
                    lhs: ir::Expr::Name("cmp".to_string()),
                    rhs: ir::Expr::Binary {
                        op: ir::BinaryOp::Cmp,
                        lhs: Box::new(src1.clone()),
                        rhs: Box::new(src2.clone()),
                    }
                });

                func.add(ir::Stmt::Assign {
                    lhs: dest,
                    rhs: ir::Expr::Binary {
                        op: ir::BinaryOp::Sub,
                        lhs: Box::new(src1.clone()),
                        rhs: Box::new(src2.clone()),
                    }
                });
            }
            Arm64Insn::ARM64_INS_RET => {
                func.add(ir::Stmt::Return(ir::Expr::Name("w0".to_string())));
            }
            Arm64Insn::ARM64_INS_CSEL => {
                let cond = match cc_to_ir(arch_detail.cc()) {
                    None => None,
                    Some(op) => Some(ir::Expr::Unary {
                        op,
                        expr: Box::new(ir::Expr::Name("cmp".to_string())),
                    })
                };

                let dest = op_to_expr(&cs, &ops[0]);
                let src1 = op_to_expr(&cs, &ops[1]);
                let src2 = op_to_expr(&cs, &ops[2]);

                let label1 = Rc::new(ir::Label(format!("@0x{:x}@a", insn.address())));
                let label2 = Rc::new(ir::Label(format!("@0x{:x}@b", insn.address())));
                
                func.add(ir::Stmt::Branch {
                    cond,
                    target: label1.clone()
                });
                func.add(ir::Stmt::Assign {
                    lhs: dest.clone(),
                    rhs: src2
                });
                func.add(ir::Stmt::Branch {
                    cond: None,
                    target: label2.clone()
                });
                func.add(ir::Stmt::Label(Rc::downgrade(&label1)));
                func.add(ir::Stmt::Assign {
                    lhs: dest,
                    rhs: src1
                });
                func.add(ir::Stmt::Label(Rc::downgrade(&label2)));
            }
            Arm64Insn::ARM64_INS_BL => {
                let addr = match &ops[0] {
                    ArchOperand::Arm64Operand(Arm64Operand { op_type: Arm64OperandType::Imm(val), .. }) => {
                        *val as u64
                    }
                    _ => panic!("Branch operand type")
                };

                func.add(ir::Stmt::Assign {
                    lhs: ir::Expr::Name("w0".to_string()),
                    rhs: ir::Expr::Call {
                        func: Box::new(ir::Expr::Num(addr as i64)),
                        args: vec![]
                    }
                });
            }
            Arm64Insn::ARM64_INS_B => {
                let cond = match cc_to_ir(arch_detail.cc()) {
                    None => None,
                    Some(op) => Some(ir::Expr::Unary {
                        op,
                        expr: Box::new(ir::Expr::Name("cmp".to_string())),
                    })
                };

                match &ops[0] {
                    ArchOperand::Arm64Operand(Arm64Operand { op_type: Arm64OperandType::Imm(val), .. }) => {
                        label_relocs.push((func.block().len(), *val as u64));
                    }
                    _ => panic!("Branch operand type")
                }
                
                func.add(ir::Stmt::Branch {
                    cond,
                    target: tmp_label.clone()
                });
            }
            Arm64Insn::ARM64_INS_STP => {
                let src1 = op_to_expr(&cs, &ops[0]);
                let src2 = op_to_expr(&cs, &ops[1]);
                let dest = op_to_mem_addr(&cs, &ops[2]);

                func.add(ir::Stmt::Assign {
                    lhs: ir::Expr::Deref(Box::new(dest.clone())),
                    rhs: src1
                });

                func.add(ir::Stmt::Assign {
                    lhs: ir::Expr::Deref(Box::new(ir::Expr::Binary {
                        op: ir::BinaryOp::Add,
                        lhs: Box::new(dest),
                        rhs: Box::new(ir::Expr::Num(8)) // FIXME: Determine real size
                    })),
                    rhs: src2
                });
            }
            Arm64Insn::ARM64_INS_LDP => {
                let src1 = op_to_expr(&cs, &ops[0]);
                let src2 = op_to_expr(&cs, &ops[1]);
                let dest = op_to_mem_addr(&cs, &ops[2]);

                func.add(ir::Stmt::Assign {
                    lhs: src1,
                    rhs: ir::Expr::Deref(Box::new(dest.clone()))
                });

                func.add(ir::Stmt::Assign {
                    lhs: src2,
                    rhs: ir::Expr::Deref(Box::new(ir::Expr::Binary {
                        op: ir::BinaryOp::Add,
                        lhs: Box::new(dest),
                        rhs: Box::new(ir::Expr::Num(8)) // FIXME: Determine real size
                    }))
                });
            }
            _ => todo!("Unimplented instruction: {} {}", insn.mnemonic().unwrap(), insn.op_str().unwrap())
        }
    }

    for (reloc_idx, reloc_target) in label_relocs {
        match func.block_mut().at_mut(reloc_idx) {
            ir::Stmt::Branch { target, .. } => {
                *target = labels.get(&reloc_target).expect("Label not found").clone();
            }
            _ => panic!("Not a branch")
        }
    }

    assert_eq!(Rc::strong_count(&tmp_label), 1);

    Ok(func)
}
