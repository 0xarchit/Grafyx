use crate::ir::{Graph, Node, Edge, NodeKind, RelationType};
use rusqlite::{params, Connection};
use std::collections::HashSet;
use std::fs;
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
                hash TEXT
            )",
            [],
        )?;

        // Migration: ensure files table has hash column and remove old ones if they exist
        // Note: For simplicity in this upgrade, we just ensure the column exists.
        let _ = conn.execute("ALTER TABLE files ADD COLUMN hash TEXT", []);

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

    pub fn get_file_hash(conn: &Connection, path: &str) -> Option<String> {
        conn.query_row(
            "SELECT hash FROM files WHERE path = ?1",
            params![path],
            |row| row.get(0),
        ).ok()
    }

    pub fn load_file_data(conn: &Connection, path: &str) -> Result<(Vec<Node>, Vec<Edge>)> {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        let mut stmt = conn.prepare("SELECT id, kind, name, language, file_path, start_line, end_line, service FROM nodes WHERE file_path = ?1")?;
        let node_iter = stmt.query_map(params![path], |row| {
            let kind_str: String = row.get(1)?;
            let kind = std::str::FromStr::from_str(&kind_str).unwrap_or_else(|_| {
                tracing::warn!("Unknown NodeKind in DB: {}. Defaulting to Variable", kind_str);
                NodeKind::Variable
            });
            Ok(Node {
                id: row.get(0)?,
                kind,
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
        // Note: Only loads outgoing edges (from_node_id matches) for incremental caching.
        // Incoming edges are loaded when their source files are processed.
        if !nodes.is_empty() {
            let mut stmt = conn.prepare("
                SELECT from_node_id, to_node_id, relation_type 
                FROM edges 
                WHERE from_node_id IN (SELECT id FROM nodes WHERE file_path = ?1)
            ")?;
            
            let edge_iter = stmt.query_map(params![path], |row| {
                let rel_str: String = row.get(2)?;
                let relation_type = std::str::FromStr::from_str(&rel_str).unwrap_or_else(|_| {
                    tracing::warn!("Unknown RelationType in DB: {}. Defaulting to Uses", rel_str);
                    RelationType::Uses
                });
                Ok(Edge {
                    from_node_id: row.get(0)?,
                    to_node_id: row.get(1)?,
                    relation_type,
                })
            })?;

            for edge in edge_iter {
                edges.push(edge?);
            }
        }

        Ok((nodes, edges))
    }

    pub fn update_file_hash(conn: &Connection, path: &str, hash: &str) -> Result<()> {
        conn.execute(
            "INSERT OR REPLACE INTO files (path, hash) VALUES (?1, ?2)",
            params![path, hash],
        )?;
        Ok(())
    }

    pub fn save_sqlite(graph: &Graph, output_dir: &Path) -> Result<()> {
        let conn = Self::open_db(output_dir)?;
        Self::init_db(&conn)?;
        
        let mut conn = conn;
        let tx = conn.transaction()?;
        {
            // Clean up stale data for the files present in the current graph
            let file_paths: HashSet<&String> = graph.nodes.iter()
                .filter(|n| !n.file_path.is_empty())
                .map(|n| &n.file_path)
                .collect();

            for path in file_paths {
                // Delete edges where either end is a node in this file
                tx.execute(
                    "DELETE FROM edges WHERE from_node_id IN (SELECT id FROM nodes WHERE file_path = ?1) 
                     OR to_node_id IN (SELECT id FROM nodes WHERE file_path = ?1)",
                    params![path],
                )?;
                // Delete the nodes themselves
                tx.execute("DELETE FROM nodes WHERE file_path = ?1", params![path])?;
            }

            for node in &graph.nodes {
                let kind_str = node.kind.to_string();
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
                let rel_str = edge.relation_type.to_string();
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
        let safe_json = json_data.replace("</script>", "<\\/script>");
        
        let data_script = format!("<script id=\"graph-data\" type=\"application/json\">{}</script>", safe_json);
        let final_html = template.replace("{{ GRAPH_DATA_PLACEHOLDER }}", &data_script);
        
        fs::write(&file_path, final_html).with_context(|| format!("Failed to write HTML report to {:?}", file_path))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Node, NodeKind};

    #[test]
    fn test_storage_sqlite_roundtrip() {
        let conn = Connection::open_in_memory().unwrap();
        Storage::init_db(&conn).unwrap();

        let node = Node {
            id: "node1".to_string(),
            kind: NodeKind::Function,
            name: "test_func".to_string(),
            language: "rust".to_string(),
            file_path: "src/main.rs".to_string(),
            service: "core".to_string(),
            start_line: 10,
            end_line: 20,
        };

        // Test manual insertion and load to verify logic
        let kind_str = node.kind.to_string();
        conn.execute(
            "INSERT INTO nodes (id, kind, name, language, file_path, service, start_line, end_line) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
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
        ).unwrap();

        let (nodes, _edges) = Storage::load_file_data(&conn, "src/main.rs").unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].name, "test_func");
        assert_eq!(nodes[0].kind, NodeKind::Function);
    }

    #[test]
    fn test_sqlite_concurrency() {
        use std::sync::Arc;
        use std::thread;
        let temp_dir = tempfile::tempdir().unwrap();
        let db_path = Arc::new(temp_dir.path().to_path_buf());
        
        // Init DB
        {
            let conn = Storage::open_db(&db_path).unwrap();
            Storage::init_db(&conn).unwrap();
        }

        let mut handles = Vec::new();
        for i in 0..5 {
            let path = Arc::clone(&db_path);
            handles.push(thread::spawn(move || {
                let _conn = Storage::open_db(&path).unwrap();
                for j in 0..50 {
                    let node = Node {
                        id: format!("node_{}_{}", i, j),
                        kind: NodeKind::Function,
                        name: format!("func_{}", j),
                        language: "rust".to_string(),
                        file_path: format!("file_{}_{}.rs", i, j),
                        service: "test".to_string(),
                        start_line: 0,
                        end_line: 0,
                    };
                    let graph = Graph { nodes: vec![node], edges: vec![] };
                    Storage::save_sqlite(&graph, &path).unwrap();
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let conn = Storage::open_db(&db_path).unwrap();
        let mut stmt = conn.prepare("SELECT COUNT(*) FROM nodes").unwrap();
        let count: i32 = stmt.query_row([], |r| r.get(0)).unwrap();
        assert_eq!(count, 250);
    }
}

