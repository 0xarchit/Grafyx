use super::CodeParser;
use crate::ir::{Edge, Node, NodeKind, RelationType};
use std::path::Path;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};
use streaming_iterator::StreamingIterator;
use anyhow::{Result, Context};

pub struct GenericParser {
    language: Language,
    query_str: &'static str,
    lang_name: &'static str,
}

impl GenericParser {
    pub fn new(language: Language, lang_name: &'static str) -> Self {
        Self {
            language,
            query_str: Self::default_query(lang_name),
            lang_name,
        }
    }

    fn default_query(lang: &str) -> &'static str {
        match lang {
            "javascript" | "typescript" => "[(import_statement) @import (call_expression function: (identifier) @call) (member_expression property: (property_identifier) @call)]",
            "python" => "[(import_statement) @import (import_from_statement) @import (call function: (identifier) @call) (call function: (attribute attribute: (identifier) @call))] ",
            "java" => "[(import_declaration) @import (method_invocation name: (identifier) @call)]",
            "go" => "[(import_declaration) @import (call_expression function: (identifier) @call) (call_expression function: (selector_expression field: (field_identifier) @call))]",
            "rust" => "[(mod_item name: (identifier) @import) (call_expression function: (identifier) @call) (call_expression function: (field_expression field: (field_identifier) @call)) (macro_invocation macro: (identifier) @call) (use_declaration) @import]",
            _ => "",
        }
    }

    fn clean_node_name(&self, text: &str) -> String {
        let text = text.trim();
        if text.is_empty() { return "".to_string(); }

        match self.lang_name {
            "python" => {
                let py_text = text.trim();
                // Handle "from x import y"
                if py_text.starts_with("from ") && py_text.contains(" import ") {
                    let parts: Vec<&str> = py_text.split(" import ").collect();
                    if parts.len() > 1 {
                        let module = parts[0].replace("from ", "").trim().to_string();
                        let item = parts[1].split(" as ").next().unwrap_or(parts[1]).trim().to_string();
                        return format!("{}.{}", module, item);
                    }
                }
                
                // Handle "import x as y"
                let clean = py_text.replace("import ", "")
                    .split(" as ")
                    .next()
                    .unwrap_or(py_text)
                    .trim()
                    .to_string();
                clean
            }
            "javascript" | "typescript" => {
                // Handle "import ... from 'path'"
                if text.contains(" from ") {
                    if let Some(path_start) = text.find('\'').or_else(|| text.find('\"')) {
                        let quote = text.as_bytes()[path_start];
                        let path_part = &text[path_start + 1..];
                        if let Some(path_end) = path_part.find(quote as char) {
                            let mod_path = &path_part[..path_end];
                            return mod_path.split('/').next_back().unwrap_or(mod_path).to_string();
                        }
                    }
                }
                // Handle require('path')
                if text.contains("require(") {
                    if let Some(path_start) = text.find('\'').or_else(|| text.find('\"')) {
                        let quote = text.as_bytes()[path_start];
                        let path_part = &text[path_start + 1..];
                        if let Some(path_end) = path_part.find(quote as char) {
                            let mod_path = &path_part[..path_end];
                            return mod_path.split('/').next_back().unwrap_or(mod_path).to_string();
                        }
                    }
                }
                text.to_string()
            }
            "rust" => {
                text.trim_start_matches("use ")
                    .trim_end_matches(';')
                    .trim()
                    .to_string()
            }
            "java" => {
                text.trim_start_matches("import ")
                    .trim_end_matches(';')
                    .trim()
                    .to_string()
            }
            "go" => {
                // Strip quotes and get the last part of the path
                let clean = text.trim_matches('"').trim_matches('\'');
                let parts: Vec<&str> = clean.split('/').collect();
                if parts.len() > 1 {
                    // Avoid returning just "internal" or some generic segment if possible
                    let last = parts.last().unwrap_or(&clean);
                    if (*last == "internal" || *last == "pkg") && parts.len() > 2 {
                        return format!("{}/{}", parts[parts.len() - 2], last);
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
        
        let query = match Query::new(&self.language, self.query_str) {
            Ok(q) => q,
            Err(e) => {
                tracing::debug!("Query compilation failed for {}: {}", self.lang_name, e);
                return Ok((nodes, edges));
            }
        };

        let mut cursor = QueryCursor::new();
        let mut matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
            
            while let Some(m) = matches.next() {
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

                        let start_line = capture.node.start_position().row;
                        let end_line = capture.node.end_position().row;
                        let call_id = format!("{}::{}::{}::L{}", self.lang_name, file_name, name, start_line);
                        nodes.push(Node {
                            id: call_id.clone(),
                            kind,
                            name: name.to_string(),
                            language: self.lang_name.to_string(),
                            file_path: file_name.clone(),
                            service: "".to_string(),
                            start_line,
                            end_line,
                        });
                        edges.push(Edge {
                            from_node_id: file_id.clone(),
                            to_node_id: call_id,
                            relation_type: relation,
                        });
                    }
                }
            }

        
        Ok((nodes, edges))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_python_parsing() {
        let parser = GenericParser::new(
            tree_sitter_python::LANGUAGE.into(),
            "python",
        );
        let content = "import os\nfrom math import sqrt\nsqrt(16)";
        let (nodes, edges) = parser.parse(&PathBuf::from("test.py"), content).expect("Parse failed");

        // Nodes: File node + os + math.sqrt + call to sqrt
        // Note: The query captures might create duplicates or specific IDs.
        assert!(nodes.iter().any(|n| n.name == "os" && n.kind == NodeKind::Module));
        assert!(nodes.iter().any(|n| n.name == "math.sqrt" && n.kind == NodeKind::Module));
        assert!(nodes.iter().any(|n| n.name == "sqrt" && n.kind == NodeKind::Function));
        assert!(edges.len() >= 3);
    }

    #[test]
    fn test_js_parsing() {
        let parser = GenericParser::new(
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
        );
        let content = "import { x } from './lib';\nconst r = require('fs');\nconsole.log(r);";
        let (nodes, _edges) = parser.parse(&PathBuf::from("test.js"), content).expect("Parse failed");

        assert!(nodes.iter().any(|n| n.name == "lib" && n.kind == NodeKind::Module));
        // Note: GenericParser might not handle require unless query is set up for it, 
        // but here we check clean_node_name logic too.
    }

    #[test]
    fn test_go_path_cleaning() {
        let parser = GenericParser::new(
            tree_sitter_go::LANGUAGE.into(),
            "go",
        );
        assert_eq!(parser.clean_node_name("\"github.com/user/project/pkg\""), "project/pkg");
        assert_eq!(parser.clean_node_name("\"github.com/user/project/internal\""), "project/internal");
    }

    #[test]
    fn test_rust_parsing() {
        let parser = GenericParser::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
        );
        let content = "use std::collections::HashMap;\nfn main() { let mut m = HashMap::new(); println!(\"test\"); }";
        let (nodes, _edges) = parser.parse(&PathBuf::from("main.rs"), content).expect("Parse failed");
        
        assert!(nodes.iter().any(|n| n.name == "std::collections::HashMap" && n.kind == NodeKind::Module));
        assert!(nodes.iter().any(|n| n.name == "println" && n.kind == NodeKind::Function));
    }

    #[test]
    fn test_java_parsing() {
        let parser = GenericParser::new(
            tree_sitter_java::LANGUAGE.into(),
            "java",
        );
        let content = "import java.util.List;\npublic class Test { public void run() { list.add(1); } }";
        let (nodes, _edges) = parser.parse(&PathBuf::from("Test.java"), content).expect("Parse failed");
        
        assert!(nodes.iter().any(|n| n.name == "java.util.List" && n.kind == NodeKind::Module));
        assert!(nodes.iter().any(|n| n.name == "add" && n.kind == NodeKind::Function));
    }

    #[test]
    fn test_ts_parsing() {
        let parser = GenericParser::new(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
        );
        let content = "import { x } from './lib';\nfunction test() { console.log(x); }";
        let (nodes, _edges) = parser.parse(&PathBuf::from("test.ts"), content).expect("Parse failed");
        
        assert!(nodes.iter().any(|n| n.name == "lib" && n.kind == NodeKind::Module));
        assert!(nodes.iter().any(|n| n.name == "log" && n.kind == NodeKind::Function));
    }

    #[test]
    fn test_empty_file() {
        let parser = GenericParser::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
        );
        let (nodes, edges) = parser.parse(&PathBuf::from("empty.rs"), "").expect("Parse failed");
        assert_eq!(nodes.len(), 1); // Only the file node
        assert_eq!(edges.len(), 0);
    }

    #[test]
    fn test_syntax_error() {
        let parser = GenericParser::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
        );
        let content = "fn main() { !!! }"; // Invalid rust
        let (nodes, edges) = parser.parse(&PathBuf::from("error.rs"), content).expect("Parse failed");
        // Tree-sitter is robust; it will still produce a tree (potentially with ERROR nodes)
        // Our queries should just return nothing or safe results.
        assert!(nodes.len() >= 1);
        assert_eq!(edges.len(), 0);
    }
}
