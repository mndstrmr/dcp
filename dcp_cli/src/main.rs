use std::{fs::File, io::Read, collections::HashMap};

use clap::Parser;

#[derive(clap::Parser, Debug)]
struct Args {
    path: String
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

    let code = match dcp::macho::code_from(&buf) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Could not decode {}: {:?}", args.path, err);
            std::process::exit(1);
        }
    };

    let functions = match code {
        dcp::macho::CodeResult::UnknownBlock(unknown, addr) => vec![(None, unknown, addr)],
        dcp::macho::CodeResult::Functions(functions) => functions,
    };

    let mut function_ids = HashMap::new();
    for (i, (_, _, addr)) in functions.iter().enumerate() {
        function_ids.insert(*addr, dcp::expr::FuncId(i));
    }

    let abi = dcp::Abi {
        callee_saved: {
            let mut regs: Vec<_> = (19..=29).map(|x| dcp::armv8::X[x]).collect();
            regs.push(dcp::armv8::X[29]);
            regs.push(dcp::armv8::X[30]);
            regs.push(dcp::armv8::X[31]);
            regs
        },
        args: (0..=7).map(|x| dcp::armv8::X[x]).collect(),
        eliminate: vec![dcp::armv8::X[29]],
        base_reg: dcp::armv8::X[31]
    };

    let mut global_nodes: Vec<_> = functions.iter().map(|(_name, code, addr)| {
        let lir = dcp::armv8::to_lir(code, *addr, &function_ids).expect("Could not convert to LIR");

        let blir = dcp::lir_to_lirnodes(lir);
        let cfg = dcp::gen_local_cfg(&blir);

        dcp::GlobalCfgNode::new(cfg, blir)
    }).collect();

    dcp::func_args(&mut global_nodes, &abi);
    let (nodes, sigs): (Vec<_>, Vec<_>) = global_nodes.into_iter().map(dcp::GlobalCfgNode::split).unzip();

    for (cfg, mut blir) in nodes {
        dcp::insert_func_args(&sigs, &mut blir);
        for elim in &abi.eliminate {
            dcp::elim_name(&cfg, &mut blir, &abi, elim);
        }
        
        dcp::elim_dead_writes(&cfg, &mut blir, &abi);
        dcp::inline_single_use_names(&cfg, &mut blir, &abi);

        for block in &mut blir {
            dcp::lir::reduce_binops(&mut block.code);
        }

        let stack_frame = dcp::mem_to_name(&mut blir, &abi);

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

        dcp::mir::collapse_cmp(&mut mir.code);
        dcp::mir::reduce_binops(&mut mir.code);

        // dcp::stack_frame::name_locals(&mut mir, abi.base_reg);

        println!("{}", mir);
    }
}
