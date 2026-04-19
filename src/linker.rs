use crate::ir::{Graph, NodeKind};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

pub struct Linker;

impl Linker {
    pub fn new() -> Self {
        Self
    }

    pub fn link(&self, graph: &mut Graph) {
        let mut unique_nodes: HashMap<String, Uuid> = HashMap::new();
        let mut final_nodes = Vec::new();
        let mut id_map = HashMap::new();

        for node in &graph.nodes {
            match node.kind {
                NodeKind::File => {
                    final_nodes.push(node.clone());
                }
                _ => {
                    if let Some(&existing_id) = unique_nodes.get(&node.name) {
                        id_map.insert(node.id, existing_id);
                    } else {
                        unique_nodes.insert(node.name.clone(), node.id);
                        final_nodes.push(node.clone());
                    }
                }
            }
        }
        graph.nodes = final_nodes;

        for edge in &mut graph.edges {
            if let Some(&new_to) = id_map.get(&edge.to_node_id) {
                edge.to_node_id = new_to;
            }
            if let Some(&new_from) = id_map.get(&edge.from_node_id) {
                edge.from_node_id = new_from;
            }
        }
        
        let mut unique_edges = HashSet::new();
        let mut final_edges = Vec::new();
        for edge in &graph.edges {
            let sig = (edge.from_node_id, edge.to_node_id, edge.relation_type.clone());
            if unique_edges.insert(sig) {
                final_edges.push(edge.clone());
            }
        }
        graph.edges = final_edges;
    }
}
