use crate::ir::{Edge, Graph, Node, NodeKind, RelationType};
use std::collections::{HashMap, HashSet};
use std::path::Path;

pub struct Linker {
    dirs: Vec<String>,
}

#[derive(Clone)]
struct ServiceMapping {
    id: String,
    name: String,
    dir: String,
}

impl Linker {
    pub fn new(dirs: Vec<String>) -> Self {
        Self { dirs }
    }

    fn normalize_path(path: &str) -> String {
        let mut normalized = path.replace('\\', "/");
        while normalized.len() > 1 && normalized.ends_with('/') {
            normalized.pop();
        }
        normalized
    }

    fn file_extensions_for_lookup() -> &'static [&'static str] {
        &[
            "", ".js", ".mjs", ".cjs", ".jsx", ".ts", ".tsx", ".tx", ".py", ".go", ".rs", ".java", "/index.js", "/index.ts",
            "/index.tsx", "/index.jsx", "/mod.rs",
        ]
    }

    fn path_from_file_id(file_id: &str) -> Option<String> {
        file_id.strip_prefix("FILE::").map(|p| p.to_string())
    }

    fn resolve_relative_import(from_file_path: &str, spec: &str) -> Vec<String> {
        let from_parent = Path::new(from_file_path).parent().unwrap_or_else(|| Path::new(""));
        let candidate = from_parent.join(spec);
        let mut out = Vec::new();
        for suffix in Self::file_extensions_for_lookup() {
            let joined = format!("{}{}", candidate.to_string_lossy(), suffix);
            out.push(Self::normalize_path(&joined));
        }
        out
    }

    fn resolve_dotted_import(spec: &str) -> Vec<String> {
        let slash_path = spec.replace('.', "/").replace("::", "/");
        let mut out = Vec::new();
        for suffix in Self::file_extensions_for_lookup() {
            out.push(Self::normalize_path(&format!("{}{}", slash_path, suffix)));
        }
        out
    }

    fn shared_path_prefix_depth(left: &str, right: &str) -> usize {
        let l = Path::new(left);
        let r = Path::new(right);
        l.components().zip(r.components()).take_while(|(a, b)| a == b).count()
    }

    fn choose_closest_id(
        from_file_path: &str,
        candidate_ids: &[String],
        id_to_path: &HashMap<String, String>,
    ) -> Option<String> {
        candidate_ids
            .iter()
            .max_by_key(|id| {
                let target_path = id_to_path.get(*id).map(String::as_str).unwrap_or("");
                Self::shared_path_prefix_depth(from_file_path, target_path)
            })
            .cloned()
    }

    fn external_package_name(spec: &str) -> String {
        let cleaned = spec.trim();
        if cleaned.is_empty() {
            return "unknown".to_string();
        }

        if cleaned.starts_with('@') {
            let mut parts = cleaned.split('/');
            let first = parts.next().unwrap_or(cleaned);
            let second = parts.next().unwrap_or("");
            if second.is_empty() {
                return first.to_string();
            }
            return format!("{}/{}", first, second);
        }

        cleaned
            .split(|c| c == '/' || c == '.' || c == ':')
            .find(|p| !p.is_empty())
            .unwrap_or("unknown")
            .to_string()
    }

    fn resolve_import_target(
        from_file_path: &str,
        from_lang: &str,
        from_service: &str,
        spec: &str,
        path_to_file_id: &HashMap<String, String>,
        stem_to_ids: &HashMap<(String, String), Vec<String>>,
        file_id_to_path: &HashMap<String, String>,
        nodes_by_id: &HashMap<String, Node>,
    ) -> Option<String> {
        if let Some(id) = path_to_file_id.get(&Self::normalize_path(spec)) {
            return Some(id.clone());
        }

        let mut candidates = Vec::new();

        if spec.starts_with("./") || spec.starts_with("../") || spec.starts_with('.') {
            candidates.extend(Self::resolve_relative_import(from_file_path, spec));
        } else {
            candidates.extend(Self::resolve_dotted_import(spec));
            candidates.extend(Self::resolve_relative_import(from_file_path, spec));
        }

        for candidate in candidates {
            if let Some(id) = path_to_file_id.get(&candidate) {
                return Some(id.clone());
            }
        }

        let normalized_spec = Self::normalize_path(spec);
        let mut suffix_targets: Vec<String> = Vec::new();
        for suffix in Self::file_extensions_for_lookup() {
            let key = Self::normalize_path(&format!("{}{}", normalized_spec, suffix));
            for (path, id) in path_to_file_id {
                if path.ends_with(&key) {
                    suffix_targets.push(id.clone());
                }
            }
        }
        suffix_targets.sort();
        suffix_targets.dedup();
        if !suffix_targets.is_empty() {
            let filtered: Vec<String> = suffix_targets
                .into_iter()
                .filter(|id| nodes_by_id.get(id).map_or(false, |n| n.language == from_lang))
                .collect();
            if !filtered.is_empty() {
                return Self::choose_closest_id(from_file_path, &filtered, file_id_to_path);
            }
        }

        let stem = spec
            .split(|c| c == '/' || c == '.' || c == ':')
            .filter(|s| !s.is_empty())
            .last()
            .unwrap_or("")
            .to_string();

        if let Some(matches) = stem_to_ids.get(&(from_lang.to_string(), stem.clone())) {
            let service_matches: Vec<String> = matches
                .iter()
                .filter(|id| nodes_by_id.get(*id).map_or(false, |n| n.service == from_service))
                .cloned()
                .collect();
            if !service_matches.is_empty() {
                return Self::choose_closest_id(from_file_path, &service_matches, file_id_to_path);
            }
            return Self::choose_closest_id(from_file_path, matches, file_id_to_path);
        }

        None
    }

    fn service_for_path(path: &str, services: &[ServiceMapping]) -> String {
        let normalized = Self::normalize_path(path);
        let node_path = Path::new(&normalized);
        for svc in services {
            let svc_path = Path::new(&svc.dir);
            if node_path.starts_with(svc_path) {
                return svc.id.clone();
            }
        }
        String::new()
    }

    pub fn link(&self, graph: &mut Graph) {
        let root_id = "GLOBAL::ROOT".to_string();

        let mut services: Vec<ServiceMapping> = self
            .dirs
            .iter()
            .map(|dir| {
                let normalized_dir = Self::normalize_path(dir);
                let path = Path::new(&normalized_dir);
                let service_name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or(&normalized_dir)
                    .to_string();

                ServiceMapping {
                    id: format!("SERVICE::{}", normalized_dir),
                    name: service_name,
                    dir: normalized_dir,
                }
            })
            .collect();

        services.sort_by_key(|s| std::cmp::Reverse(s.dir.len()));

        let mut nodes_by_id: HashMap<String, Node> = HashMap::new();
        for mut node in std::mem::take(&mut graph.nodes) {
            node.file_path = Self::normalize_path(&node.file_path);
            if node.service.is_empty() && !node.file_path.is_empty() {
                node.service = Self::service_for_path(&node.file_path, &services);
            }
            nodes_by_id.insert(node.id.clone(), node);
        }

        nodes_by_id.entry(root_id.clone()).or_insert(Node {
            id: root_id.clone(),
            kind: NodeKind::Root,
            name: "ROOT".to_string(),
            language: "global".to_string(),
            file_path: String::new(),
            service: "global".to_string(),
            start_line: 0,
            end_line: 0,
            weight: 0.0,
        });

        for svc in &services {
            nodes_by_id.entry(svc.id.clone()).or_insert(Node {
                id: svc.id.clone(),
                kind: NodeKind::Service,
                name: svc.name.clone(),
                language: "service".to_string(),
                file_path: svc.dir.clone(),
                service: svc.id.clone(),
                start_line: 0,
                end_line: 0,
                weight: 0.0,
            });
        }

        let mut path_to_file_id: HashMap<String, String> = HashMap::new();
        let mut file_id_to_path: HashMap<String, String> = HashMap::new();
        let mut stem_to_ids: HashMap<(String, String), Vec<String>> = HashMap::new();

        for node in nodes_by_id.values() {
            if node.kind == NodeKind::File {
                let normalized = Self::normalize_path(&node.file_path);
                path_to_file_id.insert(normalized.clone(), node.id.clone());
                file_id_to_path.insert(node.id.clone(), normalized.clone());
                if let Some(stem) = Path::new(&normalized).file_stem().and_then(|s| s.to_str()) {
                    stem_to_ids
                        .entry((node.language.clone(), stem.to_string()))
                        .or_default()
                        .push(node.id.clone());
                }
            }
        }

        let mut edge_counts: HashMap<(String, String, RelationType), f64> = HashMap::new();

        for svc in &services {
            *edge_counts
                .entry((root_id.clone(), svc.id.clone(), RelationType::RootLink))
                .or_insert(0.0) += 1.0;
        }

        for node in nodes_by_id.values() {
            if node.kind == NodeKind::File {
                if let Some(svc) = services.iter().find(|s| s.id == node.service) {
                    *edge_counts
                        .entry((svc.id.clone(), node.id.clone(), RelationType::Defines))
                        .or_insert(0.0) += 1.0;
                }
            }
        }

        let mut function_ids_by_file: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut function_ids_by_service: HashMap<(String, String, String), Vec<String>> = HashMap::new();
        let mut function_ids_by_name: HashMap<(String, String), Vec<String>> = HashMap::new();
        let mut function_id_to_path: HashMap<String, String> = HashMap::new();

        for node in nodes_by_id.values() {
            if node.kind == NodeKind::Function {
                function_ids_by_file
                    .entry(node.file_path.clone())
                    .or_default()
                    .push((node.name.clone(), node.id.clone()));
                function_ids_by_service
                    .entry((node.service.clone(), node.language.clone(), node.name.clone()))
                    .or_default()
                    .push(node.id.clone());
                function_ids_by_name
                    .entry((node.language.clone(), node.name.clone()))
                    .or_default()
                    .push(node.id.clone());
                function_id_to_path.insert(node.id.clone(), node.file_path.clone());
            }
        }

        let mut imported_file_ids_by_file_id: HashMap<String, Vec<String>> = HashMap::new();
        let mut external_module_ids: HashSet<String> = HashSet::new();
        let mut external_call_ids: HashSet<String> = HashSet::new();

        let mut non_import_edges = Vec::new();
        for edge in std::mem::take(&mut graph.edges) {
            if edge.from_node_id == edge.to_node_id {
                continue;
            }

            if edge.relation_type == RelationType::Imports {
                if let Some(spec) = edge.to_node_id.strip_prefix("IMPORT::") {
                    let from_file_path = if let Some(path) = file_id_to_path.get(&edge.from_node_id) {
                        path.clone()
                    } else if let Some(path) = Self::path_from_file_id(&edge.from_node_id) {
                        path
                    } else {
                        String::new()
                    };

                    if !from_file_path.is_empty() {
                        let from_lang = nodes_by_id.get(&edge.from_node_id).map(|n| n.language.as_str()).unwrap_or("");
                        let from_service = nodes_by_id.get(&edge.from_node_id).map(|n| n.service.as_str()).unwrap_or("");
                        if let Some(target_file_id) =
                            Self::resolve_import_target(
                                &from_file_path,
                                from_lang,
                                from_service,
                                spec,
                                &path_to_file_id,
                                &stem_to_ids,
                                &file_id_to_path,
                                &nodes_by_id,
                            )
                        {
                            imported_file_ids_by_file_id
                                .entry(edge.from_node_id.clone())
                                .or_default()
                                .push(target_file_id.clone());
                            *edge_counts
                                .entry((edge.from_node_id.clone(), target_file_id, RelationType::Imports))
                                .or_insert(0.0) += edge._w.max(1.0);
                        } else {
                            let package_name = Self::external_package_name(spec);
                            let package_id = format!("PKG::{}", package_name);
                            external_module_ids.insert(package_id.clone());
                            *edge_counts
                                .entry((edge.from_node_id.clone(), package_id, RelationType::Imports))
                                .or_insert(0.0) += edge._w.max(1.0);
                        }
                    }
                    continue;
                }
            }
            non_import_edges.push(edge);
        }

        for edge in non_import_edges {
            if edge.relation_type == RelationType::Calls {
                if let Some(call_name) = edge.to_node_id.strip_prefix("CALL::") {
                    let from_node = nodes_by_id.get(&edge.from_node_id);
                    let (from_file_path, from_service, from_file_id, from_lang) = if let Some(node) = from_node {
                        if node.kind == NodeKind::File {
                            (node.file_path.clone(), node.service.clone(), node.id.clone(), node.language.clone())
                        } else {
                            (
                                node.file_path.clone(),
                                node.service.clone(),
                                format!("FILE::{}", node.file_path),
                                node.language.clone(),
                            )
                        }
                    } else if let Some(path) = Self::path_from_file_id(&edge.from_node_id) {
                        let service = Self::service_for_path(&path, &services);
                        let lang = nodes_by_id.get(&edge.from_node_id).map(|n| n.language.clone()).unwrap_or_default();
                        (path.clone(), service, format!("FILE::{}", path), lang)
                    } else {
                        (String::new(), String::new(), String::new(), String::new())
                    };

                    let mut resolved: Option<String> = None;

                    if let Some(local_funcs) = function_ids_by_file.get(&from_file_path) {
                        if let Some((_, target_id)) = local_funcs.iter().find(|(name, _)| name == call_name) {
                            resolved = Some(target_id.clone());
                        }
                    }

                    if resolved.is_none() {
                        if let Some(imported_files) = imported_file_ids_by_file_id.get(&from_file_id) {
                            for imported_file_id in imported_files {
                                if let Some(imported_path) = file_id_to_path.get(imported_file_id) {
                                    if let Some(imported_funcs) = function_ids_by_file.get(imported_path) {
                                        if let Some((_, target_id)) =
                                            imported_funcs.iter().find(|(name, _)| name == call_name)
                                        {
                                            resolved = Some(target_id.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if resolved.is_none() {
                        if let Some(candidates) =
                            function_ids_by_service.get(&(from_service.clone(), from_lang.clone(), call_name.to_string()))
                        {
                            resolved = Self::choose_closest_id(&from_file_path, candidates, &function_id_to_path);
                        }
                    }

                    if resolved.is_none() {
                        if let Some(candidates) = function_ids_by_name.get(&(from_lang.clone(), call_name.to_string())) {
                            resolved = Self::choose_closest_id(&from_file_path, candidates, &function_id_to_path);
                        }
                    }

                    if let Some(target_id) = resolved {
                        *edge_counts
                            .entry((edge.from_node_id.clone(), target_id, RelationType::Calls))
                            .or_insert(0.0) += edge._w.max(1.0);
                    } else {
                        let ext_id = format!("EXT::{}", call_name);
                        external_call_ids.insert(ext_id.clone());
                        *edge_counts
                            .entry((edge.from_node_id.clone(), ext_id, RelationType::Calls))
                            .or_insert(0.0) += edge._w.max(1.0);
                    }

                    continue;
                }
            }

            if nodes_by_id.contains_key(&edge.from_node_id) && nodes_by_id.contains_key(&edge.to_node_id) {
                *edge_counts
                    .entry((edge.from_node_id, edge.to_node_id, edge.relation_type))
                    .or_insert(0.0) += edge._w.max(1.0);
            }
        }

        for package_id in &external_module_ids {
            if !nodes_by_id.contains_key(package_id) {
                let package_name = package_id.strip_prefix("PKG::").unwrap_or(package_id).to_string();
                nodes_by_id.insert(
                    package_id.clone(),
                    Node {
                        id: package_id.clone(),
                        kind: NodeKind::Module,
                        name: package_name,
                        language: "external".to_string(),
                        file_path: "external".to_string(),
                        service: "external".to_string(),
                        start_line: 0,
                        end_line: 0,
                        weight: 0.5,
                    },
                );
            }
        }

        for ext_id in &external_call_ids {
            if !nodes_by_id.contains_key(ext_id) {
                let ext_name = ext_id.strip_prefix("EXT::").unwrap_or(ext_id).to_string();
                nodes_by_id.insert(
                    ext_id.clone(),
                    Node {
                        id: ext_id.clone(),
                        kind: NodeKind::Call,
                        name: ext_name,
                        language: "external".to_string(),
                        file_path: "external".to_string(),
                        service: "external".to_string(),
                        start_line: 0,
                        end_line: 0,
                        weight: 0.5,
                    },
                );
            }
        }

        let mut node_to_service_id: HashMap<String, String> = HashMap::new();
        for node in nodes_by_id.values() {
            if node.service.is_empty() || node.service == "global" || node.service == "external" {
                continue;
            }
            if let Some(svc) = services.iter().find(|s| s.id == node.service) {
                node_to_service_id.insert(node.id.clone(), svc.id.clone());
            }
        }

        let mut service_edges: HashMap<(String, String), f64> = HashMap::new();
        for ((from, to, rel), weight) in &edge_counts {
            if *rel == RelationType::RootLink || *rel == RelationType::Defines {
                continue;
            }
            if let (Some(from_svc), Some(to_svc)) = (node_to_service_id.get(from), node_to_service_id.get(to)) {
                if from_svc != to_svc {
                    *service_edges.entry((from_svc.clone(), to_svc.clone())).or_insert(0.0) += *weight;
                }
            }
        }

        for ((from, to), weight) in service_edges {
            *edge_counts
                .entry((from, to, RelationType::ServiceCall))
                .or_insert(0.0) += weight;
        }

        let mut existing_ids: HashSet<String> = nodes_by_id.keys().cloned().collect();
        let mut finalized_edges = Vec::new();
        let mut referenced: HashSet<String> = HashSet::new();

        for ((from, to, rel), weight) in edge_counts {
            if from == to {
                continue;
            }
            if !(existing_ids.contains(&from) && existing_ids.contains(&to)) {
                continue;
            }
            referenced.insert(from.clone());
            referenced.insert(to.clone());
            finalized_edges.push(Edge {
                from_node_id: from,
                to_node_id: to,
                relation_type: rel,
                _w: weight,
            });
        }

        let mut finalized_nodes = Vec::new();
        for (_, mut node) in nodes_by_id {
            let mut keep = matches!(node.kind, NodeKind::Root | NodeKind::Service) || referenced.contains(&node.id);
            
            if !keep && (node.kind == NodeKind::Function || node.kind == NodeKind::Class) {
                let file_id = format!("FILE::{}", node.file_path);
                if referenced.contains(&file_id) {
                    keep = true;
                }
            }

            if keep {
                node.weight = 0.0;
                finalized_nodes.push(node);
            }
        }

        let kept_ids: HashSet<String> = finalized_nodes.iter().map(|n| n.id.clone()).collect();
        finalized_edges.retain(|e| kept_ids.contains(&e.from_node_id) && kept_ids.contains(&e.to_node_id));

        existing_ids = kept_ids;

        let mut node_weights: HashMap<String, f64> = HashMap::new();
        for edge in &finalized_edges {
            *node_weights.entry(edge.to_node_id.clone()).or_insert(0.0) += edge._w;
            *node_weights.entry(edge.from_node_id.clone()).or_insert(0.0) += edge._w * 0.5;
        }

        for node in &mut finalized_nodes {
            if existing_ids.contains(&node.id) {
                node.weight = node_weights.get(&node.id).cloned().unwrap_or(1.0);
            }
        }

        graph.nodes = finalized_nodes;
        graph.edges = finalized_edges;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_node(id: &str, path: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::File,
            name: path.to_string(),
            language: "python".to_string(),
            file_path: path.to_string(),
            service: String::new(),
            start_line: 0,
            end_line: 10,
            weight: 1.0,
        }
    }

    fn func_node(id: &str, name: &str, path: &str) -> Node {
        Node {
            id: id.to_string(),
            kind: NodeKind::Function,
            name: name.to_string(),
            language: "python".to_string(),
            file_path: path.to_string(),
            service: String::new(),
            start_line: 1,
            end_line: 3,
            weight: 1.0,
        }
    }

    #[test]
    fn resolves_internal_import_and_call() {
        let a_path = "/repo/a.py";
        let b_path = "/repo/b.py";
        let a_file = format!("FILE::{}", a_path);
        let b_file = format!("FILE::{}", b_path);
        let foo_id = format!("FUNC::{}::foo::1", a_path);
        let bar_id = format!("FUNC::{}::bar::1", b_path);

        let mut graph = Graph {
            nodes: vec![
                file_node(&a_file, a_path),
                file_node(&b_file, b_path),
                func_node(&foo_id, "foo", a_path),
                func_node(&bar_id, "bar", b_path),
            ],
            edges: vec![
                Edge {
                    from_node_id: a_file.clone(),
                    to_node_id: foo_id.clone(),
                    relation_type: RelationType::Defines,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: b_file.clone(),
                    to_node_id: bar_id.clone(),
                    relation_type: RelationType::Defines,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: b_file.clone(),
                    to_node_id: "IMPORT::./a".to_string(),
                    relation_type: RelationType::Imports,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: bar_id.clone(),
                    to_node_id: "CALL::foo".to_string(),
                    relation_type: RelationType::Calls,
                    _w: 1.0,
                },
            ],
        };

        Linker::new(vec!["/repo".to_string()]).link(&mut graph);

        assert!(graph
            .edges
            .iter()
            .any(|e| e.from_node_id == b_file && e.to_node_id == a_file && e.relation_type == RelationType::Imports));

        assert!(graph
            .edges
            .iter()
            .any(|e| e.from_node_id == bar_id && e.to_node_id == foo_id && e.relation_type == RelationType::Calls));

        assert!(!graph.nodes.iter().any(|n| n.id == "EXT::foo"));
        assert!(!graph.nodes.iter().any(|n| n.id.starts_with("IMPORT::")));
    }

    #[test]
    fn classifies_external_import_and_prunes_orphan() {
        let file_path = "/repo/index.js";
        let file_id = format!("FILE::{}", file_path);

        let mut graph = Graph {
            nodes: vec![
                Node {
                    id: file_id.clone(),
                    kind: NodeKind::File,
                    name: file_path.to_string(),
                    language: "javascript".to_string(),
                    file_path: file_path.to_string(),
                    service: String::new(),
                    start_line: 0,
                    end_line: 10,
                    weight: 1.0,
                },
                Node {
                    id: "FUNC::orphan".to_string(),
                    kind: NodeKind::Function,
                    name: "orphan".to_string(),
                    language: "javascript".to_string(),
                    file_path: file_path.to_string(),
                    service: String::new(),
                    start_line: 1,
                    end_line: 1,
                    weight: 1.0,
                },
            ],
            edges: vec![Edge {
                from_node_id: file_id.clone(),
                to_node_id: "IMPORT::react".to_string(),
                relation_type: RelationType::Imports,
                _w: 1.0,
            }],
        };

        Linker::new(vec!["/repo".to_string()]).link(&mut graph);

        assert!(graph.nodes.iter().any(|n| n.id == "PKG::react"));
        assert!(graph
            .edges
            .iter()
            .any(|e| e.from_node_id == file_id && e.to_node_id == "PKG::react" && e.relation_type == RelationType::Imports));
        assert!(graph.nodes.iter().any(|n| n.id == "FUNC::orphan"));
    }

    #[test]
    fn resolves_duplicate_filenames_using_path_proximity() {
        let a_utils = "/repo/a/utils.py";
        let b_utils = "/repo/b/utils.py";
        let b_main = "/repo/b/main.py";

        let a_utils_file = format!("FILE::{}", a_utils);
        let b_utils_file = format!("FILE::{}", b_utils);
        let b_main_file = format!("FILE::{}", b_main);

        let a_helper = format!("FUNC::{}::helper::1", a_utils);
        let b_helper = format!("FUNC::{}::helper::1", b_utils);
        let b_runner = format!("FUNC::{}::runner::1", b_main);

        let mut graph = Graph {
            nodes: vec![
                file_node(&a_utils_file, a_utils),
                file_node(&b_utils_file, b_utils),
                file_node(&b_main_file, b_main),
                func_node(&a_helper, "helper", a_utils),
                func_node(&b_helper, "helper", b_utils),
                func_node(&b_runner, "runner", b_main),
            ],
            edges: vec![
                Edge {
                    from_node_id: a_utils_file.clone(),
                    to_node_id: a_helper.clone(),
                    relation_type: RelationType::Defines,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: b_utils_file.clone(),
                    to_node_id: b_helper.clone(),
                    relation_type: RelationType::Defines,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: b_main_file.clone(),
                    to_node_id: b_runner.clone(),
                    relation_type: RelationType::Defines,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: b_main_file.clone(),
                    to_node_id: "IMPORT::./utils".to_string(),
                    relation_type: RelationType::Imports,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: b_runner.clone(),
                    to_node_id: "CALL::helper".to_string(),
                    relation_type: RelationType::Calls,
                    _w: 1.0,
                },
            ],
        };

        Linker::new(vec!["/repo".to_string()]).link(&mut graph);

        assert!(graph
            .edges
            .iter()
            .any(|e| e.from_node_id == b_main_file && e.to_node_id == b_utils_file && e.relation_type == RelationType::Imports));
        assert!(graph
            .edges
            .iter()
            .any(|e| e.from_node_id == b_runner && e.to_node_id == b_helper && e.relation_type == RelationType::Calls));
    }

    #[test]
    fn isolates_language_scopes_for_calls() {
        let py_path = "/repo/app.py";
        let rs_path = "/repo/main.rs";
        let py_file = format!("FILE::{}", py_path);
        let rs_file = format!("FILE::{}", rs_path);
        let py_caller = format!("FUNC::{}::caller::1", py_path);
        let py_callee = format!("FUNC::{}::run::1", py_path);
        let rs_callee = format!("FUNC::{}::run::1", rs_path);

        let mut graph = Graph {
            nodes: vec![
                Node {
                    id: py_file.clone(),
                    kind: NodeKind::File,
                    name: "app.py".into(),
                    language: "python".into(),
                    file_path: py_path.into(),
                    service: "SERVICE::/repo".into(),
                    start_line: 0,
                    end_line: 10,
                    weight: 1.0,
                },
                Node {
                    id: rs_file.clone(),
                    kind: NodeKind::File,
                    name: "main.rs".into(),
                    language: "rust".into(),
                    file_path: rs_path.into(),
                    service: "SERVICE::/repo".into(),
                    start_line: 0,
                    end_line: 10,
                    weight: 1.0,
                },
                Node {
                    id: py_caller.clone(),
                    kind: NodeKind::Function,
                    name: "caller".into(),
                    language: "python".into(),
                    file_path: py_path.into(),
                    service: "SERVICE::/repo".into(),
                    start_line: 1,
                    end_line: 2,
                    weight: 1.0,
                },
                Node {
                    id: py_callee.clone(),
                    kind: NodeKind::Function,
                    name: "run".into(),
                    language: "python".into(),
                    file_path: py_path.into(),
                    service: "SERVICE::/repo".into(),
                    start_line: 3,
                    end_line: 4,
                    weight: 1.0,
                },
                Node {
                    id: rs_callee.clone(),
                    kind: NodeKind::Function,
                    name: "run".into(),
                    language: "rust".into(),
                    file_path: rs_path.into(),
                    service: "SERVICE::/repo".into(),
                    start_line: 1,
                    end_line: 1,
                    weight: 1.0,
                },
            ],
            edges: vec![
                Edge {
                    from_node_id: py_file.clone(),
                    to_node_id: py_caller.clone(),
                    relation_type: RelationType::Defines,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: py_file.clone(),
                    to_node_id: py_callee.clone(),
                    relation_type: RelationType::Defines,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: rs_file.clone(),
                    to_node_id: rs_callee.clone(),
                    relation_type: RelationType::Defines,
                    _w: 1.0,
                },
                Edge {
                    from_node_id: py_caller.clone(),
                    to_node_id: "CALL::run".into(),
                    relation_type: RelationType::Calls,
                    _w: 1.0,
                },
            ],
        };

        Linker::new(vec!["/repo".to_string()]).link(&mut graph);

        assert!(graph.edges.iter().any(|e| {
            e.from_node_id == py_caller && e.to_node_id == py_callee && e.relation_type == RelationType::Calls
        }));
        assert!(!graph.edges.iter().any(|e| {
            e.from_node_id == py_caller && e.to_node_id == rs_callee
        }));
    }

    #[test]
    fn isolates_language_scopes_for_imports() {
        let js_path = "/repo/config.js";
        let go_path = "/repo/config.go";
        let js_file = format!("FILE::{}", js_path);
        let go_file = format!("FILE::{}", go_path);
        let importer_path = "/repo/app.js";
        let importer_file = format!("FILE::{}", importer_path);

        let mut graph = Graph {
            nodes: vec![
                Node {
                    id: js_file.clone(),
                    kind: NodeKind::File,
                    name: "config.js".into(),
                    language: "javascript".into(),
                    file_path: js_path.into(),
                    service: "SERVICE::/repo".into(),
                    start_line: 0,
                    end_line: 10,
                    weight: 1.0,
                },
                Node {
                    id: go_file.clone(),
                    kind: NodeKind::File,
                    name: "config.go".into(),
                    language: "go".into(),
                    file_path: go_path.into(),
                    service: "SERVICE::/repo".into(),
                    start_line: 0,
                    end_line: 10,
                    weight: 1.0,
                },
                Node {
                    id: importer_file.clone(),
                    kind: NodeKind::File,
                    name: "app.js".into(),
                    language: "javascript".into(),
                    file_path: importer_path.into(),
                    service: "SERVICE::/repo".into(),
                    start_line: 0,
                    end_line: 10,
                    weight: 1.0,
                },
            ],
            edges: vec![
                Edge {
                    from_node_id: importer_file.clone(),
                    to_node_id: "IMPORT::config".into(),
                    relation_type: RelationType::Imports,
                    _w: 1.0,
                },
            ],
        };

        Linker::new(vec!["/repo".to_string()]).link(&mut graph);

        assert!(graph.edges.iter().any(|e| {
            e.from_node_id == importer_file && e.to_node_id == js_file && e.relation_type == RelationType::Imports
        }));
        assert!(!graph.edges.iter().any(|e| {
            e.from_node_id == importer_file && e.to_node_id == go_file
        }));
    }
}
