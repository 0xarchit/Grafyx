use crate::ir::{Graph, NodeKind};
use std::collections::{HashMap, HashSet};

pub struct Linker;

impl Linker {
    pub fn new() -> Self {
        Self
    }

    pub fn link(&self, graph: &mut Graph) {
        let mut unique_nodes: HashMap<String, String> = HashMap::new();
        let mut final_nodes = Vec::new();
        let mut id_map = HashMap::new();

        for node in &graph.nodes {
            match node.kind {
                NodeKind::File => {
                    final_nodes.push(node.clone());
                }
                _ => {
                    if let Some(existing_id) = unique_nodes.get(&node.name) {
                        id_map.insert(node.id.clone(), existing_id.clone());
                    } else {
                        unique_nodes.insert(node.name.clone(), node.id.clone());
                        final_nodes.push(node.clone());
                    }
                }
            }
        }
        graph.nodes = final_nodes;

        for edge in &mut graph.edges {
            if let Some(new_to) = id_map.get(&edge.to_node_id) {
                edge.to_node_id = new_to.clone();
            }
            if let Some(new_from) = id_map.get(&edge.from_node_id) {
                edge.from_node_id = new_from.clone();
            }
        }
        
        // Remove dead local edges mapping to itself if any, and deduplicate edges
        let mut unique_edges = HashSet::new();
        let mut final_edges = Vec::new();
        for edge in &graph.edges {
            if edge.from_node_id == edge.to_node_id { continue; }
            let sig = (edge.from_node_id.clone(), edge.to_node_id.clone(), edge.relation_type.clone());
            if unique_edges.insert(sig) {
                final_edges.push(edge.clone());
            }
        }
        graph.edges = final_edges;
    }
}
