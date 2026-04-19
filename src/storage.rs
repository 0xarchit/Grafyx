use crate::ir::{Graph, NodeKind, RelationType};
use rusqlite::{params, Connection};
use std::fs::File;
use std::path::Path;

pub struct Storage;

impl Storage {
    pub fn save_json(graph: &Graph, output_dir: &Path) {
        let file_path = output_dir.join("graph.json");
        if let Ok(file) = File::create(file_path) {
            let _ = serde_json::to_writer_pretty(file, graph);
        }
    }

    pub fn save_sqlite(graph: &Graph, output_dir: &Path) {
        let db_path = output_dir.join("graph.db");
        if let Ok(conn) = Connection::open(&db_path) {
            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS nodes (
                    id TEXT PRIMARY KEY,
                    kind TEXT,
                    name TEXT,
                    language TEXT,
                    file_path TEXT,
                    start_line INTEGER,
                    end_line INTEGER
                )",
                [],
            );

            let _ = conn.execute(
                "CREATE TABLE IF NOT EXISTS edges (
                    from_node_id TEXT,
                    to_node_id TEXT,
                    relation_type TEXT
                )",
                [],
            );

            for node in &graph.nodes {
                let kind_str = match node.kind {
                    NodeKind::File => "File",
                    NodeKind::Module => "Module",
                    NodeKind::Class => "Class",
                    NodeKind::Function => "Function",
                    NodeKind::Variable => "Variable",
                };
                let _ = conn.execute(
                    "INSERT OR REPLACE INTO nodes (id, kind, name, language, file_path, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        node.id.to_string(),
                        kind_str,
                        node.name,
                        node.language,
                        node.file_path,
                        node.start_line,
                        node.end_line
                    ],
                );
            }

            for edge in &graph.edges {
                let rel_str = match edge.relation_type {
                    RelationType::Imports => "Imports",
                    RelationType::Calls => "Calls",
                    RelationType::Defines => "Defines",
                    RelationType::Extends => "Extends",
                    RelationType::Implements => "Implements",
                    RelationType::Uses => "Uses",
                    RelationType::CallsService => "CallsService",
                    RelationType::ApiLink => "ApiLink",
                };
                let _ = conn.execute(
                    "INSERT INTO edges (from_node_id, to_node_id, relation_type) VALUES (?1, ?2, ?3)",
                    params![
                        edge.from_node_id.to_string(),
                        edge.to_node_id.to_string(),
                        rel_str
                    ],
                );
            }
        }
    }

    pub fn save_html(graph: &Graph, output_dir: &Path) {
        let file_path = output_dir.join("index.html");
        let template = include_str!("template.html");
        let json_data = match serde_json::to_string(graph) {
            Ok(v) => v,
            Err(_) => "{\"nodes\":[],\"edges\":[]}".to_string(),
        };
        let final_html = template.replace("{{GRAPH_DATA_PLACEHOLDER}}", &json_data);
        if let Ok(mut file) = File::create(file_path) {
            use std::io::Write;
            let _ = file.write_all(final_html.as_bytes());
        }
    }
}
