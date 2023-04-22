#![feature(let_chains)]

const INDENT: &str = "    ";
const NEWLINE_INDENT: &str = "\n    ";

mod arch;
pub use arch::*;

mod ofile;
pub use ofile::*;

pub mod cfg;
mod ir;
pub use ir::*;

mod local_cfg;
pub use local_cfg::*;

mod dataflow;
pub use dataflow::*;
pub mod loop_detect;
