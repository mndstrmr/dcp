#![feature(let_chains)]

const NEWLINE_INDENT: &str = "\n    ";

mod arch;
pub use arch::*;

mod ofile;
pub use ofile::*;

pub mod cfg;
mod ir;
pub use ir::*;

mod cfg_gen;
pub use cfg_gen::*;
mod dataflow;
pub use dataflow::*;
pub mod loop_detect;
mod order;
pub use order::*;
