use crate::ir::Graph;
use std::collections::{HashMap, HashSet};

pub struct Linker {
    dirs: Vec<String>,
}

struct ServiceMapping {
    id: String,
    name: String,
    dir: String,
}

impl Linker {
    pub fn new(dirs: Vec<String>) -> Self {
        Self { dirs }
    }
 
    pub fn link(&self, graph: &mut Graph) {
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
 
        // Consolidate service mapping and sort by length for longest-prefix match
        let mut services: Vec<ServiceMapping> = self.dirs.iter().map(|dir| {
            let service_name = std::path::Path::new(dir)
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(dir);
            let service_id = format!("SERVICE::{}", service_name);
            
            ServiceMapping {
                id: service_id,
                name: service_name.to_string(),
                dir: dir.clone(),
            }
        }).collect();
        
        services.sort_by_key(|s| std::cmp::Reverse(s.dir.len()));

        for svc in &services {
            graph.nodes.push(crate::ir::Node {
                id: svc.id.clone(),
                kind: crate::ir::NodeKind::Service,
                name: svc.name.clone(),
                language: "service".to_string(),
                file_path: svc.dir.clone(),
                service: svc.name.clone(),
                start_line: 0,
                end_line: 0,
            });
            
            graph.edges.push(crate::ir::Edge {
                from_node_id: root_id.clone(),
                to_node_id: svc.id.clone(),
                relation_type: crate::ir::RelationType::RootLink,
            });
        }
 
        let mut unique_nodes: HashMap<String, String> = HashMap::new();
        let mut final_nodes = Vec::new();
        let mut id_map = HashMap::new();
        let mut file_path_to_id = HashMap::new();
        let mut file_stem_to_ids: HashMap<String, Vec<String>> = HashMap::new();
 
        for node in &graph.nodes {
            if node.kind == crate::ir::NodeKind::File {
                file_path_to_id.insert(node.file_path.clone(), node.id.clone());
                let path = std::path::Path::new(&node.file_path);
                if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                    file_stem_to_ids.entry(stem.to_string()).or_default().push(node.id.clone());
                }
            }
        }
 
        let old_nodes = std::mem::take(&mut graph.nodes);
        let mut key_buffer = String::with_capacity(256);
        
        for mut node in old_nodes {
            if node.kind == crate::ir::NodeKind::Root || node.kind == crate::ir::NodeKind::Service {
                final_nodes.push(node);
                continue;
            }
            
            // Assign service via longest prefix match
            for svc in &services {
                if node.file_path.starts_with(&svc.dir) {
                    node.service = svc.name.clone();
                    if node.kind == crate::ir::NodeKind::File {
                        graph.edges.push(crate::ir::Edge {
                            from_node_id: svc.id.clone(),
                            to_node_id: node.id.clone(),
                            relation_type: crate::ir::RelationType::RootLink,
                        });
                    }
                    break;
                }
            }
 
            match node.kind {
                crate::ir::NodeKind::File => {
                    final_nodes.push(node);
                }
                crate::ir::NodeKind::Module => {
                    // Try exact path first, then stem
                    if let Some(file_id) = file_path_to_id.get(&node.name).or_else(|| {
                         file_stem_to_ids.get(&node.name).and_then(|ids| ids.first())
                    }) {
                        id_map.insert(node.id.clone(), file_id.clone());
                    } else {
                        key_buffer.clear();
                        use std::fmt::Write;
                        let _ = write!(key_buffer, "{:?}::{}::{}", node.kind, node.name, node.file_path);
                        if let Some(existing_id) = unique_nodes.get(&key_buffer) {
                            id_map.insert(node.id.clone(), existing_id.clone());
                        } else {
                            unique_nodes.insert(key_buffer.clone(), node.id.clone());
                            final_nodes.push(node);
                        }
                    }
                }
                _ => {
                    key_buffer.clear();
                    use std::fmt::Write;
                    let _ = write!(key_buffer, "{:?}::{}::{}", node.kind, node.name, node.file_path);
                    if let Some(existing_id) = unique_nodes.get(&key_buffer) {
                        id_map.insert(node.id.clone(), existing_id.clone());
                    } else {
                        unique_nodes.insert(key_buffer.clone(), node.id.clone());
                        final_nodes.push(node);
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
        
        let mut unique_edges = HashSet::new();
        let mut final_edges = Vec::with_capacity(graph.edges.len());
        for edge in std::mem::take(&mut graph.edges) {
            if edge.from_node_id == edge.to_node_id { continue; }
            
            let sig = (edge.from_node_id.clone(), edge.to_node_id.clone(), edge.relation_type.clone());
            if !unique_edges.contains(&sig) {
                unique_edges.insert(sig);
                final_edges.push(edge);
            }
        }
        graph.edges = final_edges;
    }
}
