#![feature(let_chains)]

use std::{fs::File, io::Read};

use clap::Parser;
use dcp::pretty;

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

    // Load lir (direct translation from binary)
    let (mut module, mut defs) = match dcp::load_lir_from_binary(&buf) {
        Ok(x) => x,
        Err(err) => {
            eprintln!("Could not decode {}: {}", args.path, err);
            std::process::exit(1);
        }
    };

    let mut mir_func_defs = Vec::new();

    // Add signatures to functions
    dcp::dataflow::func_args(&mut module, &defs);
    dcp::dataflow::insert_func_args(&module, &mut defs);

    for mut function in defs.into_iter() {
        let Some(name) = &module.find_decl(function.funcid).unwrap().name else {
            continue;
        };
        if name != "fflush" {
            continue;
        }

        dcp::dataflow::compress_cfg(&mut function.local_cfg, &mut function.local_lirnodes);
        dcp::dataflow::inline_short_returns(&mut function.local_cfg, &mut function.local_lirnodes);
        
        // Eliminate frame pointers
        // FIXME: fp/sp should not be eliminated entirely, as they are needed upon return
        for eliminate in &module.abi.eliminate {
            for loc in dcp::dataflow::ssaify(&function.local_cfg, &mut function.local_lirnodes, eliminate, &module.abi) {
                dcp::dataflow::elim_ssa_loc(&mut function.local_lirnodes, loc);
            }
        }
        
        // Clean up code
        // FIXME: This should all probably be in a loop of some sort
        dcp::dataflow::elim_dead_writes(&function.local_cfg, &mut function.local_lirnodes, &module.abi);
        dcp::dataflow::inline_single_use_names(&function.local_cfg, &mut function.local_lirnodes, &module.abi);
        dcp::opt::reduce_binops_lir(&mut function.local_lirnodes);

        // Mem to reg, then cleanup again
        let stack_frame = dcp::dataflow::mem_to_name(&mut function.local_lirnodes, &module.abi);
        dcp::dataflow::elim_dead_writes(&function.local_cfg, &mut function.local_lirnodes, &module.abi);
        dcp::dataflow::inline_single_use_names(&function.local_cfg, &mut function.local_lirnodes, &module.abi);

        // Place code down, and get MIR
        let code = dcp::reorder_code(&function.local_cfg, &function.local_cfg.dominators(), function.local_lirnodes);
        let mut mir = dcp::mir::MirFunc::new(function.funcid, vec![], code, stack_frame);
        
        // Remove redundant jumps (FIXME: Are both really necessary?)
        dcp::opt::compress_control_flow(&mut mir);
        dcp::opt::cull_fallthrough_jumps(&mut mir);

        // Improves if/if-else/loop/while/for
        // FIXME: Repeat for as long as necessary, not a fixed number of times
        for _ in 0..5 {
            dcp::opt::insert_loops(&mut mir);
            dcp::opt::gotos_to_loop_continues(&mut mir);
            dcp::opt::gotos_to_loop_breaks(&mut mir);
            dcp::opt::trim_labels(&mut mir);
            dcp::opt::elim_unreachable(&mut mir);
            dcp::opt::step_back_breaks(&mut mir);
            dcp::opt::final_continues(&mut mir);
            dcp::opt::inline_terminating_if(&mut mir);
            dcp::opt::inf_loops_unreachable(&mut mir);
            dcp::opt::loop_start_label_swap(&mut mir);
            dcp::opt::gotos_to_loop_breaks(&mut mir);
            dcp::opt::terminating_to_break(&mut mir);
            dcp::opt::trim_labels(&mut mir);
            dcp::opt::loops_to_whiles(&mut mir);
            dcp::opt::whiles_to_fors(&mut mir);
            dcp::opt::flip_negated_ifs(&mut mir);
            dcp::opt::compress_if_chains(&mut mir);
        }

        // Final prettification
        dcp::opt::collapse_cmp(&mut mir);
        dcp::opt::reduce_binops(&mut mir);

        mir_func_defs.push(mir);
    }

    for def in mir_func_defs {
        let printer = pretty::PrettyPrinter::new(&def, &module);
        println!("{}", printer);
    }
}
