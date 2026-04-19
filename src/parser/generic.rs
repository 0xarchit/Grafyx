use super::CodeParser;
use crate::ir::{Edge, Node, NodeKind, RelationType};
use std::path::Path;
use tree_sitter::{Language, Parser as TSParser, Query, QueryCursor};

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
}

impl CodeParser for GenericParser {
    fn parse(&self, file_path: &Path, content: &str) -> (Vec<Node>, Vec<Edge>) {
        let mut parser = TSParser::new();
        let _ = parser.set_language(&self.language);
        
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        
        let tree = match parser.parse(content, None) {
            Some(t) => t,
            None => return (nodes, edges),
        };
        
        let file_name = file_path.to_string_lossy().to_string();
        let file_id = format!("{}::{}::FILE", self.lang_name, file_name);
        
        nodes.push(Node {
            id: file_id.clone(),
            kind: NodeKind::File,
            name: file_name.clone(),
            language: self.lang_name.to_string(),
            file_path: file_name.clone(),
            start_line: 0,
            end_line: content.lines().count(),
        });
        
        if let Ok(query) = Query::new(&self.language, self.query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
            
            for m in matches {
                for capture in m.captures {
                    if let Ok(call_name) = capture.node.utf8_text(content.as_bytes()) {
                        let call_id = format!("{}::{}::{}", self.lang_name, file_name, call_name);
                        nodes.push(Node {
                            id: call_id.clone(),
                            kind: NodeKind::Function,
                            name: call_name.to_string(),
                            language: self.lang_name.to_string(),
                            file_path: file_name.clone(),
                            start_line: capture.node.start_position().row,
                            end_line: capture.node.end_position().row,
                        });
                        edges.push(Edge {
                            from_node_id: file_id.clone(),
                            to_node_id: call_id,
                            relation_type: RelationType::Calls,
                        });
                    }
                }
            }
        }
        
        (nodes, edges)
    }
}
