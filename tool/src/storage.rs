use crate::ir::{Graph, Node, Edge, NodeKind, RelationType};
use rusqlite::{params, Connection};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use anyhow::{Result, Context};

pub struct Storage;

impl Storage {
    pub fn save_json(graph: &Graph, output_dir: &Path) -> Result<()> {
        let file_path = output_dir.join("grafyx.json");
        let file = File::create(&file_path)
            .with_context(|| format!("Failed to create JSON file at {:?}", file_path))?;
        let writer = BufWriter::new(file);
        serde_json::to_writer_pretty(writer, graph)
            .with_context(|| "Failed to serialize graph to JSON")?;
        Ok(())
    }

    pub fn open_db(output_dir: &Path) -> Result<Connection> {
        let db_path = output_dir.join("grafyx.db");
        let conn = Connection::open(db_path).context("Failed to open SQLite database")?;
        
        // Performance & Concurrency tuning
        conn.execute("PRAGMA journal_mode=WAL", []).ok();
        conn.execute("PRAGMA busy_timeout=5000", []).ok();
        
        Ok(conn)
    }

    pub fn init_db(conn: &Connection) -> Result<()> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS files (
                path TEXT PRIMARY KEY,
                last_modified INTEGER,
                size INTEGER
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                kind TEXT,
                name TEXT,
                language TEXT,
                file_path TEXT,
                service TEXT,
                start_line INTEGER,
                end_line INTEGER
            )",
            [],
        )?;

        // Simple migration to add service column if it doesn't exist
        let _ = conn.execute("ALTER TABLE nodes ADD COLUMN service TEXT", []);

        conn.execute(
            "CREATE TABLE IF NOT EXISTS edges (
                from_node_id TEXT,
                to_node_id TEXT,
                relation_type TEXT,
                PRIMARY KEY (from_node_id, to_node_id, relation_type)
            )",
            [],
        )?;
        
        conn.execute("CREATE INDEX IF NOT EXISTS idx_nodes_file_path ON nodes(file_path)", [])?;
        conn.execute("CREATE INDEX IF NOT EXISTS idx_edges_from ON edges(from_node_id)", [])?;
        
        Ok(())
    }

    pub fn get_file_mtime(conn: &Connection, path: &str) -> Option<u64> {
        conn.query_row(
            "SELECT last_modified FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        ).ok()
    }

    pub fn load_file_data(conn: &Connection, path: &str) -> Result<(Vec<Node>, Vec<Edge>)> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let mut stmt = conn.prepare("SELECT id, kind, name, language, file_path, start_line, end_line, service FROM nodes WHERE file_path = ?1")?;
        let node_iter = stmt.query_map(params![path], |row| {
            Ok(Node {
                id: row.get(0)?,
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
                service: row.get(7)?,
            })
        })?;

        for node in node_iter {
            nodes.push(node?);
        }

        // Efficiently load edges for all nodes in this file using a single query
        if !nodes.is_empty() {
            let mut stmt = conn.prepare("
                SELECT from_node_id, to_node_id, relation_type 
                FROM edges 
                WHERE from_node_id IN (SELECT id FROM nodes WHERE file_path = ?1)
            ")?;
            
            let edge_iter = stmt.query_map(params![path], |row| {
                Ok(Edge {
                    from_node_id: row.get(0)?,
                    to_node_id: row.get(1)?,
                    relation_type: match row.get::<_, String>(2)?.as_str() {
                        "RootLink" => RelationType::RootLink,
                        "ServiceCall" => RelationType::ServiceCall,
                        "Imports" => RelationType::Imports,
                        "Calls" => RelationType::Calls,
                        "Defines" => RelationType::Defines,
                        "Extends" => RelationType::Extends,
                        "Implements" => RelationType::Implements,
                        "Uses" => RelationType::Uses,
                        "ApiLink" => RelationType::ApiLink,
                        _ => RelationType::Uses,
                    },
                })
            })?;

            for edge in edge_iter {
                edges.push(edge?);
            }
        }

        Ok((nodes, edges))
    }

    pub fn update_file_metadata(conn: &Connection, path: &str, mtime: u64, size: u64) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO files (path, last_modified, size) VALUES (?1, ?2, ?3)",
            params![path, mtime as i64, size as i64],
        )?;
        Ok(())
    }

    pub fn save_sqlite(graph: &Graph, output_dir: &Path) -> Result<()> {
        let conn = Self::open_db(output_dir)?;
        Self::init_db(&conn)?;
        
        let mut conn = conn;
        let tx = conn.transaction()?;
        {
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
                tx.execute(
                    "INSERT OR REPLACE INTO nodes (id, kind, name, language, file_path, service, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                    params![
                        node.id,
                        kind_str,
                        node.name,
                        node.language,
                        node.file_path,
                        node.service,
                        node.start_line,
                        node.end_line
                    ],
                )?;
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
                tx.execute(
                    "INSERT OR REPLACE INTO edges (from_node_id, to_node_id, relation_type) VALUES (?1, ?2, ?3)",
                    params![
                        edge.from_node_id,
                        edge.to_node_id,
                        rel_str
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn save_html(graph: &Graph, output_dir: &Path) -> Result<()> {
        let file_path = output_dir.join("index.html");
        let template = include_str!("template.html");
        let json_data = serde_json::to_string(graph).context("Failed to serialize graph for HTML")?;
        let final_html = template.replace("{{GRAPH_DATA_PLACEHOLDER}}", &json_data);
        let mut file = File::create(&file_path).with_context(|| format!("Failed to create HTML file at {:?}", file_path))?;
        use std::io::Write;
        file.write_all(final_html.as_bytes()).context("Failed to write HTML content")?;
        Ok(())
    }
}

