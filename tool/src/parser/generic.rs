use crate::ir::{Edge, Node, NodeKind, RelationType};
use crate::parser::CodeParser;
use anyhow::{Context, Result};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use tree_sitter::{Language, Parser, Query, QueryCursor, StreamingIterator};

pub struct GenericParser {
    language: Language,
    lang_name: String,
}

impl GenericParser {
    pub fn new(language: Language, lang_name: &str) -> Self {
        Self {
            language,
            lang_name: lang_name.to_string(),
        }
    }

    fn get_query(&self) -> &str {
        match self.lang_name.as_str() {
            "javascript" | "jsx" | "js" => r#"
                (function_declaration name: (identifier) @func.def)
                (method_definition name: (property_identifier) @func.def)
                (variable_declarator name: (identifier) @func.def value: (arrow_function))
                (variable_declarator name: (identifier) @func.def value: (function_expression))
                (assignment_expression left: (identifier) @func.def right: (arrow_function))
                (assignment_expression left: (identifier) @func.def right: (function_expression))
                (class_declaration name: (identifier) @class.def)
                (import_statement) @import
                ((call_expression function: (identifier) @require.fn arguments: (arguments (string) @import.require))
                  (#eq? @require.fn "require"))
                (call_expression function: (identifier) @call (#not-eq? @call "require"))
                (call_expression function: (member_expression property: (property_identifier) @call))
            "#,
            "typescript" | "tsx" | "tx" => r#"
                (function_declaration name: (identifier) @func.def)
                (method_definition name: (property_identifier) @func.def)
                (variable_declarator name: (identifier) @func.def value: (arrow_function))
                (variable_declarator name: (identifier) @func.def value: (function_expression))
                (assignment_expression left: (identifier) @func.def right: (arrow_function))
                (assignment_expression left: (identifier) @func.def right: (function_expression))
                (class_declaration name: (type_identifier) @class.def)
                (import_statement) @import
                ((call_expression function: (identifier) @require.fn arguments: (arguments (string) @import.require))
                  (#eq? @require.fn "require"))
                (call_expression function: (identifier) @call (#not-eq? @call "require"))
                (call_expression function: (member_expression property: (property_identifier) @call))
            "#,
            "python" => r#"
                (function_definition name: (identifier) @func.def)
                (class_definition name: (identifier) @class.def)
                (import_from_statement) @import
                (import_statement) @import
                (call function: (_) @call)
            "#,
            "java" => r#"
                (method_declaration name: (identifier) @func.def)
                (constructor_declaration name: (identifier) @func.def)
                (class_declaration name: (identifier) @class.def)
                (interface_declaration name: (identifier) @class.def)
                (import_declaration) @import
                (method_invocation name: (identifier) @call)
            "#,
            "go" => r#"
                (function_declaration name: (identifier) @func.def)
                (method_declaration name: (field_identifier) @func.def)
                (type_declaration (type_spec name: (type_identifier) @class.def))
                (import_spec) @import
                (call_expression function: (_) @call)
            "#,
            "rust" => r#"
                (function_item name: (identifier) @func.def)
                (struct_item name: (type_identifier) @class.def)
                (enum_item name: (type_identifier) @class.def)
                (mod_item name: (identifier) @class.def)
                (use_declaration) @import
                (call_expression function: (_) @call)
            "#,
            _ => "(ERROR) @error",
        }
    }

    fn clean_name(name: &str) -> String {
        name.trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '{' || c == '}' || c == '(' || c == ')')
            .to_string()
    }

    fn normalize_import_spec(spec: &str) -> Option<String> {
        let cleaned = spec
            .trim()
            .trim_end_matches(';')
            .trim_matches(|c| c == '"' || c == '\'' || c == '`')
            .trim()
            .to_string();

        if cleaned.is_empty() {
            None
        } else {
            Some(cleaned)
        }
    }

    fn quoted_segments(text: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut in_quote: Option<char> = None;
        let mut buf = String::new();

        for ch in text.chars() {
            if let Some(q) = in_quote {
                if ch == q {
                    if !buf.trim().is_empty() {
                        out.push(buf.trim().to_string());
                    }
                    buf.clear();
                    in_quote = None;
                } else {
                    buf.push(ch);
                }
            } else if ch == '"' || ch == '\'' || ch == '`' {
                in_quote = Some(ch);
            }
        }

        out
    }

    fn extract_js_ts_imports(raw: &str) -> Vec<String> {
        let segments = Self::quoted_segments(raw);
        if segments.is_empty() {
            Vec::new()
        } else {
            vec![segments.last().cloned().unwrap_or_default()]
        }
    }

    fn extract_python_imports(raw: &str) -> Vec<String> {
        let mut out = Vec::new();
        let trimmed = raw.trim();

        if let Some(rest) = trimmed.strip_prefix("import ") {
            for part in rest.split(',') {
                let item = part
                    .trim()
                    .split(" as ")
                    .next()
                    .unwrap_or("")
                    .trim();
                if !item.is_empty() {
                    out.push(item.to_string());
                }
            }
            return out;
        }

        if let Some(rest) = trimmed.strip_prefix("from ") {
            let base = rest.split(" import ").next().unwrap_or("").trim();
            if !base.is_empty() {
                out.push(base.to_string());
            }
        }

        out
    }

    fn extract_java_imports(raw: &str) -> Vec<String> {
        let trimmed = raw.trim().trim_end_matches(';').trim();
        let without_import = trimmed
            .strip_prefix("import static ")
            .or_else(|| trimmed.strip_prefix("import "))
            .unwrap_or(trimmed)
            .trim();

        if without_import.is_empty() {
            Vec::new()
        } else {
            vec![without_import.to_string()]
        }
    }

    fn extract_rust_imports(raw: &str) -> Vec<String> {
        let trimmed = raw.trim().trim_end_matches(';').trim();
        let mut body = trimmed;
        if let Some(rest) = body.strip_prefix("pub ") {
            body = rest.trim();
        }
        body = body.strip_prefix("use ").unwrap_or(body).trim();
        if body.is_empty() {
            return Vec::new();
        }

        if let (Some(left), Some(right)) = (body.find('{'), body.rfind('}')) {
            let prefix = body[..left].trim().trim_end_matches("::");
            let inside = &body[left + 1..right];
            let mut out = Vec::new();
            for item in inside.split(',') {
                let atom = item.trim();
                if atom.is_empty() {
                    continue;
                }
                if atom == "self" {
                    if !prefix.is_empty() {
                        out.push(prefix.to_string());
                    }
                    continue;
                }
                if prefix.is_empty() {
                    out.push(atom.to_string());
                } else {
                    out.push(format!("{}::{}", prefix, atom));
                }
            }
            return out;
        }

        vec![body.to_string()]
    }

    fn extract_go_imports(raw: &str) -> Vec<String> {
        let quoted = Self::quoted_segments(raw);
        if !quoted.is_empty() {
            return quoted;
        }

        let trimmed = raw.trim();
        if let Some(rest) = trimmed.strip_prefix("import ") {
            let candidate = rest
                .split_whitespace()
                .last()
                .unwrap_or("")
                .trim_matches(|c| c == '"' || c == '\'');
            if !candidate.is_empty() {
                return vec![candidate.to_string()];
            }
        }

        Vec::new()
    }

    fn extract_import_specs(&self, raw: &str) -> Vec<String> {
        let specs = match self.lang_name.as_str() {
            "javascript" | "jsx" | "js" | "typescript" | "tsx" | "tx" => {
                Self::extract_js_ts_imports(raw)
            }
            "python" => Self::extract_python_imports(raw),
            "java" => Self::extract_java_imports(raw),
            "go" => Self::extract_go_imports(raw),
            "rust" => Self::extract_rust_imports(raw),
            _ => Vec::new(),
        };

        specs
            .into_iter()
            .filter_map(|s| Self::normalize_import_spec(&s))
            .collect()
    }

    fn normalize_call_name(raw: &str) -> Option<String> {
        let mut cleaned = raw.trim();
        if cleaned.is_empty() {
            return None;
        }

        if let Some(idx) = cleaned.find('(') {
            cleaned = &cleaned[..idx];
        }

        let cleaned = cleaned
            .trim()
            .trim_matches(|c| c == '"' || c == '\'' || c == '`' || c == '{' || c == '}' || c == '(' || c == ')');

        if cleaned.is_empty() {
            return None;
        }

        let canonical = cleaned.replace("::", ".").replace("->", ".");
        let tail = canonical
            .split('.')
            .filter(|s| !s.trim().is_empty())
            .last()
            .unwrap_or(cleaned)
            .trim();

        if tail.is_empty() {
            None
        } else {
            Some(tail.to_string())
        }
    }
}

impl CodeParser for GenericParser {
    fn parse(&self, file_path: &Path, content: &str) -> Result<(Vec<Node>, Vec<Edge>)> {
        let mut parser = Parser::new();
        parser.set_language(&self.language).context("Failed to set language")?;
        
        let tree = parser.parse(content, None).context("Failed to parse content")?;
        let root_node = tree.root_node();
        
        let query_str = self.get_query();
        let query = Query::new(&self.language, query_str).context("Failed to create query")?;
        let mut cursor = QueryCursor::new();
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        let mut seen_nodes: HashSet<String> = HashSet::new();
        let mut seen_edges: HashSet<(String, String, RelationType)> = HashSet::new();
        let mut function_scopes: Vec<(usize, usize, String)> = Vec::new();
        let mut function_keys: HashMap<(String, usize), String> = HashMap::new();

        let file_path_str = file_path.to_string_lossy().to_string().replace('\\', "/");
        let file_node_id = format!("FILE::{}", file_path_str);

        seen_nodes.insert(file_node_id.clone());
        nodes.push(Node {
            id: file_node_id.clone(),
            kind: NodeKind::File,
            name: file_path_str.clone(),
            language: self.lang_name.clone(),
            file_path: file_path_str.clone(),
            service: "".to_string(),
            start_line: 0,
            end_line: content.lines().count(),
            weight: 1.0,
        });

        let mut matches = cursor.matches(&query, root_node, content.as_bytes());
        while let Some(m) = matches.next() {
            for capture in m.captures {
                let capture_name = query.capture_names()[capture.index as usize];
                let raw_text = capture.node.utf8_text(content.as_bytes()).unwrap_or("").to_string();
                let node_text = Self::clean_name(&raw_text);
                let start_line = capture.node.start_position().row + 1;
                let end_line = capture.node.end_position().row + 1;

                match capture_name {
                    "func.def" => {
                        let node_id = format!("FUNC::{}::{}::{}", file_path_str, node_text, start_line);
                        if seen_nodes.insert(node_id.clone()) {
                            nodes.push(Node {
                                id: node_id.clone(),
                                kind: NodeKind::Function,
                                name: node_text.clone(),
                                language: self.lang_name.clone(),
                                file_path: file_path_str.clone(),
                                service: "".to_string(),
                                start_line,
                                end_line,
                                weight: 1.0,
                            });
                        }
                        function_keys.insert((node_text.clone(), start_line), node_id.clone());
                        let decl_node = capture
                            .node
                            .parent()
                            .filter(|p| {
                                p.kind().contains("function")
                                    || p.kind().contains("method")
                                    || p.kind().contains("constructor")
                                    || p.kind() == "variable_declarator"
                                    || p.kind() == "assignment_expression"
                            })
                            .unwrap_or(capture.node);
                        function_scopes.push((decl_node.start_byte(), decl_node.end_byte(), node_id.clone()));
                        let edge_key = (file_node_id.clone(), node_id.clone(), RelationType::Defines);
                        if seen_edges.insert(edge_key.clone()) {
                            edges.push(Edge {
                                from_node_id: edge_key.0,
                                to_node_id: edge_key.1,
                                relation_type: edge_key.2,
                                _w: 1.0,
                            });
                        }
                    }
                    "class.def" => {
                        let node_id = format!("CLASS::{}::{}::{}", file_path_str, node_text, start_line);
                        if seen_nodes.insert(node_id.clone()) {
                            nodes.push(Node {
                                id: node_id.clone(),
                                kind: NodeKind::Class,
                                name: node_text,
                                language: self.lang_name.clone(),
                                file_path: file_path_str.clone(),
                                service: "".to_string(),
                                start_line,
                                end_line,
                                weight: 1.0,
                            });
                        }
                        let edge_key = (file_node_id.clone(), node_id.clone(), RelationType::Defines);
                        if seen_edges.insert(edge_key.clone()) {
                            edges.push(Edge {
                                from_node_id: edge_key.0,
                                to_node_id: edge_key.1,
                                relation_type: edge_key.2,
                                _w: 1.0,
                            });
                        }
                    }
                    "import" => {
                        let specs = self.extract_import_specs(&raw_text);
                        for spec in specs {
                            let to_id = format!("IMPORT::{}", spec);
                            let edge_key = (file_node_id.clone(), to_id, RelationType::Imports);
                            if seen_edges.insert(edge_key.clone()) {
                                edges.push(Edge {
                                    from_node_id: edge_key.0,
                                    to_node_id: edge_key.1,
                                    relation_type: edge_key.2,
                                    _w: 1.0,
                                });
                            }
                        }
                    }
                    "import.require" => {
                        if let Some(spec) = Self::normalize_import_spec(&raw_text) {
                            let to_id = format!("IMPORT::{}", spec);
                            let edge_key = (file_node_id.clone(), to_id, RelationType::Imports);
                            if seen_edges.insert(edge_key.clone()) {
                                edges.push(Edge {
                                    from_node_id: edge_key.0,
                                    to_node_id: edge_key.1,
                                    relation_type: edge_key.2,
                                    _w: 1.0,
                                });
                            }
                        }
                    }
                    "call" => {
                        let Some(call_name) = Self::normalize_call_name(&node_text) else {
                            continue;
                        };
                        if call_name == "require" {
                            continue;
                        }
                        let node_id = format!("CALL::{}", call_name);
                        let mut from_id = file_node_id.clone();

                        let call_byte = capture.node.start_byte();
                        if let Some((_, _, owner_id)) = function_scopes
                            .iter()
                            .filter(|(s, e, _)| *s <= call_byte && call_byte <= *e)
                            .min_by_key(|(s, e, _)| e - s)
                        {
                            from_id = owner_id.clone();
                        }

                        let mut p = capture.node.parent();
                        while let Some(parent_node) = p {
                            if from_id != file_node_id {
                                break;
                            }
                            let pk = parent_node.kind();
                            if pk.contains("function") || pk.contains("method") || pk.contains("constructor") {
                                if let Some(name_node) = parent_node.child_by_field_name("name") {
                                    let name_text = name_node.utf8_text(content.as_bytes()).unwrap_or("");
                                    let sl = name_node.start_position().row + 1;
                                    let owner_name = Self::clean_name(name_text);
                                    if let Some(existing) = function_keys.get(&(owner_name.clone(), sl)) {
                                        from_id = existing.clone();
                                    } else {
                                        from_id = format!("FUNC::{}::{}::{}", file_path_str, owner_name, sl);
                                    }
                                    break;
                                }
                            }

                            if pk == "variable_declarator" {
                                if let Some(name_node) = parent_node.child_by_field_name("name") {
                                    let owner_name = Self::clean_name(name_node.utf8_text(content.as_bytes()).unwrap_or(""));
                                    let sl = name_node.start_position().row + 1;
                                    if let Some(existing) = function_keys.get(&(owner_name.clone(), sl)) {
                                        from_id = existing.clone();
                                        break;
                                    }
                                }
                            }

                            if pk == "assignment_expression" {
                                if let Some(left_node) = parent_node.child_by_field_name("left") {
                                    let owner_name = Self::clean_name(left_node.utf8_text(content.as_bytes()).unwrap_or(""));
                                    let sl = left_node.start_position().row + 1;
                                    if let Some(existing) = function_keys.get(&(owner_name.clone(), sl)) {
                                        from_id = existing.clone();
                                        break;
                                    }
                                }
                            }

                            p = parent_node.parent();
                        }

                        let edge_key = (from_id, node_id, RelationType::Calls);
                        if seen_edges.insert(edge_key.clone()) {
                            edges.push(Edge {
                                from_node_id: edge_key.0,
                                to_node_id: edge_key.1,
                                relation_type: edge_key.2,
                                _w: 1.0,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok((nodes, edges))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_js_arrow_functions_and_scoped_calls() {
        let parser = GenericParser::new(tree_sitter_javascript::LANGUAGE.into(), "javascript");
        let src = "const foo = () => { bar(); }; function bar() { return 1; }";
        let path = Path::new("/repo/app.js");

        let (nodes, edges) = parser.parse(path, src).expect("parse should succeed");

        assert!(nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "foo"));
        assert!(nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "bar"));

        let foo_id = nodes
            .iter()
            .find(|n| n.kind == NodeKind::Function && n.name == "foo")
            .map(|n| n.id.clone())
            .expect("foo function node must exist");

        assert!(edges.iter().any(|e| {
            e.relation_type == RelationType::Calls && e.from_node_id == foo_id && e.to_node_id == "CALL::bar"
        }));
    }

    #[test]
    fn extracts_python_async_functions() {
        let parser = GenericParser::new(tree_sitter_python::LANGUAGE.into(), "python");
        let src = "async def worker():\n    return 1\n";
        let path = Path::new("/repo/app.py");

        let (nodes, _) = parser.parse(path, src).expect("parse should succeed");
        assert!(nodes.iter().any(|n| n.kind == NodeKind::Function && n.name == "worker"));
    }
}
