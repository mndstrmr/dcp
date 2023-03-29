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

    let code = match dcp_ofile::code_from(&buf) {
        Ok(code) => code,
        Err(err) => {
            eprintln!("Could not decode {}: {:?}", args.path, err);
            std::process::exit(1);
        }
    };

    let functions = match code {
        dcp_ofile::CodeResult::UnknownBlock(unknown) => vec![(None, unknown)],
        dcp_ofile::CodeResult::Functions(functions) => functions,
    };

    for (name, code) in functions {
        let ir = dcp_func(code);
        if let Some(name) = name {
            println!("{name} {}", ir.to_string());
        } else {
            println!("{}", ir.to_string());
        }
    }
}

fn dcp_func(code: &[u8]) -> dcp_ir::Func {
    let mut ir = match dcp_armv8_to_ir::to_function(code) {
        Ok(ir) => ir,
        Err(err) => {
            eprintln!("Could not convert to function: {}", err);
            std::process::exit(1);
        }
    };

    dcp_tidy::elim_nop_assignments(ir.block_mut());

    let mut graph = dcp_cfg::ControlFlowGraph::new();
    let entry = graph.add_node();
    graph.set_entry(entry);
    let mut nodes = dcp_ir_to_cfg::func_to_ir_nodes(ir, &mut graph);
    graph.trim_unreachable();
    let dominators = graph.dominators();

    // println!("{}", graph.to_dot());

    dcp_dataflow::elim_dead_writes(&graph, &mut nodes, &dcp_armv8_to_ir::ABI);
    dcp_dataflow::inline_single_use_names(&graph, &mut nodes, &dcp_armv8_to_ir::ABI);

    let mut ir = dcp_reorder::reorder_code(&graph, &dominators, nodes);
    
    dcp_tidy::elim_consecutive_jump_labels(ir.block_mut());
    dcp_tidy::elim_consecutive_control_flow(ir.block_mut());

    dcp_cf_pat::mutate(ir.block_mut());
    dcp_tidy::elim_consecutive_control_flow(ir.block_mut());
    dcp_tidy::loop_break_early(ir.block_mut());
    dcp_tidy::elim_loop_final_continue(ir.block_mut());
    dcp_cf_pat::loops_to_fors(ir.block_mut());
    dcp_cf_pat::gotos_to_for_continue(ir.block_mut());
    dcp_tidy::collapse_cmp(ir.block_mut());

    ir
}
