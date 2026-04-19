pub mod cli;
pub mod ir;
pub mod linker;
pub mod parser;
pub mod scanner;
pub mod storage;

use clap::Parser;
use cli::{Cli, Commands};
use ir::Graph;
use linker::Linker;
use parser::{js::JsParser, CodeParser};
use scanner::{Language, Scanner};
use std::fs;
use std::path::Path;
use storage::Storage;

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Scan { dirs, merge, ignore, format, output } => {
            let scanner = Scanner::new(dirs.clone(), ignore.clone());
            let files = scanner.scan();
            let mut graph = Graph::new();
            let js_parser = JsParser::new();

            for (file_path, lang) in files {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    if lang == Language::JavaScript {
                        let (nodes, edges) = js_parser.parse(&file_path, &content);
                        graph.nodes.extend(nodes);
                        graph.edges.extend(edges);
                    }
                }
            }

            let linker = Linker::new();
            linker.link(&mut graph);

            let out_path = Path::new(output);
            if !out_path.exists() {
                let _ = fs::create_dir_all(out_path);
            }

            if format == "json" || format == "both" {
                Storage::save_json(&graph, out_path);
            }
            if format == "sqlite" || format == "both" {
                Storage::save_sqlite(&graph, out_path);
            }
        }
    }
}
