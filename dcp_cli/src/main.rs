use std::{fs::File, io::Read};

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
        dcp::macho::CodeResult::UnknownBlock(unknown) => vec![(None, unknown)],
        dcp::macho::CodeResult::Functions(functions) => functions,
    };

    let mut base = 0x1000000;
    for (_name, code) in functions {
        base += code.len() as u64;

        let lir = dcp::armv8::to_lir(code, base).expect("Could not convert to MIA");

        let mut blir = dcp::lir_to_lirnodes(lir);
        let cfg = dcp::gen_cfg(&blir);

        let abi = dcp::Abi {
            callee_saved: {
                let mut regs: Vec<_> = (19..=29).map(|x| dcp::armv8::X[x]).collect();
                regs.push(dcp::armv8::X[29]);
                regs.push(dcp::armv8::X[30]);
                regs.push(dcp::armv8::X[31]);
                regs
            }
        };

        dcp::elim_dead_writes(&cfg, &mut blir, &abi);
        dcp::inline_single_use_names(&cfg, &mut blir, &abi);

        let mut mir = dcp::reorder_code(&cfg, &cfg.dominators(), blir);
        
        dcp::mir::compress_control_flow(&mut mir.code);
        dcp::mir::cull_fallthrough_jumps(&mut mir.code);

        dcp::loop_detect::insert_loops(&mut mir.code);
        dcp::loop_detect::gotos_to_loop_continues(&mut mir.code);
        dcp::mir::trim_labels(&mut mir.code);
        dcp::loop_detect::step_back_breaks(&mut mir.code);
        dcp::loop_detect::final_continues(&mut mir.code);
        dcp::loop_detect::loops_to_whiles(&mut mir.code);
        dcp::loop_detect::whiles_to_fors(&mut mir.code);

        dcp::mir::collapse_cmp(&mut mir.code);
        println!("{}", mir);
    }
}
