use crate::{lir, expr, ssaify::Loc};

pub fn elim_ssa_loc(nodes: &mut Vec<lir::LirNode>, loc: Loc) {
    let lir::Lir::Assign { dst: expr::Expr::Name(name), src } = &nodes[loc.node].code[loc.stmt] else {
        panic!("Not an assignment");
    };

    let name = name.to_string();
    let replacement = src.clone();

    for node in nodes {
        for stmt in &mut node.code {
            stmt.replace_name(&name, &replacement);
        }
    }
}
