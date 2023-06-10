use std::collections::HashMap;

use crate::{dataflow::Abi, expr, armv8, lir_to_lirnodes, gen_local_cfg, wasm, lir, cfg::ControlFlowGraph};

pub mod macho;
pub mod wasmmod;

pub struct FunctionDecl {
    pub name: Option<String>,
    pub args: Vec<&'static str>,
    pub funcid: expr::FuncId
}

pub struct FunctionDef {
    pub funcid: expr::FuncId,
    pub local_cfg: ControlFlowGraph,
    pub local_lirnodes: Vec<lir::LirNode>,
}

pub struct Module {
    pub abi: Abi,
    pub functions: Vec<FunctionDecl>,
}

pub struct FunctionDefSet(Vec<FunctionDef>);

impl FunctionDefSet {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn find(&self, funcid: expr::FuncId) -> Option<&FunctionDef> {
        for function in &self.0 {
            if function.funcid == funcid {
                return Some(function);
            }
        }
        None
    }

    pub fn find_mut(&mut self, funcid: expr::FuncId) -> Option<&mut FunctionDef> {
        for function in &mut self.0 {
            if function.funcid == funcid {
                return Some(function);
            }
        }
        None
    }

    pub fn into_iter(self) -> impl Iterator<Item=FunctionDef> {
        self.0.into_iter()
    }
}

impl Module {
    pub fn find_decl(&self, funcid: expr::FuncId) -> Option<&FunctionDecl> {
        for function in &self.functions {
            if function.funcid == funcid {
                return Some(function);
            }
        }
        None
    }

    pub fn find_decl_mut(&mut self, funcid: expr::FuncId) -> Option<&mut FunctionDecl> {
        for function in &mut self.functions {
            if function.funcid == funcid {
                return Some(function);
            }
        }
        None
    }
}

pub enum DecodeError {
    UnknownFormat,
    UnknownArch,
    NoCode,
    Invalid
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecodeError::UnknownFormat => write!(f, "unrecognised file format"),
            DecodeError::UnknownArch => write!(f, "unrecognised architecture"),
            DecodeError::NoCode => write!(f, "contains no code"),
            DecodeError::Invalid => write!(f, "corrupt file"),
        }
    }
}

fn decode_arm64(mut functions: Vec<(Option<String>, &[u8], u64)>) -> Result<(Module, FunctionDefSet), DecodeError> {
    let mut module = Module {
        abi: armv8::abi(),
        functions: vec![],
    };
    let mut defs = Vec::new();

    let mut function_ids = HashMap::new();
    for (i, (name, _, addr)) in functions.iter_mut().enumerate() {
        function_ids.insert(*addr, expr::FuncId(i));

        module.functions.push(FunctionDecl {
            args: vec![],
            funcid: expr::FuncId(i),
            name: name.take()
        });
    }

    for (i, (_, code, addr)) in functions.into_iter().enumerate() {
        let lir = armv8::to_lir(code, addr, &function_ids).expect("Could not convert to LIR");

        let lirnodes = lir_to_lirnodes(lir);
        defs.push(FunctionDef {
            funcid: expr::FuncId(i),
            local_cfg: gen_local_cfg(&lirnodes),
            local_lirnodes: lirnodes
        });
    }

    Ok((module, FunctionDefSet(defs)))
}

fn decode_macho(code: macho::CodeResult, arch: Option<macho::MachoArch>) -> Result<(Module, FunctionDefSet), DecodeError> {
    let functions = match code {
        macho::CodeResult::UnknownBlock(unknown, addr) => vec![(None, unknown, addr)],
        macho::CodeResult::Functions(functions) => functions,
    };

    match arch {
        Some(macho::MachoArch::Arm64) => decode_arm64(functions),
        _ => Err(DecodeError::UnknownArch)
    }
}

fn decode_wasm(wmodule: wasmmod::Module) -> Result<(Module, FunctionDefSet), DecodeError> {
    let mut module = Module {
        abi: wasm::abi(),
        functions: vec![],
    };
    let mut defs = Vec::new();

    for import in wmodule.imports() {
        module.functions.push(FunctionDecl {
            name: Some(import.name.clone()),
            args: vec![],
            funcid: expr::FuncId(import.idx)
        });
    }

    for func in wmodule.functions() {
        if func.idx >= 246 {
            continue;
        }

        let lir = match wasm::to_lir(&func.body, wmodule.types()) {
            Ok(lir) => lir,
            Err(err) => {
                eprintln!("Could not translate wasm function: {err}");
                return Err(DecodeError::Invalid)
            }
        };

        module.functions.push(FunctionDecl {
            name: func.name.clone(),
            args: vec![],
            funcid: expr::FuncId(func.idx)
        });

        let lirnodes = lir_to_lirnodes(lir);
        defs.push(FunctionDef {
            funcid: expr::FuncId(func.idx),
            local_cfg: gen_local_cfg(&lirnodes),
            local_lirnodes: lirnodes
        });
    }

    Ok((module, FunctionDefSet(defs)))
}

pub fn load_lir_from_binary(buf: &[u8]) -> Result<(Module, FunctionDefSet), DecodeError> {
    match macho::code_from(&buf) {
        Ok((code, arch)) => return decode_macho(code, arch),
        Err(macho::OfileErr::NoCode) => return Err(DecodeError::NoCode),
        Err(macho::OfileErr::UnknownFormat) => {}
        Err(macho::OfileErr::Invalid) => return Err(DecodeError::Invalid)
    }

    match wasmmod::module_from(&buf) {
        Ok(module) => return decode_wasm(module),
        Err(wasmmod::WasmDecodeError::InvalidFormat) => {}
        Err(wasmmod::WasmDecodeError::Invalid) => return Err(DecodeError::Invalid),
    }

    Err(DecodeError::UnknownFormat)
}
