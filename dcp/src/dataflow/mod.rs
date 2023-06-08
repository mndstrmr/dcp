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

pub mod ssaify;

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
