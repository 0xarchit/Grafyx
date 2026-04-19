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
                "[(import_statement) @import (call_expression function: (identifier) @call) (member_expression property: (property_identifier) @call)]",
                "javascript",
            );
            let ts_parser = GenericParser::new(
                tree_sitter_typescript::language_typescript(),
                "[(import_statement) @import (call_expression function: (identifier) @call) (member_expression property: (property_identifier) @call)]",
                "typescript",
            );
            let py_parser = GenericParser::new(
                tree_sitter_python::language(),
                "[(import_statement) @import (import_from_statement) @import (call function: (identifier) @call) (call function: (attribute attribute: (identifier) @call))] ",
                "python",
            );
            let java_parser = GenericParser::new(
                tree_sitter_java::language(),
                "[(import_declaration) @import (method_invocation name: (identifier) @call)]",
                "java",
            );
            let go_parser = GenericParser::new(
                tree_sitter_go::language(),
                "[(import_declaration) @import (call_expression function: (identifier) @call) (call_expression function: (selector_expression field: (field_identifier) @call))]",
                "go",
            );
            let rust_parser = GenericParser::new(
                tree_sitter_rust::language(),
                "[(mod_item name: (identifier) @import) (call_expression function: (identifier) @call) (call_expression function: (field_expression field: (field_identifier) @call)) (macro_invocation macro: (identifier) @call) (use_declaration argument: (scoped_identifier name: (identifier) @import))]",
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
