use crate::ir::Graph;
use std::collections::{HashMap, HashSet};

pub struct Linker {
    dirs: Vec<String>,
}

impl Linker {
    pub fn new(dirs: Vec<String>) -> Self {
        Self { dirs }
    }

    pub fn link(&self, graph: &mut Graph) {
        // 1. Identify ROOT and Service Nodes
        let root_id = "GLOBAL::ROOT".to_string();
        graph.nodes.push(crate::ir::Node {
            id: root_id.clone(),
            kind: crate::ir::NodeKind::Root,
            name: "ROOT".to_string(),
            language: "global".to_string(),
            file_path: "".to_string(),
            service: "global".to_string(),
            start_line: 0,
            end_line: 0,
        });

        let mut service_map = HashMap::new();
        for dir in &self.dirs {
            let service_name = std::path::Path::new(dir)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(dir);
            let service_id = format!("SERVICE::{}", service_name);
            
            graph.nodes.push(crate::ir::Node {
                id: service_id.clone(),
                kind: crate::ir::NodeKind::Service,
                name: service_name.to_string(),
                language: "service".to_string(),
                file_path: dir.clone(),
                service: service_name.to_string(),
                start_line: 0,
                end_line: 0,
            });
            
            graph.edges.push(crate::ir::Edge {
                from_node_id: root_id.clone(),
                to_node_id: service_id.clone(),
                relation_type: crate::ir::RelationType::RootLink,
            });
            
            service_map.insert(dir.clone(), (service_id, service_name.to_string()));
        }

        // 2. Assign files to services and deduplicate
        let mut unique_nodes: HashMap<String, String> = HashMap::new();
        let mut final_nodes = Vec::new();
        let mut id_map = HashMap::new();
        let mut file_index = HashMap::new();

        // Pass-through ROOT and SERVICE nodes already in graph
        for node in &graph.nodes {
            if node.kind == crate::ir::NodeKind::Root || node.kind == crate::ir::NodeKind::Service {
                final_nodes.push(node.clone());
                continue;
            }
        }

        for node in &graph.nodes {
            if node.kind == crate::ir::NodeKind::File {
                let path = std::path::Path::new(&node.file_path);
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    file_index.insert(stem.to_string(), node.id.clone());
                }
            }
        }

        for mut node in graph.nodes.clone() {
            if node.kind == crate::ir::NodeKind::Root || node.kind == crate::ir::NodeKind::Service {
                continue;
            }
            
            // Assign service
            for (dir, (svc_id, svc_name)) in &service_map {
                if node.file_path.contains(dir) {
                    node.service = svc_name.clone();
                    if node.kind == crate::ir::NodeKind::File {
                         graph.edges.push(crate::ir::Edge {
                            from_node_id: svc_id.clone(),
                            to_node_id: node.id.clone(),
                            relation_type: crate::ir::RelationType::RootLink, // subtle link
                        });
                    }
                    break;
                }
            }

            match node.kind {
                crate::ir::NodeKind::File => {
                    final_nodes.push(node.clone());
                }
                crate::ir::NodeKind::Module => {
                    if let Some(file_id) = file_index.get(&node.name) {
                        id_map.insert(node.id.clone(), file_id.clone());
                    } else {
                        if let Some(existing_id) = unique_nodes.get(&node.name) {
                            id_map.insert(node.id.clone(), existing_id.clone());
                        } else {
                            unique_nodes.insert(node.name.clone(), node.id.clone());
                            final_nodes.push(node.clone());
                        }
                    }
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
