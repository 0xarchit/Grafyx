use crate::ir::{Graph, Node, Edge, NodeKind, RelationType};
use rusqlite::{params, Connection};
use std::fs::File;
use std::path::Path;

pub struct Storage;

impl Storage {
    pub fn save_json(graph: &Graph, output_dir: &Path) {
        let file_path = output_dir.join("grafyx.json");
        if let Ok(file) = File::create(file_path) {
            let _ = serde_json::to_writer_pretty(file, graph);
        }
    }

    pub fn open_db(output_dir: &Path) -> Option<Connection> {
        let db_path = output_dir.join("grafyx.db");
        Connection::open(db_path).ok()
    }

    pub fn init_db(conn: &Connection) {
        let _ = conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                last_modified INTEGER,
                size INTEGER
            )",
            [],
        );

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
    }

    pub fn get_file_mtime(conn: &Connection, path: &str) -> Option<u64> {
        conn.query_row(
            "SELECT last_modified FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        ).ok()
    }

    pub fn load_file_data(conn: &Connection, path: &str) -> (Vec<Node>, Vec<Edge>) {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        if let Ok(mut stmt) = conn.prepare("SELECT id, kind, name, language, file_path, start_line, end_line FROM nodes WHERE file_path = ?1") {
            let node_iter = stmt.query_map(params![path], |row| {
                Ok(Node {
                    id: row.get::<_, String>(0)?.into(),
                    kind: match row.get::<_, String>(1)?.as_str() {
                        "Root" => NodeKind::Root,
                        "Service" => NodeKind::Service,
                        "File" => NodeKind::File,
                        "Module" => NodeKind::Module,
                        "Class" => NodeKind::Class,
                        "Function" => NodeKind::Function,
                        _ => NodeKind::Variable,
                    },
                    name: row.get(2)?,
                    language: row.get(3)?,
                    file_path: row.get(4)?,
                    start_line: row.get(5)?,
                    end_line: row.get(6)?,
                    service: String::new(), // Populated by linker
                })
            }).ok();

            if let Some(iter) = node_iter {
                for n in iter.flatten() {
                    nodes.push(n);
                }
            }
        }

        // Edges related to these nodes
        // Note: This is simpler if we just load edges where from_node_id is in nodes list
        for node in &nodes {
            if let Ok(mut stmt) = conn.prepare("SELECT from_node_id, to_node_id, relation_type FROM edges WHERE from_node_id = ?1") {
                let edge_iter = stmt.query_map(params![node.id.to_string()], |row| {
                    Ok(Edge {
                        from_node_id: row.get::<_, String>(0)?.into(),
                        to_node_id: row.get::<_, String>(1)?.into(),
                        relation_type: match row.get::<_, String>(2)?.as_str() {
                            "RootLink" => RelationType::RootLink,
                            "ServiceCall" => RelationType::ServiceCall,
                            "Imports" => RelationType::Imports,
                            "Calls" => RelationType::Calls,
                            _ => RelationType::Uses,
                        },
                    })
                }).ok();
                if let Some(iter) = edge_iter {
                    for e in iter.flatten() {
                        edges.push(e);
                    }
                }
            }
        }

        (nodes, edges)
    }

    pub fn update_file_metadata(conn: &Connection, path: &str, mtime: u64, size: u64) {
        let _ = conn.execute(
            "INSERT OR REPLACE INTO files (path, last_modified, size) VALUES (?1, ?2, ?3)",
            params![path, mtime as i64, size as i64],
        );
    }

    pub fn save_sqlite(graph: &Graph, output_dir: &Path) {
        if let Some(conn) = Self::open_db(output_dir) {
            Self::init_db(&conn);
            
            let tx = conn.unchecked_transaction().ok();
            if let Some(tx) = tx {
                for node in &graph.nodes {
                    let kind_str = match node.kind {
                        NodeKind::Root => "Root",
                        NodeKind::Service => "Service",
                        NodeKind::File => "File",
                        NodeKind::Module => "Module",
                        NodeKind::Class => "Class",
                        NodeKind::Function => "Function",
                        NodeKind::Variable => "Variable",
                    };
                    let _ = tx.execute(
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
                        RelationType::RootLink => "RootLink",
                        RelationType::ServiceCall => "ServiceCall",
                        RelationType::Imports => "Imports",
                        RelationType::Calls => "Calls",
                        RelationType::Defines => "Defines",
                        RelationType::Extends => "Extends",
                        RelationType::Implements => "Implements",
                        RelationType::Uses => "Uses",
                        RelationType::ApiLink => "ApiLink",
                    };
                    let _ = tx.execute(
                        "INSERT INTO edges (from_node_id, to_node_id, relation_type) VALUES (?1, ?2, ?3)",
                        params![
                            edge.from_node_id.to_string(),
                            edge.to_node_id.to_string(),
                            rel_str
                        ],
                    );
                }
                let _ = tx.commit();
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

