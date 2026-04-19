use super::CodeParser;
use crate::ir::{Edge, Node, NodeKind, RelationType};
use std::path::Path;
use tree_sitter::{Parser as TSParser, Query, QueryCursor};
use uuid::Uuid;

pub struct JsParser;

impl JsParser {
    pub fn new() -> Self {
        Self
    }
}

impl CodeParser for JsParser {
    fn parse(&self, file_path: &Path, content: &str) -> (Vec<Node>, Vec<Edge>) {
        let mut parser = TSParser::new();
        parser.set_language(&tree_sitter_javascript::language()).expect("Language set error");
        
        let mut nodes = Vec::new();
        let mut edges = Vec::new();
        
        let tree = match parser.parse(content, None) {
            Some(t) => t,
            None => return (nodes, edges),
        };
        
        let file_id = Uuid::new_v4();
        let file_name = file_path.to_string_lossy().to_string();
        
        nodes.push(Node {
            id: file_id,
            kind: NodeKind::File,
            name: file_name.clone(),
            language: "javascript".to_string(),
            file_path: file_name.clone(),
            start_line: 0,
            end_line: content.lines().count(),
        });
        
        let query_str = "(call_expression function: (identifier) @call_name)";
        if let Ok(query) = Query::new(&tree_sitter_javascript::language(), query_str) {
            let mut cursor = QueryCursor::new();
            let matches = cursor.matches(&query, tree.root_node(), content.as_bytes());
            
            for m in matches {
                for capture in m.captures {
                    if let Ok(call_name) = capture.node.utf8_text(content.as_bytes()) {
                        let call_id = Uuid::new_v4();
                        nodes.push(Node {
                            id: call_id,
                            kind: NodeKind::Function,
                            name: call_name.to_string(),
                            language: "javascript".to_string(),
                            file_path: file_name.clone(),
                            start_line: capture.node.start_position().row,
                            end_line: capture.node.end_position().row,
                        });
                        edges.push(Edge {
                            from_node_id: file_id,
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
