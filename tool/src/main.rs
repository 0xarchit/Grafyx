#![deny(unsafe_code)]

mod cli;
mod ir;
mod linker;
mod parser;
mod scanner;
mod storage;
mod update;

use anyhow::{bail, Context, Result};
use clap::Parser;
use cli::{Cli, Commands};
use ir::{Graph, Node, Edge};
use std::path::Path;
use linker::Linker;
use parser::{generic::GenericParser, CodeParser};
use rayon::prelude::*;
use scanner::{Language, Scanner};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::fs;
use storage::Storage;
use tracing::info;
use colored::Colorize;
use std::time::Instant;

#[cfg(windows)]
use winreg::enums::*;
#[cfg(windows)]
use winreg::RegKey;

#[cfg(not(windows))]
fn backup_file(path: &Path) -> Result<()> {
    if path.exists() {
        let backup_path = path.with_extension(format!("{}.bak", path.extension().and_then(|s| s.to_str()).unwrap_or("backup")));
        fs::copy(path, &backup_path)
            .with_context(|| format!("Failed to create backup at {:?}", backup_path))?;
        info!("Created backup: {:?}", backup_path);
    }
    Ok(())
}

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
                let updated_path = if current_path.is_empty() { 
                    install_path_str.clone() 
                } else { 
                    format!("{};{}", current_path, install_path_str) 
                };
                env.set_value("Path", &updated_path).context("Failed to update PATH")?;
                println!("{} Installation complete! Binary copied to {}", "SUCCESS".green().bold(), install_path_str.cyan());
            } else {
                println!("{} Grafyx is already in your PATH.", "INFO".yellow().bold());
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
        
        let shells = [".bashrc", ".zshrc", ".profile", ".zprofile"];
        let marker = "# GRAFYX_ENV_START";
        let path_line = format!("\n{} \nexport PATH=\"$PATH:{}\"\n# GRAFYX_ENV_END\n", marker, install_dir.display());

        for shell in shells {
            let shell_path = home_dir.join(shell);
            if shell_path.exists() {
                backup_file(&shell_path)?;
                if let Ok(content) = fs::read_to_string(&shell_path) {
                    if !content.contains(marker) {
                        use std::io::Write;
                        let mut file = fs::OpenOptions::new()
                            .append(true)
                            .open(&shell_path)
                            .with_context(|| format!("Failed to open shell config for writing: {:?}", shell_path))?;
                        writeln!(file, "{}", path_line).with_context(|| format!("Failed to append path to {}", shell_path.display()))?;
                    }
                }
            }
        }
        println!("{} Installation complete! Binary copied to {}", "SUCCESS".green().bold(), install_dir.display().to_string().cyan());
    }
    Ok(())
}

fn handle_uninstall() -> Result<()> {
    let home_dir = home::home_dir().context("Could not find home directory")?;

    #[cfg(windows)]
    {
        let install_dir = home_dir.join("AppData").join("Local").join("grafyx");
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir).context("Failed to remove installation directory")?;
        }

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(env) = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE) {
            let current_path: String = env.get_value("Path").unwrap_or_default();
            let install_path_suffix = "grafyx\\bin";
            
            let parts: Vec<&str> = current_path.split(';').filter(|p| !p.ends_with(install_path_suffix)).collect();
            let updated_path = parts.join(";");
            
            if updated_path != current_path {
                env.set_value("Path", &updated_path).context("Failed to clean PATH")?;
            }
        }
        println!("{} Uninstallation complete. Registries and binaries cleaned.", "SUCCESS".green().bold());
    }

    #[cfg(not(windows))]
    {
        let dest = home_dir.join(".local").join("bin").join("grafyx");
        if dest.exists() {
            fs::remove_file(&dest).context("Failed to remove binary")?;
        }

        let shells = [".bashrc", ".zshrc", ".profile", ".zprofile"];
        let marker_start = "# GRAFYX_ENV_START";
        let marker_end = "# GRAFYX_ENV_END";

        for shell in shells {
            let shell_path = home_dir.join(shell);
            if shell_path.exists() {
                if let Ok(content) = fs::read_to_string(&shell_path) {
                    if content.contains(marker_start) {
                        let lines: Vec<&str> = content.lines().collect();
                        let mut new_lines = Vec::new();
                        let mut skipping = false;
                        for line in lines {
                            if line.contains(marker_start) { skipping = true; continue; }
                            if line.contains(marker_end) { skipping = false; continue; }
                            if !skipping { new_lines.push(line); }
                        }
                        fs::write(&shell_path, new_lines.join("\n")).context("Failed to clean shell profile")?;
                    }
                }
            }
        }
        println!("{} Uninstallation complete. Shell profiles and binaries cleaned.", "SUCCESS".green().bold());
    }
    Ok(())
}

fn print_banner() {
    let banner = r#"
    
     _______  ______    _______  _______  __   __  __   __ 
    |       ||    _ |  |   _   ||       ||  | |  ||  |_|  |
    |    ___||   | ||  |  |_|  ||    ___||  |_|  ||       |
    |   | __ |   |_||_ |       ||   |___ |       ||_     _|
    |   ||  ||    __  ||       ||    ___||_     _|  |   |  
    |   |_| ||   |  | ||   _   ||   |      |   |    |   |  
    |_______||___|  |_||__| |__||___|      |___|    |___|
    "#;
    println!("{}", banner.bright_cyan());
    println!("{}", "The Living Knowledge Graph for Modern Codebases".bright_black().italic());
    println!();
}

#[cfg(feature = "self-update")]
fn handle_upgrade() -> Result<()> {
    println!("Checking for updates...");
    let releases = self_update::backends::github::ReleaseList::configure()
        .repo_owner("0xarchit")
        .repo_name("grafyx")
        .build()
        .context("Failed to configure release list")?
        .fetch()
        .context("Failed to fetch releases")?;

    if releases.is_empty() {
        println!("No releases found.");
        return Ok(());
    }

    let latest = &releases[0];
    let latest_version = &latest.version;
    let current_version = env!("CARGO_PKG_VERSION");

    if self_update::version::bump_is_greater(current_version, latest_version)? {
        println!("New version {} available! (Current: {})", latest_version, current_version);
        
        let target = self_update::get_target();
        let bin_asset = latest.asset_for(target, None)
            .context("No binary asset found for your platform")?;
            
        let sig_asset_name = format!("{}.sig", bin_asset.name);
        let sig_asset = latest.assets.iter().find(|a| a.name == sig_asset_name)
            .context(format!("No signature asset found ({}). Security verification is mandatory for updates.", sig_asset_name))?;

        let tmp_dir = tempfile::Builder::new().prefix("grafyx-update").tempdir()?;
        let tmp_bin = tmp_dir.path().join(&bin_asset.name);
        let tmp_sig = tmp_dir.path().join(&sig_asset.name);

        println!("Downloading update: {}...", bin_asset.name);
        let mut bin_file = fs::File::create(&tmp_bin).context("Failed to create temp binary file")?;
        self_update::Download::from_url(&bin_asset.download_url)
            .show_progress(true)
            .download_to(&mut bin_file)
            .context("Failed to download binary")?;

        println!("Downloading signature: {}...", sig_asset.name);
        let mut sig_file = fs::File::create(&tmp_sig).context("Failed to create temp signature file")?;
        self_update::Download::from_url(&sig_asset.download_url)
            .download_to(&mut sig_file)
            .context("Failed to download signature")?;

        println!("Verifying cryptographic signature...");
        let sig_bytes = std::fs::read(&tmp_sig).context("Failed to read signature file")?;
        update::verify_signature(&tmp_bin, &sig_bytes)
            .context("Security verification failed! The update is untrusted.")?;
            
        println!("Signature verified successfully.");
        println!("Installing update...");
        
        self_update::Move::from_source(&tmp_bin)
            .replace_using_temp(&tmp_bin)
            .to_dest(&std::env::current_exe().context("Failed to locate current executable")?)
            .context("Failed to install new binary")?;

        println!("Successfully updated to Grafyx {}!", latest_version);
    } else {
        println!("Already up to date ({}).", current_version);
    }
    Ok(())
}

fn resolve_parser(lang: &Language) -> Option<GenericParser> {
    match lang {
        Language::JavaScript => Some(GenericParser::new(
            tree_sitter_javascript::LANGUAGE.into(),
            "javascript",
        )),
        Language::TypeScript => Some(GenericParser::new(
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            "typescript",
        )),
        Language::Python => Some(GenericParser::new(
            tree_sitter_python::LANGUAGE.into(),
            "python",
        )),
        Language::Java => Some(GenericParser::new(
            tree_sitter_java::LANGUAGE.into(),
            "java",
        )),
        Language::Go => Some(GenericParser::new(
            tree_sitter_go::LANGUAGE.into(),
            "go",
        )),
        Language::Rust => Some(GenericParser::new(
            tree_sitter_rust::LANGUAGE.into(),
            "rust",
        )),
        Language::Unknown => None,
    }
}

fn main() -> Result<()> {
    print_banner();
    let parse_failures = AtomicUsize::new(0);
    let start_time = Instant::now();

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
    let cli = Cli::parse();

    match &cli.command {
        Commands::Scan { dirs, ignore, format, output } => {
            println!("{} {} ...", "INIT".bright_cyan().bold(), "Deep Structural Scan".white());
            for dir in dirs {
                let p = Path::new(dir);
                if !p.exists() {
                    bail!("Scanning target directory does not exist: {}", dir);
                }
                if !p.is_dir() {
                    bail!("Scanning target is not a directory: {}", dir);
                }
            }
            
            let out_path = Path::new(output);
            if !out_path.exists() {
                fs::create_dir_all(out_path).context("Failed to create output directory")?;
            }

            // Ensure DB is initialized before scanning for cache lookups
            if let Ok(conn) = Storage::open_db(out_path) {
                let _ = Storage::init_db(&conn);
            }

            let scanner = Scanner::new(dirs.clone(), ignore.clone());
            let mut files = scanner.scan();
            
            // Ensure deterministic ordering across runs
            files.sort_by(|a, b| a.0.cmp(&b.0));
            
            let total_files = files.len();

            let results: Vec<(Vec<Node>, Vec<Edge>, bool)> = files.into_par_iter().map(|(file_path, lang)| {
                let conn = Storage::open_db(out_path).ok();
                
                if let Ok(content) = fs::read_to_string(&file_path) {
                    let hash = blake3::hash(content.as_bytes()).to_hex().to_string();

                    if let Some(ref c) = conn {
                        if let Some(old_hash) = Storage::get_file_hash(c, &file_path.to_string_lossy()) {
                            if old_hash == hash {
                                if let Ok(data) = Storage::load_file_data(c, &file_path.to_string_lossy()) {
                                    return (data.0, data.1, true);
                                }
                            }
                        }
                    }

                    if let Some(parser) = resolve_parser(&lang) {
                        match parser.parse(&file_path, &content) {
                            Ok((nodes, edges)) => {
                                if let Some(ref c) = conn {
                                    let _ = Storage::update_file_hash(c, &file_path.to_string_lossy(), &hash);
                                }
                                return (nodes, edges, false);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse {}: {}", file_path.display(), e);
                                parse_failures.fetch_add(1, Ordering::Relaxed);
                            }
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

            let fail_count = parse_failures.load(Ordering::Relaxed);
            info!("Scan complete. Total files: {}, Cached: {}, Failed: {}", total_files, skipped_count, fail_count);

            let linker = Linker::new(dirs.clone());
            linker.link(&mut graph);

            match format {
                cli::OutputFormat::Json => {
                    Storage::save_json(&graph, out_path)?;
                }
                cli::OutputFormat::Sqlite => {
                    Storage::save_sqlite(&graph, out_path)?;
                }
                cli::OutputFormat::Both => {
                    Storage::save_json(&graph, out_path)?;
                    Storage::save_sqlite(&graph, out_path)?;
                }
            }
            Storage::save_html(&graph, out_path)?;

            println!("\n{} Analysis written to {} in {:.2?}", 
                "DONE".bright_green().bold(), 
                out_path.display().to_string().cyan(),
                start_time.elapsed()
            );
        }
        Commands::Install => {
            handle_install()?;
        }
        Commands::Uninstall => {
            handle_uninstall()?;
        }
        #[cfg(feature = "self-update")]
        Commands::Upgrade => {
            handle_upgrade()?;
        }
    }
    Ok(())
}

