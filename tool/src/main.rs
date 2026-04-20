#![allow(unsafe_code)]

use grafyx::cli::{Cli, Commands, OutputFormat};
use grafyx::ir::{Graph, Node, Edge};
use grafyx::linker::Linker;
use grafyx::parser::{generic::GenericParser, CodeParser};
use grafyx::scanner::{Language, Scanner};
use grafyx::storage::Storage;
use grafyx::update;

use anyhow::{bail, Context, Result};
use clap::Parser;
use std::path::Path;
use rayon::prelude::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::collections::HashSet;
use std::fs;
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
        let install_dir = home_dir.join("AppData").join("Roaming").join("grafyx").join("bin");
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
                
                broadcast_setting_change();
                
                println!("{} Installation complete! Binary copied to {}", "SUCCESS".green().bold(), install_path_str.cyan());
                println!("{} New terminal windows (including VS Code) will now recognize 'grafyx'.", "INFO".yellow().bold());
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
        
        let shells = [
            ".bashrc", ".zshrc", ".profile", ".zprofile", 
            ".bash_profile", ".bash_login"
        ];
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
        let install_dir = home_dir.join("AppData").join("Roaming").join("grafyx");
        let bin_dir = install_dir.join("bin");
        
        // Check if we are running from the installation directory
        let current_exe = std::env::current_exe().ok();
        let is_running_from_install = current_exe.as_ref().map_or(false, |p| p.starts_with(&bin_dir));

        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(env) = hkcu.open_subkey_with_flags("Environment", KEY_READ | KEY_WRITE) {
            let current_path: String = env.get_value("Path").unwrap_or_default();
            let install_path_suffix = "grafyx\\bin";
            
            let parts: Vec<&str> = current_path.split(';').filter(|p| !p.ends_with(install_path_suffix)).collect();
            let updated_path = parts.join(";");
            
            if updated_path != current_path {
                env.set_value("Path", &updated_path).context("Failed to clean PATH")?;
                broadcast_setting_change();
            }
        }

        if install_dir.exists() {
            if is_running_from_install {
                println!("{} Running from installation directory. Spawning cleanup script...", "INFO".yellow().bold());
                
                let target_dir = install_dir.to_string_lossy();
                // Windows trick: Rename the running exe and spawn a cmd to delete the folder after a delay
                let script = format!(
                    "timeout /t 1 /nobreak > NUL && rd /s /q \"{}\"",
                    target_dir
                );
                
                std::process::Command::new("cmd")
                    .args(&["/C", &script])
                    .spawn()
                    .context("Failed to spawn cleanup script")?;
                
                println!("{} Uninstallation scheduled. The installation directory will be removed momentarily.", "SUCCESS".green().bold());
                std::process::exit(0);
            } else {
                fs::remove_dir_all(&install_dir).context("Failed to remove installation directory")?;
                println!("{} Uninstallation complete. Registries and binaries cleaned.", "SUCCESS".green().bold());
            }
        }
    }

    #[cfg(not(windows))]
    {
        let dest = home_dir.join(".local").join("bin").join("grafyx");
        if dest.exists() {
            fs::remove_file(&dest).context("Failed to remove binary")?;
        }

        let shells = [
            ".bashrc", ".zshrc", ".profile", ".zprofile",
            ".bash_profile", ".bash_login"
        ];
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
                        let mut result = new_lines.join("\n");
                        if !result.is_empty() { result.push('\n'); }
                        fs::write(&shell_path, result).context("Failed to clean shell profile")?;
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
        let asset_name = match target {
            "x86_64-unknown-linux-musl" => "grafyx-linux-amd64-static",
            "aarch64-apple-darwin" => "grafyx-macos-aarch64",
            "x86_64-apple-darwin" => "grafyx-macos-x86_64",
            "x86_64-pc-windows-msvc" => "grafyx-windows-amd64.exe",
            _ => "",
        };

        let bin_asset = if !asset_name.is_empty() {
            latest.assets.iter().find(|a| a.name == asset_name).cloned()
        } else {
            latest.asset_for(target, None)
        }.context(format!("No binary asset found for your platform ({}). Please update manually at https://github.com/0xarchit/grafyx/releases", target))?;
            
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
        
        let install_path = std::env::current_exe().context("Failed to locate current executable")?;
        let temp_install_path = install_path.with_extension("tmp_update");
        
        self_update::Move::from_source(&tmp_bin)
            .replace_using_temp(&temp_install_path)
            .to_dest(&install_path)
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

fn discover_services(roots: &[String]) -> Vec<String> {
    let mut services = HashSet::new();
    let markers = ["package.json", "Cargo.toml", "go.mod", "pyproject.toml", "requirements.txt", "pom.xml", "build.gradle"];

    for root in roots {
        let root_path = Path::new(root);
        if !root_path.exists() { continue; }

        // Check root itself
        for marker in &markers {
            if root_path.join(marker).exists() {
                services.insert(root_path.to_string_lossy().to_string());
                break;
            }
        }

        // Look deeper for common monorepo patterns (one level down)
        if let Ok(entries) = fs::read_dir(root_path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                    if ["node_modules", "target", "build", "dist", ".git", ".idea", ".vscode"].contains(&name) {
                        continue;
                    }

                    // Check if this dir or its immediate children are services
                    for marker in &markers {
                        if path.join(marker).exists() {
                            services.insert(path.to_string_lossy().to_string());
                            break;
                        }
                    }

                    // Check one more level if it's a known container like 'packages' or 'apps'
                    if name == "packages" || name == "apps" {
                        if let Ok(sub_entries) = fs::read_dir(&path) {
                            for sub_entry in sub_entries.flatten() {
                                let sub_path = sub_entry.path();
                                if sub_path.is_dir() {
                                    for marker in &markers {
                                        if sub_path.join(marker).exists() {
                                            services.insert(sub_path.to_string_lossy().to_string());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if services.is_empty() {
        roots.to_vec()
    } else {
        services.into_iter().collect()
    }
}

fn main() -> Result<()> {
    print_banner();
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

            let scanner_dirs = dirs.clone();
            let discovered = discover_services(&dirs);
            if discovered.len() > dirs.len() {
                println!("{} Detected {} sub-services in project structure.", "INFO".yellow().bold(), discovered.len());
                // We keep scanner_dirs as the user requested root to ensure all files are scanned,
                // but we pass discovered to the Linker to define service boundaries.
            }

            let scanner = Scanner::new(scanner_dirs.clone(), ignore.clone());
            let mut files = scanner.scan();
            
            // Ensure deterministic ordering across runs
            files.sort_by(|a, b| a.0.cmp(&b.0));
            
            let total_files = files.len();
            let parse_failures = AtomicUsize::new(0);

            let results: Vec<(String, String, Vec<Node>, Vec<Edge>, bool)> = files.into_par_iter().map(|(file_path, lang)| {
                let file_path_str = file_path.to_string_lossy().to_string();
                let conn = Storage::open_db(out_path).ok();
                
                if let Ok(content) = fs::read_to_string(&file_path) {
                    let hash = blake3::hash(content.as_bytes()).to_hex().to_string();

                    if let Some(ref c) = conn {
                        if let Some(old_hash) = Storage::get_file_hash(c, &file_path_str) {
                            if old_hash == hash {
                                if let Ok(data) = Storage::load_file_data(c, &file_path_str) {
                                    return (file_path_str, hash, data.0, data.1, true);
                                }
                            }
                        }
                    }

                    if let Some(parser) = resolve_parser(&lang) {
                        match parser.parse(&file_path, &content) {
                            Ok((nodes, edges)) => {
                                return (file_path_str, hash, nodes, edges, false);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse {}: {}", file_path.display(), e);
                                parse_failures.fetch_add(1, Ordering::Relaxed);
                            }
                        }
                    }
                }

                (file_path_str, "".to_string(), Vec::new(), Vec::new(), false)
            }).collect();

            let mut graph = Graph::new();
            let mut skipped_count = 0;
            let mut pending_hashes = Vec::new();
            for (file_path, hash, nodes, edges, skipped) in results {
                graph.nodes.extend(nodes);
                graph.edges.extend(edges);
                if skipped { 
                    skipped_count += 1; 
                } else if !hash.is_empty() {
                    pending_hashes.push((file_path, hash));
                }
            }

            let fail_count = parse_failures.load(Ordering::Relaxed);
            info!("Scan complete. Total files: {}, Cached: {}, Failed: {}", total_files, skipped_count, fail_count);

            let linker = Linker::new(discovered);
            linker.link(&mut graph);

            let sqlite_saved = match format {
                OutputFormat::Json => {
                    Storage::save_json(&graph, out_path)?;
                    false
                }
                OutputFormat::Sqlite => {
                    Storage::save_sqlite(&graph, out_path)?;
                    true
                }
                OutputFormat::Both => {
                    Storage::save_json(&graph, out_path)?;
                    Storage::save_sqlite(&graph, out_path)?;
                    true
                }
            };

            if sqlite_saved {
                if let Ok(conn) = Storage::open_db(out_path) {
                    for (path, hash) in pending_hashes {
                        let _ = Storage::update_file_hash(&conn, &path, &hash);
                    }
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

#[cfg(windows)]
fn broadcast_setting_change() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        SendMessageTimeoutW, HWND_BROADCAST, WM_SETTINGCHANGE, SMTO_ABORTIFHUNG,
    };
    use windows_sys::Win32::Foundation::LPARAM;

    let env = "Environment\0".encode_utf16().collect::<Vec<u16>>();
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            env.as_ptr() as LPARAM,
            SMTO_ABORTIFHUNG,
            5000,
            std::ptr::null_mut(),
        );
    }
}

