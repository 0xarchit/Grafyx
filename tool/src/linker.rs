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
 
        let mut sorted_dirs: Vec<_> = self.dirs.iter().collect();
        sorted_dirs.sort_by_key(|d| std::cmp::Reverse(d.len()));
 
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
        for mut node in old_nodes {
            if node.kind == crate::ir::NodeKind::Root || node.kind == crate::ir::NodeKind::Service {
                final_nodes.push(node);
                continue;
            }
            
            // Assign service via longest prefix match
            for dir in &sorted_dirs {
                if node.file_path.starts_with(*dir) {
                    if let Some((svc_id, svc_name)) = service_map.get(*dir) {
                        node.service = svc_name.clone();
                        if node.kind == crate::ir::NodeKind::File {
                            graph.edges.push(crate::ir::Edge {
                                from_node_id: svc_id.clone(),
                                to_node_id: node.id.clone(),
                                relation_type: crate::ir::RelationType::RootLink,
                            });
                        }
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
                        let key = format!("{:?}::{}::{}", node.kind, node.name, node.file_path);
                        if let Some(existing_id) = unique_nodes.get(&key) {
                            id_map.insert(node.id.clone(), existing_id.clone());
                        } else {
                            unique_nodes.insert(key, node.id.clone());
                            final_nodes.push(node);
                        }
                    }
                }
                _ => {
                    let key = format!("{:?}::{}::{}", node.kind, node.name, node.file_path);
                    if let Some(existing_id) = unique_nodes.get(&key) {
                        id_map.insert(node.id.clone(), existing_id.clone());
                    } else {
                        unique_nodes.insert(key, node.id.clone());
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
        let mut final_edges = Vec::new();
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
