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
            let path = std::path::Path::new(dir);
            let service_name = path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(dir);
            
            let service_id = format!("SERVICE::{}", dir);
            
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
        let mut id_to_file_path = HashMap::new();
        let mut file_stem_to_ids: HashMap<String, Vec<String>> = HashMap::new();
        for node in &graph.nodes {
            if node.kind == crate::ir::NodeKind::File {
                file_path_to_id.insert(node.file_path.clone(), node.id.clone());
                id_to_file_path.insert(node.id.clone(), node.file_path.clone());
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
            
            // Assign service via longest prefix match (checking path boundaries)
            for svc in &services {
                let node_path = std::path::Path::new(&node.file_path);
                let svc_path = std::path::Path::new(&svc.dir);
                
                if node_path.starts_with(svc_path) {
                    node.service = svc.name.clone();
                    if node.kind == crate::ir::NodeKind::File {
                        graph.edges.push(crate::ir::Edge {
                            from_node_id: svc.id.clone(),
                            to_node_id: node.id.clone(),
                            relation_type: crate::ir::RelationType::ServiceCall,
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
                    // Disambiguation: If multiple files match a stem (e.g. auth.py),
                    // prioritize the one in the same directory or closest ancestor.
                    let target_id = file_path_to_id.get(&node.name).cloned().or_else(|| {
                         file_stem_to_ids.get(&node.name).and_then(|ids: &Vec<String>| {
                            if ids.len() == 1 {
                                Some(ids[0].clone())
                            } else {
                                // Scoped search: find ID with maximum shared path prefix with node.file_path
                                ids.iter().max_by_key(|id| {
                                    if let Some(target_path) = id_to_file_path.get(*id) {
                                        let p1 = std::path::Path::new(&node.file_path);
                                        let p2 = std::path::Path::new(target_path);
                                        let mut score = 0;
                                        for (c1, c2) in p1.components().zip(p2.components()) {
                                            if c1 == c2 {
                                                score += 1;
                                            } else {
                                                break;
                                            }
                                        }
                                        score
                                    } else {
                                        0
                                    }
                                }).cloned()
                            }
                         })
                    });

                    if let Some(file_id) = target_id {
                        id_map.insert(node.id.clone(), file_id);
                    } else {
                        key_buffer.clear();
                        key_buffer.push_str(&node.kind.to_string());
                        key_buffer.push_str("::");
                        key_buffer.push_str(&node.name);
                        key_buffer.push_str("::");
                        key_buffer.push_str(&node.file_path);
                        key_buffer.push_str("::");
                        key_buffer.push_str(&node.start_line.to_string());
                        key_buffer.push_str("-");
                        key_buffer.push_str(&node.end_line.to_string());
                        
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
                    key_buffer.push_str(&node.kind.to_string());
                    key_buffer.push_str("::");
                    key_buffer.push_str(&node.name);
                    key_buffer.push_str("::");
                    key_buffer.push_str(&node.file_path);
                    key_buffer.push_str("::");
                    key_buffer.push_str(&node.start_line.to_string());
                    key_buffer.push_str("-");
                    key_buffer.push_str(&node.end_line.to_string());

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
            
            // Signature optimization: use references for hashing where possible
            // but we need a stable key.
            let sig = (edge.from_node_id.clone(), edge.to_node_id.clone(), edge.relation_type.clone());
            if !unique_edges.contains(&sig) {
                unique_edges.insert(sig);
                final_edges.push(edge);
            }
        }
        graph.edges = final_edges;
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Node, NodeKind, Edge, RelationType};

    #[test]
    fn test_linker_service_mapping() {
        let linker = Linker::new(vec!["/path/to/app".to_string(), "/path/to/app/service1".to_string()]);
        let mut graph = Graph::new();
        graph.nodes.push(Node {
            id: "node1".to_string(),
            kind: NodeKind::File,
            name: "test.py".to_string(),
            language: "python".to_string(),
            file_path: "/path/to/app/service1/test.py".to_string(),
            service: "".to_string(),
            start_line: 0,
            end_line: 0,
        });

        linker.link(&mut graph);

        // Check if node1 got assigned to service1 (longest prefix)
        let node = graph.nodes.iter().find(|n| n.id == "node1").unwrap();
        assert_eq!(node.service, "service1");
        
        // Root node + 2 service nodes + node1 = 4 nodes
        assert_eq!(graph.nodes.len(), 4);
    }

    #[test]
    fn test_linker_module_remapping() {
        let linker = Linker::new(vec!["/app".to_string()]);
        let mut graph = Graph::new();
        
        // Target file
        graph.nodes.push(Node {
            id: "file1_id".to_string(),
            kind: NodeKind::File,
            name: "/app/lib.py".to_string(),
            language: "python".to_string(),
            file_path: "/app/lib.py".to_string(),
            service: "".to_string(),
            start_line: 0,
            end_line: 0,
        });

        // Module node representing an import of lib.py
        graph.nodes.push(Node {
            id: "import_node_id".to_string(),
            kind: NodeKind::Module,
            name: "lib".to_string(), // Stem-based match
            language: "python".to_string(),
            file_path: "/app/main.py".to_string(),
            service: "".to_string(),
            start_line: 1,
            end_line: 1,
        });
        // Source file that imports lib
        graph.nodes.push(Node {
            id: "main_file_id".to_string(),
            kind: NodeKind::File,
            name: "/app/main.py".to_string(),
            language: "python".to_string(),
            file_path: "/app/main.py".to_string(),
            service: "".to_string(),
            start_line: 0,
            end_line: 0,
        });

        graph.edges.push(Edge {
            from_node_id: "main_file_id".to_string(),
            to_node_id: "import_node_id".to_string(),
            relation_type: RelationType::Imports,
        });

        linker.link(&mut graph);

        // The edge to import_node_id should be remapped to file1_id
        assert!(graph.edges.iter().any(|e| e.to_node_id == "file1_id"));
    }
}
