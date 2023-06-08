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

    // Load lir (direct translation from binary)
    let (abi, mut global_nodes) = match dcp::load_lir_from_binary(&buf) {
        Ok(x) => x,
        Err(err) => {
            eprintln!("Could not decode {}: {}", args.path, err);
            std::process::exit(1);
        }
    };

    // Add signatures to functions
    dcp::dataflow::func_args(&mut global_nodes, &abi);
    let (nodes, sigs): (Vec<_>, Vec<_>) = global_nodes.into_iter().map(dcp::dataflow::GlobalCfgNode::split).unzip();

    for (cfg, mut blir) in nodes {
        // Add registers to function calls if necessary
        dcp::dataflow::insert_func_args(&sigs, &mut blir);
        
        // Eliminate frame pointers
        // FIXME: fp/sp should not be eliminated entirely, as they are needed upon return
        for eliminate in &abi.eliminate {
            for loc in dcp::dataflow::ssaify(&cfg, &mut blir, eliminate, &abi) {
                dcp::dataflow::elim_ssa_loc(&mut blir, loc);
            }
        }
        
        // Clean up code
        // FIXME: This should all probably be in a loop of some sort
        dcp::dataflow::elim_dead_writes(&cfg, &mut blir, &abi);
        dcp::dataflow::inline_single_use_names(&cfg, &mut blir, &abi);
        dcp::opt::reduce_binops_lir(&mut blir);

        // Mem to reg, then cleanup again
        let stack_frame = dcp::dataflow::mem_to_name(&mut blir, &abi);
        dcp::dataflow::elim_dead_writes(&cfg, &mut blir, &abi);
        dcp::dataflow::inline_single_use_names(&cfg, &mut blir, &abi);

        // Place code down, and get MIR
        let code = dcp::reorder_code(&cfg, &cfg.dominators(), blir);
        let mut mir = dcp::mir::MirFunc::new(vec![], vec![], code, stack_frame);
        
        // Remove redundant jumps (FIXME: Are both really necessary?)
        dcp::opt::compress_control_flow(&mut mir);
        dcp::opt::cull_fallthrough_jumps(&mut mir);

        // Improves if/if-else/loop/while/for
        dcp::opt::inline_terminating_if(&mut mir);
        dcp::opt::insert_loops(&mut mir);
        dcp::opt::gotos_to_loop_continues(&mut mir);
        dcp::opt::gotos_to_loop_breaks(&mut mir);
        dcp::opt::trim_labels(&mut mir);
        dcp::opt::elim_unreachable(&mut mir);
        dcp::opt::step_back_breaks(&mut mir);
        dcp::opt::final_continues(&mut mir);
        dcp::opt::loops_to_whiles(&mut mir);
        dcp::opt::whiles_to_fors(&mut mir);
        dcp::opt::flip_negated_ifs(&mut mir);

        // Final prettification
        dcp::opt::collapse_cmp(&mut mir);
        dcp::opt::reduce_binops(&mut mir);

        println!("{}", mir);
    }
}
