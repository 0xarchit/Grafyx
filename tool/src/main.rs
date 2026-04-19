pub mod cli;
pub mod ir;
pub mod linker;
pub mod parser;
pub mod scanner;
pub mod storage;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use ir::{Graph, Node, Edge};
use linker::Linker;
use parser::{generic::GenericParser, CodeParser};
use rayon::prelude::*;
use scanner::{Language, Scanner};
use std::fs;
use std::path::Path;
use storage::Storage;
use tracing::info;
use tracing_subscriber;

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

fn handle_install() -> Result<()> {
    let home_dir = home::home_dir().context("Could not find home directory")?;
    let current_exe = std::env::current_exe().context("Could not get current executable path")?;
    
    #[cfg(windows)]
    {
        let install_dir = home_dir.join("AppData").join("Local").join("grafyx").join("bin");
        fs::create_dir_all(&install_dir).context("Failed to create installation directory")?;
        let dest = install_dir.join("grafyx.exe");
        
        fs::copy(&current_exe, &dest).context("Failed to copy binary")?;
        
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(env) = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE) {
            let current_path: String = env.get_value("Path").unwrap_or_default();
            let install_path_str = install_dir.to_string_lossy().to_string();
            
            if !current_path.split(';').any(|p| p == install_path_str) {
                let updated_path = format!("{};{}", current_path, install_path_str);
                env.set_value("Path", &updated_path).context("Failed to update PATH")?;
                println!("Installation complete! Grafyx has been added to your PATH.");
            } else {
                println!("Grafyx is already in your PATH.");
            }
        }
    }

    #[cfg(not(windows))]
    {
        let install_dir = home_dir.join(".local").join("bin");
        fs::create_dir_all(&install_dir).context("Failed to create installation directory")?;
        let dest = install_dir.join("grafyx");
        
        fs::copy(&current_exe, &dest).context("Failed to copy binary")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest)?.permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dest, perms)?;
        }
        
        let shells = [".bashrc", ".zshrc", ".profile"];
        let path_line = format!("\nexport PATH=\"$PATH:{}\"\n", install_dir.display());

        for shell in shells {
            let shell_path = home_dir.join(shell);
            if shell_path.exists() {
                if let Ok(content) = fs::read_to_string(&shell_path) {
                    if !content.contains(&install_dir.to_string_lossy().to_string()) {
                        use std::io::Write;
                        if let Ok(mut file) = fs::OpenOptions::new().append(true).open(&shell_path) {
                            writeln!(file, "{}", path_line).ok();
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn handle_upgrade() -> Result<()> {
    println!("Checking for updates...");
    let status = self_update::backends::github::Update::configure()
        .repo_owner("0xarchit")
        .repo_name("grafyx")
        .bin_name("grafyx")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()
        .context("Failed to configure update")?
        .update()
        .context("Failed to perform update")?;

    if status.updated() {
        println!("Updated to version {}!", status.version());
    } else {
        println!("Already up to date!");
    }
    Ok(())
}

fn resolve_parser(lang: &Language) -> Option<GenericParser> {
    match lang {
        Language::JavaScript => Some(GenericParser::new(
            tree_sitter_javascript::language(),
            "[(import_statement) @import (call_expression function: (identifier) @call) (member_expression property: (property_identifier) @call)]",
            "javascript",
        )),
        Language::TypeScript => Some(GenericParser::new(
            tree_sitter_typescript::language_typescript(),
            "[(import_statement) @import (call_expression function: (identifier) @call) (member_expression property: (property_identifier) @call)]",
            "typescript",
        )),
        Language::Python => Some(GenericParser::new(
            tree_sitter_python::language(),
            "[(import_statement) @import (import_from_statement) @import (call function: (identifier) @call) (call function: (attribute attribute: (identifier) @call))] ",
            "python",
        )),
        Language::Java => Some(GenericParser::new(
            tree_sitter_java::language(),
            "[(import_declaration) @import (method_invocation name: (identifier) @call)]",
            "java",
        )),
        Language::Go => Some(GenericParser::new(
            tree_sitter_go::language(),
            "[(import_declaration) @import (call_expression function: (identifier) @call) (call_expression function: (selector_expression field: (field_identifier) @call))]",
            "go",
        )),
        Language::Rust => Some(GenericParser::new(
            tree_sitter_rust::language(),
            "[(mod_item name: (identifier) @import) (call_expression function: (identifier) @call) (call_expression function: (field_expression field: (field_identifier) @call)) (macro_invocation macro: (identifier) @call) (use_declaration argument: (scoped_identifier name: (identifier) @import))]",
            "rust",
        )),
        Language::Unknown => None,
    }
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();

    match &cli.command {
        Commands::Scan { dirs, ignore, format, output } => {
            let out_path = Path::new(output);
            if !out_path.exists() {
                fs::create_dir_all(out_path).context("Failed to create output directory")?;
            }

            let scanner = Scanner::new(dirs.clone(), ignore.clone());
            let mut files = scanner.scan();
            
            // Ensure deterministic ordering across runs
            files.sort_by(|a, b| a.0.cmp(&b.0));
            
            let total_files = files.len();

            let results: Vec<(Vec<Node>, Vec<Edge>, bool)> = files.into_par_iter().map(|(file_path, lang)| {
                let conn = Storage::open_db(out_path).ok();
                
                let mut mtime = 0;
                let mut size = 0;
                if let Ok(metadata) = fs::metadata(&file_path) {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                            mtime = duration.as_secs();
                        }
                    }
                    size = metadata.len();
                }

                if let Some(ref c) = conn {
                    if let Some(old_mtime) = Storage::get_file_mtime(c, &file_path.to_string_lossy()) {
                        if old_mtime == mtime {
                            if let Ok(data) = Storage::load_file_data(c, &file_path.to_string_lossy()) {
                                return (data.0, data.1, true);
                            }
                        }
                    }
                }

                if let Some(parser) = resolve_parser(&lang) {
                    if let Ok(content) = fs::read_to_string(&file_path) {
                        if let Ok((nodes, edges)) = parser.parse(&file_path, &content) {
                             if let Some(ref c) = conn {
                                 let _ = Storage::update_file_metadata(c, &file_path.to_string_lossy(), mtime, size);
                             }
                             return (nodes, edges, false);
                        }
                    }
                }

                (Vec::new(), Vec::new(), false)
            }).collect();

            let mut graph = Graph::new();
            let mut skipped_count = 0;
            for (nodes, edges, skipped) in results {
                graph.nodes.extend(nodes);
                graph.edges.extend(edges);
                if skipped { skipped_count += 1; }
            }

            info!("Scan complete. Total files: {}, Cached: {}", total_files, skipped_count);

            let linker = Linker::new(dirs.clone());
            linker.link(&mut graph);

            if format == "json" || format == "both" {
                Storage::save_json(&graph, out_path)?;
            }
            if format == "sqlite" || format == "both" {
                Storage::save_sqlite(&graph, out_path)?;
            }
            Storage::save_html(&graph, out_path)?;

            println!("Analysis written to {}", out_path.display());
        }
        Commands::Install => {
            handle_install()?;
        }
        Commands::Upgrade => {
            handle_upgrade()?;
        }
    }
    Ok(())
}

