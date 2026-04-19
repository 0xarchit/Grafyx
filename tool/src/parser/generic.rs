use super::CodeParser;
use crate::ir::{Edge, Node, NodeKind, RelationType};
use std::path::Path;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};
use anyhow::{Result, Context};

pub struct GenericParser {
    language: Language,
    query_str: &'static str,
    lang_name: &'static str,
}

impl GenericParser {
    pub fn new(language: Language, query_str: &'static str, lang_name: &'static str) -> Self {
        Self {
            language,
            query_str,
            lang_name,
        }
    }

    fn clean_node_name(&self, text: &str) -> String {
        let text = text.trim();
        if text.is_empty() { return "".to_string(); }

        match self.lang_name {
            "python" => {
                // Strip "import ", "from ", " as ..."
                let clean = text.replace("import ", "")
                    .replace("from ", "")
                    .split(" as ")
                    .next()
                    .unwrap_or(text)
                    .trim()
                    .to_string();
                
                // If it contains " import ", it's a "from x import y"
                if clean.contains(" import ") {
                    let parts: Vec<&str> = clean.split(" import ").collect();
                    if parts.len() > 1 {
                        return format!("{}.{}", parts[0].trim(), parts[1].trim());
                    }
                }
                clean
            }
            "javascript" | "typescript" => {
                // Handle "import ... from 'path'"
                if text.contains(" from ") {
                    if let Some(path_start) = text.find('\'').or_else(|| text.find('\"')) {
                        let path = &text[path_start + 1..];
                        if let Some(path_end) = path.find('\'').or_else(|| path.find('\"')) {
                            let mod_path = &path[..path_end];
                            return mod_path.split('/').last().unwrap_or(mod_path).to_string();
                        }
                    }
                }
                // Handle require('path')
                if text.contains("require(") {
                    if let Some(path_start) = text.find('\'').or_else(|| text.find('\"')) {
                        let path = &text[path_start + 1..];
                        if let Some(path_end) = path.find('\'').or_else(|| path.find('\"')) {
                            let mod_path = &path[..path_end];
                            return mod_path.split('/').last().unwrap_or(mod_path).to_string();
                        }
                    }
                }
                text.to_string()
            }
            "go" => {
                // Strip quotes and get the last part of the path
                let clean = text.trim_matches('"').trim_matches('\'');
                let parts: Vec<&str> = clean.split('/').collect();
                if parts.len() > 1 {
                    // Avoid returning just "internal" or some generic segment if possible
                    let last = parts.last().unwrap_or(&clean);
                    if *last == "internal" || *last == "pkg" {
                        if parts.len() > 2 {
                             return format!("{}/{}", parts[parts.len()-2], last);
                        }
                    }
                    last.to_string()
                } else {
                    clean.to_string()
                }
            }
            _ => text.to_string()
        }
    }
}


impl CodeParser for GenericParser {
    fn parse(&self, file_path: &Path, content: &str) -> Result<(Vec<Node>, Vec<Edge>)> {
        let mut parser = TSParser::new();
        parser.set_language(&self.language).context("Failed to set tree-sitter language")?;
        
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        
        let tree = parser.parse(content, None)
            .context("Tree-sitter failed to parse content")?;
        
        let file_name = file_path.to_string_lossy().to_string();
        let file_id = format!("{}::{}::FILE", self.lang_name, file_name);
        
        let end_line = tree.root_node().end_position().row;

        nodes.push(Node {
            id: file_id.clone(),
            kind: NodeKind::File,
            name: file_name.clone(),
            language: self.lang_name.to_string(),
            file_path: file_name.clone(),
            service: "".to_string(),
            start_line: 0,
            end_line,
        });
        
        if let Ok(query) = Query::new(&self.language, self.query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
            
            for m in matches {
                for capture in m.captures {
                    if let Ok(text) = capture.node.utf8_text(content.as_bytes()) {
                        let name = self.clean_node_name(text);
                        if name.is_empty() { continue; }
                        
                        let tag = query.capture_names()[capture.index as usize];
                        
                        let (kind, relation) = if tag == "import" {
                            (NodeKind::Module, RelationType::Imports)
                        } else {
                            (NodeKind::Function, RelationType::Calls)
                        };

                        let call_id = format!("{}::{}::{}", self.lang_name, file_name, name);
                        nodes.push(Node {
                            id: call_id.clone(),
                            kind,
                            name: name.to_string(),
                            language: self.lang_name.to_string(),
                            file_path: file_name.clone(),
                            service: "".to_string(),
                            start_line: capture.node.start_position().row,
                            end_line: capture.node.end_position().row,
                        });
                        edges.push(Edge {
                            from_node_id: file_id.clone(),
                            to_node_id: call_id,
                            relation_type: relation,
                        });
                    }
                }
            }
        }
        
        Ok((nodes, edges))
    }
}
