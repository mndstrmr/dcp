use std::rc::Rc;

pub struct IrNode {
    pub id: cfg::NodeId,
    pub label: Rc<ir::Label>,
    pub code: Vec<ir::Stmt>,
}

impl std::fmt::Debug for IrNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "IrNode #{} {{", self.id)?;
        for stmt in &self.code {
            if !stmt.invisible() {
                f.write_str(&format!("\n{}", stmt).replace('\n', "\n\t"))?;
            }
        }
        write!(f, "\n}}")
    }
}

pub fn irnode_by_cfg_node(id: usize, nodes: &[IrNode]) -> usize {
    nodes.binary_search_by_key(&id, |x| x.id).expect("Could not find irnode")
}

pub fn func_to_ir_nodes(mut func: ir::Func, graph: &mut cfg::ControlFlowGraph) -> Vec<IrNode> {
    let mut nodes = vec![];

    let mut curr_id = graph.get_entry().expect("No entry");
    let mut curr_label = Rc::new(ir::Label("#entry".to_string()));

    let mut relocs = Vec::new();

    let code = func.block_mut().get_mut();

    let mut i = 0;
    while i < code.len() {
        let stmt = &code[i];
        match stmt {
            ir::Stmt::Label(label) => {
                let new_label =
                    match label.upgrade() {
                        Some(label) => label,
                        None => {
                            i += 1;
                            continue
                        }
                    };

                let mut code = code.drain(..i).collect::<Vec<_>>();
                code.push(ir::Stmt::Branch {
                    cond: None,
                    target: new_label.clone()
                });

                nodes.push(IrNode {
                    id: curr_id,
                    label: curr_label,
                    code,
                });

                let new_node = graph.add_node();
                graph.add_edge(curr_id, new_node);

                curr_id = new_node;
                curr_label = new_label;
                i = 1;
            }
            ir::Stmt::Branch { cond, target } => {
                let new_node = graph.add_node();
                let new_label = Rc::new(ir::Label(format!("#{}", new_node)));

                relocs.push((curr_id, target.clone()));

                let has_cond = cond.is_some();
                let mut code = code.drain(..=i).collect::<Vec<_>>();
                if has_cond {
                    graph.add_edge(curr_id, new_node);

                    code.push(ir::Stmt::Branch {
                        cond: None,
                        target: new_label.clone()
                    });
                }

                nodes.push(IrNode {
                    id: curr_id,
                    label: curr_label,
                    code,
                });

                curr_id = new_node;
                curr_label = new_label;
                i = 0;
            }
            _ => {
                i += 1;
            }
        }
    }

    nodes.push(IrNode {
        id: curr_id,
        label: curr_label,
        code: code.drain(..).collect(),
    });

    for node in &mut nodes {
        // TODO: Use a better flag, and find a constant time solution
        if node.label.0.starts_with('#') {
            node.code.insert(0, ir::Stmt::Label(Rc::downgrade(&node.label)));
        }
    }

    for (src, dest_label) in relocs {
        for node in &mut nodes {
            if Rc::ptr_eq(&node.label, &dest_label) {
                graph.add_edge(src, node.id);
                break;
            }
        }
    }

    nodes
}
