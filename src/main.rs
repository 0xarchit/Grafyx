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
use parser::{generic::GenericParser, CodeParser};
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
            
            let js_parser = GenericParser::new(
                tree_sitter_javascript::language(),
                "(call_expression function: (identifier) @call_name)",
                "javascript",
            );
            let ts_parser = GenericParser::new(
                tree_sitter_typescript::language_typescript(),
                "(call_expression function: (identifier) @call_name)",
                "typescript",
            );
            let py_parser = GenericParser::new(
                tree_sitter_python::language(),
                "(call function: (identifier) @call_name)",
                "python",
            );
            let java_parser = GenericParser::new(
                tree_sitter_java::language(),
                "(method_invocation name: (identifier) @call_name)",
                "java",
            );
            let go_parser = GenericParser::new(
                tree_sitter_go::language(),
                "(call_expression function: (identifier) @call_name)",
                "go",
            );
            let rust_parser = GenericParser::new(
                tree_sitter_rust::language(),
                "[(mod_item name: (identifier) @n) (call_expression function: (identifier) @n) (call_expression function: (field_expression field: (field_identifier) @n)) (macro_invocation macro: (identifier) @n) (use_declaration argument: (scoped_identifier name: (identifier) @n))]",
                "rust",
            );

            for (file_path, lang) in files {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    let (nodes, edges) = match lang {
                        Language::JavaScript => js_parser.parse(&file_path, &content),
                        Language::Python => py_parser.parse(&file_path, &content),
                        Language::Go => go_parser.parse(&file_path, &content),
                        Language::Rust => rust_parser.parse(&file_path, &content),
                        Language::Java => java_parser.parse(&file_path, &content),
                        Language::Unknown => {
                            if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                                if ext == "ts" || ext == "tsx" {
                                    ts_parser.parse(&file_path, &content)
                                } else {
                                    (Vec::new(), Vec::new())
                                }
                            } else {
                                (Vec::new(), Vec::new())
                            }
                        }
                    };
                    graph.nodes.extend(nodes);
                    graph.edges.extend(edges);
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
            Storage::save_html(&graph, out_path);
        }
    }
}
