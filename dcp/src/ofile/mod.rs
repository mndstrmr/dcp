use std::collections::HashMap;

use crate::{dataflow::Abi, dataflow::GlobalCfgNode, expr, armv8, lir_to_lirnodes, gen_local_cfg, wasm};

pub mod macho;
pub mod wasmmod;

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

fn decode_arm64(functions: Vec<(Option<String>, &[u8], u64)>) -> Result<(Abi, Vec<GlobalCfgNode>), DecodeError> {
    let mut function_ids = HashMap::new();
    for (i, (_, _, addr)) in functions.iter().enumerate() {
        function_ids.insert(*addr, expr::FuncId(i));
    }

    Ok((
        armv8::abi(),
        functions.into_iter().map(|(name, code, addr)| {
            let lir = armv8::to_lir(code, addr, &function_ids).expect("Could not convert to LIR");

            let blir = lir_to_lirnodes(lir);
            let cfg = gen_local_cfg(&blir);

            GlobalCfgNode::new(cfg, blir, name)
        }).collect()
    ))
}

fn decode_macho(code: macho::CodeResult, arch: Option<macho::MachoArch>) -> Result<(Abi, Vec<GlobalCfgNode>), DecodeError> {
    let functions = match code {
        macho::CodeResult::UnknownBlock(unknown, addr) => vec![(None, unknown, addr)],
        macho::CodeResult::Functions(functions) => functions,
    };

    match arch {
        Some(macho::MachoArch::Arm64) => decode_arm64(functions),
        _ => Err(DecodeError::UnknownArch)
    }
}

fn decode_wasm(module: wasmmod::Module) -> Result<(Abi, Vec<GlobalCfgNode>), DecodeError> {
    Ok((
        wasm::abi(),
        module.functions().iter().enumerate().filter_map(|(i, func)| {
            if i > 12 {
                return None;
            }

            let lir = match wasm::to_lir(&func.body, module.types()) {
                Ok(lir) => lir,
                Err(err) => {
                    eprintln!("Could not translate wasm function: {err}");
                    return None
                }
            };

            let blir = lir_to_lirnodes(lir);
            let cfg = gen_local_cfg(&blir);

            Some(GlobalCfgNode::new(cfg, blir, func.name.clone()))
        }).collect()
    ))
}

pub fn load_lir_from_binary(buf: &[u8]) -> Result<(Abi, Vec<GlobalCfgNode>), DecodeError> {
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
