use std::{fs::File, io::Read, collections::HashMap};

use clap::Parser;

#[derive(clap::Parser, Debug)]
struct Args {
    path: String
}

enum DecodeError {
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

fn decode_arm64(functions: Vec<(Option<String>, &[u8], u64)>) -> Result<(dcp::Abi, Vec<dcp::GlobalCfgNode>), DecodeError> {
    let mut function_ids = HashMap::new();
    for (i, (_, _, addr)) in functions.iter().enumerate() {
        function_ids.insert(*addr, dcp::expr::FuncId(i));
    }

    Ok((
        dcp::armv8::abi(),
        functions.iter().map(|(_name, code, addr)| {
            let lir = dcp::armv8::to_lir(code, *addr, &function_ids).expect("Could not convert to LIR");

            let blir = dcp::lir_to_lirnodes(lir);
            let cfg = dcp::gen_local_cfg(&blir);

            dcp::GlobalCfgNode::new(cfg, blir)
        }).collect()
    ))
}

fn decode_macho(code: dcp::macho::CodeResult, arch: Option<dcp::macho::MachoArch>) -> Result<(dcp::Abi, Vec<dcp::GlobalCfgNode>), DecodeError> {
    let functions = match code {
        dcp::macho::CodeResult::UnknownBlock(unknown, addr) => vec![(None, unknown, addr)],
        dcp::macho::CodeResult::Functions(functions) => functions,
    };

    match arch {
        Some(dcp::macho::MachoArch::Arm64) => decode_arm64(functions),
        _ => Err(DecodeError::UnknownArch)
    }
}

fn decode_wasm(module: dcp::wasmmod::Module) -> Result<(dcp::Abi, Vec<dcp::GlobalCfgNode>), DecodeError> {
    Ok((
        dcp::wasm::abi(),
        module.functions().iter().map(|func| {
            let lir = dcp::wasm::to_lir(func, module.types()).expect("Could not convert to LIR");
            println!("{}", lir);

            let blir = dcp::lir_to_lirnodes(lir);
            let cfg = dcp::gen_local_cfg(&blir);

            dcp::GlobalCfgNode::new(cfg, blir)
        }).collect()
    ))
}

fn decode(buf: &[u8]) -> Result<(dcp::Abi, Vec<dcp::GlobalCfgNode>), DecodeError> {
    match dcp::macho::code_from(&buf) {
        Ok((code, arch)) => return decode_macho(code, arch),
        Err(dcp::macho::OfileErr::NoCode) => return Err(DecodeError::NoCode),
        Err(dcp::macho::OfileErr::UnknownFormat) => {},
        Err(dcp::macho::OfileErr::Invalid) => return Err(DecodeError::Invalid)
    }

    match dcp::wasmmod::module_from(&buf) {
        Ok(module) => return decode_wasm(module),
        Err(dcp::wasmmod::WasmDecodeError::InvalidFormat) => {},
        Err(dcp::wasmmod::WasmDecodeError::Invalid) => return Err(DecodeError::Invalid),
    }

    Err(DecodeError::UnknownFormat)
}

fn main() {
    let args = Args::parse();
    
    let mut file = match File::open(&args.path) {
        Ok(file) => file,
        Err(err) => {
            eprintln!("Could not open {}: {}", args.path, err);
            std::process::exit(1);
        }
    };
    
    let mut buf = Vec::new();
    match file.read_to_end(&mut buf) {
        Ok(_) => {},
        Err(err) => {
            eprintln!("Could not read {}: {}", args.path, err);
            std::process::exit(1);
        }
    }

    // let abi = dcp::armv8::abi();
    let (abi, mut global_nodes) = match decode(&buf) {
        Ok(x) => x,
        Err(err) => {
            eprintln!("Could not decode {}: {}", args.path, err);
            std::process::exit(1);
        }
    };

    dcp::func_args(&mut global_nodes, &abi);
    let (nodes, sigs): (Vec<_>, Vec<_>) = global_nodes.into_iter().map(dcp::GlobalCfgNode::split).unzip();

    for (cfg, mut blir) in nodes {
        dcp::insert_func_args(&sigs, &mut blir);
        
        // FIXME: fp/sp should not be eliminated entirely, as they are needed upon return
        for eliminate in &abi.eliminate {
            for loc in dcp::ssaify::ssaify(&cfg, &mut blir, eliminate, &abi) {
                dcp::elim_ssa_loc(&mut blir, loc);
            }
        }
        
        dcp::elim_dead_writes(&cfg, &mut blir, &abi);
        dcp::inline_single_use_names(&cfg, &mut blir, &abi);

        for block in &mut blir {
            dcp::lir::reduce_binops(&mut block.code);
        }

        let stack_frame = dcp::mem_to_name(&mut blir, &abi);

        dcp::elim_dead_writes(&cfg, &mut blir, &abi);
        dcp::inline_single_use_names(&cfg, &mut blir, &abi);

        let code = dcp::reorder_code(&cfg, &cfg.dominators(), blir);
        let mut mir = dcp::mir::MirFunc::new(vec![], vec![], code, stack_frame);
        
        dcp::mir::compress_control_flow(&mut mir.code);
        dcp::mir::cull_fallthrough_jumps(&mut mir.code);

        dcp::mir::inline_terminating_if(&mut mir.code);
        dcp::loop_detect::insert_loops(&mut mir.code);
        dcp::loop_detect::gotos_to_loop_continues(&mut mir.code);
        dcp::loop_detect::gotos_to_loop_breaks(&mut mir.code);
        dcp::mir::trim_labels(&mut mir.code);
        dcp::mir::unreachable_control_flow(&mut mir.code);
        dcp::loop_detect::step_back_breaks(&mut mir.code);
        dcp::loop_detect::final_continues(&mut mir.code);
        dcp::loop_detect::loops_to_whiles(&mut mir.code);
        dcp::loop_detect::whiles_to_fors(&mut mir.code);
        dcp::mir::flip_negated_ifs(&mut mir.code);

        dcp::mir::collapse_cmp(&mut mir.code);
        dcp::mir::reduce_binops(&mut mir.code);

        println!("{}", mir);
    }
}
