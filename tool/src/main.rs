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
use std::path::{Path};
use storage::Storage;

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

fn handle_install() {
    let home_dir = home::home_dir().expect("Could not find home directory");
    let current_exe = std::env::current_exe().expect("Could not get current executable path");
    
    #[cfg(windows)]
    {
        let install_dir = home_dir.join("AppData").join("Local").join("grafyx").join("bin");
        fs::create_dir_all(&install_dir).expect("Failed to create installation directory");
        let dest = install_dir.join("grafyx.exe");
        
        fs::copy(&current_exe, &dest).expect("Failed to copy binary");
        
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(env) = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE) {
            let current_path: String = env.get_value("Path").unwrap_or_default();
            let install_path_str = install_dir.to_string_lossy().to_string();
            
            if !current_path.split(';').any(|p| p == install_path_str) {
                let updated_path = format!("{};{}", current_path, install_path_str);
                env.set_value("Path", &updated_path).expect("Failed to update PATH");
                println!("Installation complete! Grafyx has been added to your PATH.");
                println!("Please restart your terminal to start using 'grafyx' from anywhere.");
            } else {
                println!("Grafyx is already in your PATH.");
            }
        }
    }

    #[cfg(not(windows))]
    {
        let install_dir = home_dir.join(".local").join("bin");
        fs::create_dir_all(&install_dir).expect("Failed to create installation directory");
        let dest = install_dir.join("grafyx");
        
        // Use copy + chmod for Unix
        fs::copy(&current_exe, &dest).expect("Failed to copy binary");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&dest).unwrap().permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&dest, perms).unwrap();
        }
        
        // Update shell configs
        let shells = [".bashrc", ".zshrc", ".profile"];
        let path_line = format!("\nexport PATH=\"$PATH:{}\"\n", install_dir.display());
        let mut appended = false;

        for shell in shells {
            let shell_path = home_dir.join(shell);
            if shell_path.exists() {
                if let Ok(content) = fs::read_to_string(&shell_path) {
                    if !content.contains(&install_dir.to_string_lossy().to_string()) {
                        if let Ok(mut file) = fs::OpenOptions::new().append(true).open(&shell_path) {
                            writeln!(file, "{}", path_line).ok();
                            appended = true;
                        }
                    }
                }
            }
        }

        println!("Installation complete! Grafyx has been copied to {}", dest.display());
        if appended {
            println!("Your PATH has been updated in your shell configuration.");
            println!("Please run 'source ~/.bashrc' or 'source ~/.zshrc' or restart your terminal.");
        }
    }
}

fn handle_upgrade() {
    println!("Checking for updates...");
    let status = self_update::backends::github::Update::configure()
        .repo_owner("0xarchit")
        .repo_name("kgraph")
        .bin_name("grafyx")
        .show_download_progress(true)
        .current_version(env!("CARGO_PKG_VERSION"))
        .build()
        .unwrap()
        .update();

    match status {
        Ok(s) => {
            if s.updated() {
                println!("Updated to version {}!", s.version());
            } else {
                println!("Already up to date!");
            }
        },
        Err(e) => eprintln!("Update failed: {}", e),
    }
}

fn check_for_updates_bg() {
    std::thread::spawn(move || {
        let releases = self_update::backends::github::ReleaseList::configure()
            .repo_owner("0xarchit")
            .repo_name("kgraph")
            .build()
            .unwrap()
            .fetch();

        if let Ok(releases) = releases {
            let releases: Vec<self_update::update::Release> = releases;
            if let Some(latest) = releases.first() {
                let current = env!("CARGO_PKG_VERSION");
                if latest.version != current {
                    println!("\n[!] A new version of Grafyx is available: v{} (Current: v{})", latest.version, current);
                    println!("[!] Run 'grafyx upgrade' to update.\n");
                }
            }
        }
    });
}

fn main() {
    let cli = Cli::parse();
    match &cli.command {
        Commands::Scan { dirs, merge: _, ignore, format, output } => {
            let out_path = Path::new(output);
            if !out_path.exists() {
                let _ = fs::create_dir_all(out_path);
            }

            let conn = Storage::open_db(out_path);
            if let Some(ref c) = conn {
                Storage::init_db(c);
            }

            let scanner = Scanner::new(dirs.clone(), ignore.clone());
            let files = scanner.scan();
            let mut graph = Graph::new();
            let total_files = files.len();
            let mut skipped_count = 0;
            
            // Parsers initialization...
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
                let path_str = file_path.to_string_lossy().to_string();
                let mut skipped = false;

                if let Some(ref c) = conn {
                    if let Ok(metadata) = fs::metadata(&file_path) {
                        let mtime = metadata.modified().unwrap_or(std::time::SystemTime::now())
                            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
                        
                        if let Some(old_mtime) = Storage::get_file_mtime(c, &path_str) {
                            if old_mtime == mtime {
                                let (nodes, edges) = Storage::load_file_data(c, &path_str);
                                if !nodes.is_empty() {
                                    graph.nodes.extend(nodes);
                                    graph.edges.extend(edges);
                                    skipped = true;
                                    skipped_count += 1;
                                }
                            }
                        }

                        if !skipped {
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
                                Storage::update_file_metadata(c, &path_str, mtime, metadata.len());
                            }
                        }
                    }
                }
            }

            println!("Scan complete. Total files: {}, Hot-rendered from cache: {}", total_files, skipped_count);

            let linker = Linker::new(dirs.clone());
            linker.link(&mut graph);

            if format == "json" || format == "both" {
                Storage::save_json(&graph, out_path);
            }
            if format == "sqlite" || format == "both" {
                Storage::save_sqlite(&graph, out_path);
            }
            Storage::save_html(&graph, out_path);

            // Check for updates in background
            check_for_updates_bg();
            
            // Give a tiny bit of time for the thread to potentially finish if the scan was very fast
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
        Commands::Install => {
            handle_install();
        }
        Commands::Upgrade => {
            handle_upgrade();
        }
    }
}

