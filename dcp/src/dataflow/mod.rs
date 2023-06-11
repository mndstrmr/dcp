mod dead_writes;

pub use dead_writes::*;

mod single_use;
pub use single_use::*;

mod every_use;
pub use every_use::*;

mod func_args;
pub use func_args::*;

mod stack_frame;
pub use stack_frame::*;

mod ssaify;
pub use ssaify::*;

use crate::{cfg, lir};

pub struct Abi {
    pub callee_saved: Vec<&'static str>,
    pub global: Vec<&'static str>, // FIXME: Don't put this here
    pub args: Vec<&'static str>,
    pub eliminate: Vec<&'static str>,
    pub base_reg: Option<&'static str>,
}

enum ReadWrite {
    Reads,
    Writes,
    Neither
}

pub fn compress_cfg(cfg: &mut cfg::ControlFlowGraph, nodes: &mut Vec<lir::LirNode>) {
    // println!("{}", cfg.to_dot(|n| format!("\"{n}: {}\"", nodes[n].code.len())));

    let mut n = 0;
    while n < nodes.len() {
        let node = &nodes[n];
        if node.code.len() > 1 {
            n += 1;
            continue;
        }

        if node.code.len() == 1 && !matches!(node.code.last().unwrap(), lir::Lir::Branch { cond: None, .. }) {
            n += 1;
            continue;
        }

        let Some(target) = cfg.outgoing_for(n).iter().next().cloned() else {
            n += 1;
            continue;
        };

        for incoming in cfg.incoming_for(n).clone() {
            cfg.remove_edge(incoming, n);
            cfg.add_edge(incoming, target);

            match nodes[incoming].code.last_mut() {
                Some(lir::Lir::Branch { target: dst, .. }) => {
                    if dst.0 == n {
                        *dst = lir::Label(target);
                    }
                }
                _ => {}
            }
        }

        n += 1;
    }

    cfg.trim_unreachable();
    // println!("{}", cfg.to_dot(|n| format!("\"{n}: {}\"", nodes[n].code.len())));
}

pub fn inline_short_returns(cfg: &mut cfg::ControlFlowGraph, nodes: &mut Vec<lir::LirNode>) {
    const SHORT_SIZE: usize = 15;
    
    let mut n = 0;
    while n < nodes.len() {
        let node = &nodes[n];
        
        if let Some(lir::Lir::Return(_)) = node.code.last() && node.code.len() <= SHORT_SIZE {
            let node = node.clone();
            let incoming = cfg.incoming_for(n).clone();
            
            if incoming.len() <= 1 {
                n += 1;
                continue;
            }

            for incoming in incoming {
                nodes.push(node.clone());
                let new = nodes.len() - 1;
                cfg.add_node(new);
                cfg.remove_edge(incoming, n);
                cfg.add_edge(incoming, new);

                match nodes[incoming].code.last_mut() {
                    Some(lir::Lir::Branch { target, .. }) if target.0 == n => {
                        target.0 = new;
                    }
                    _ => nodes[incoming].code.push(lir::Lir::Branch { target: lir::Label(new), cond: None })
                }
            }

            cfg.remove_node(n);
        }

        n += 1;
    }
}
