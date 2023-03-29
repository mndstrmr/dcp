#![feature(let_chains)]

mod ifs;
mod loops;
mod fors;
pub use fors::*;

pub fn mutate(block: &mut ir::Block) {
    ifs::insert_ifs(block);
    loops::insert_loops(block);
}
