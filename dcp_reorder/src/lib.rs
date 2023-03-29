// pub fn reduce_consecutive(&mut self) -> HashMap<NodeId, Vec<NodeId>> {
//     let mut sequences: HashMap<NodeId, Vec<NodeId>> = HashMap::new();

//     let mut to_visit = vec![self.entry.expect("No entry node")];
//     let mut visited = HashSet::new();

//     while let Some(node) = to_visit.pop() {
//         if visited.contains(&node) {
//             continue;
//         }

//         visited.insert(node);

//         if self.outgoing_for(node).len() == 1 {
//             let mut sequence = Vec::new();

//             while self.outgoing_for(node).len() == 1 {
//                 let next_node = *self.outgoing_for(node).iter().next().unwrap();
//                 if self.incoming_for(next_node).len() != 1 {
//                     break;
//                 }

//                 sequence.push(next_node);

//                 let dropped = self.nodes.remove(&next_node).unwrap();
//                 let node_mut = self.nodes.get_mut(&node).unwrap();
//                 node_mut.outgoing = dropped.outgoing;
//             }

//             if let Some(last) = sequence.last() {
//                 for source in self.outgoing_for(node).clone() {
//                     let incoming = &mut self.nodes.get_mut(&source).unwrap().incoming;
//                     incoming.remove(last);
//                     incoming.insert(node);
//                 }
//             }

//             sequences.insert(node, sequence);
//         }

//         to_visit.extend(self.outgoing_for(node));
//     }

//     sequences
// }

use std::collections::HashSet;

fn discover_nodes(subgraph: &HashSet<cfg::NodeId>, entry: cfg::NodeId, cfg: &cfg::ControlFlowGraph, dominators: &cfg::Dominators) -> HashSet<cfg::NodeId> {
    let mut to_visit = vec![entry];
    let mut visited = HashSet::new();
    visited.insert(entry);

    while let Some(node) = to_visit.pop() {
        for neighbour in cfg.outgoing_for(node) {
            if !dominators.implies_backwards_edge(node, *neighbour) && subgraph.contains(neighbour) && visited.insert(*neighbour) {
                to_visit.push(*neighbour);
            }
        }
    }

    visited
}

fn append_subgraph_to_block(subgraph: HashSet<cfg::NodeId>, entry: cfg::NodeId, cfg: &cfg::ControlFlowGraph, dominators: &cfg::Dominators, nodes: &mut Vec<ir_to_cfg::IrNode>, block: &mut ir::Block) {
    if !subgraph.contains(&entry) {
        return;
    }

    let irnode = ir_to_cfg::irnode_by_cfg_node(entry, nodes);
    block.get_mut().extend(nodes[irnode].code.drain(..));

    let mut node = entry;
    let outgoing = loop {
        let outgoing = cfg.outgoing_for(node).iter()
            .filter(|x| subgraph.contains(x) && !dominators.implies_backwards_edge(node, **x))
            .collect::<Vec<_>>();
        
        if outgoing.len() == 0 {
            return;
        }

        if outgoing.len() == 2 {
            break outgoing;
        }

        assert!(outgoing.len() == 1);

        let irnode = ir_to_cfg::irnode_by_cfg_node(*outgoing[0], nodes);
        block.get_mut().extend(nodes[irnode].code.drain(..));   

        node = *outgoing[0];
    };

    assert_eq!(outgoing.len(), 2);

    let a = *outgoing[0];
    let b = *outgoing[1];

    let red = discover_nodes(&subgraph, a, cfg, dominators);
    let blue = discover_nodes(&subgraph, b, cfg, dominators);

    let purple = red.intersection(&blue).copied().collect();

    append_subgraph_to_block(red.difference(&purple).copied().collect(), a, cfg, dominators, nodes, block);
    append_subgraph_to_block(blue.difference(&purple).copied().collect(), b, cfg, dominators, nodes, block);

    let purple_starts = purple.iter()
                            .filter(|x| cfg.incoming_for(**x).intersection(&purple).next().is_none())
                            .copied().collect::<Vec<_>>();

    // println!("red = {:?}", red);
    // println!("blue = {:?}", blue);
    // println!("purple = {:?}", purple);
    // println!("purple map = {:?}", purple.iter().map(|x| (x, cfg.incoming_for(*x).intersection(&purple))).collect::<Vec<_>>());
    // println!("{} {:?} {:?}", node, purple_starts, purple);

    if !purple_starts.is_empty() {
        assert!(purple_starts.len() == 1);
        append_subgraph_to_block(purple, purple_starts[0], cfg, dominators, nodes, block);
    }
}

pub fn reorder_code(graph: &cfg::ControlFlowGraph, dominators: &cfg::Dominators, mut nodes: Vec<ir_to_cfg::IrNode>) -> ir::Func {
    let mut new_func = ir::Func::new();
    append_subgraph_to_block(graph.nodes(), graph.get_entry().expect("No entry"), &graph, &dominators, &mut nodes, new_func.block_mut());
    new_func
}
