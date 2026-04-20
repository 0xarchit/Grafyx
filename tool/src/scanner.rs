use ignore::{WalkBuilder, overrides::OverrideBuilder};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    JavaScript,
    TypeScript,
    Python,
    Go,
    Rust,
    Java,
    Unknown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "js" | "jsx" => Self::JavaScript,
            "ts" | "tsx" => Self::TypeScript,
            "py" => Self::Python,
            "go" => Self::Go,
            "rs" => Self::Rust,
            "java" => Self::Java,
            _ => Self::Unknown,
        }
    }
}

pub struct Scanner {
    dirs: Vec<String>,
    ignore_patterns: Vec<String>,
}

impl Scanner {
    pub fn new(dirs: Vec<String>, ignore_patterns: Option<Vec<String>>) -> Self {
        let mut ignores = vec![
            // Global / OS
            ".git".to_string(),
            ".idea".to_string(),
            ".vscode".to_string(),
            ".DS_Store".to_string(),
            "Thumbs.db".to_string(),
            "tmp".to_string(),
            "temp".to_string(),
            "*.log".to_string(),
            "*.tmp".to_string(),
            "*.swp".to_string(),
            "*.bak".to_string(),
            "*.lock".to_string(),

            // Rust / C++
            "target".to_string(),
            "**/*.rs.bk".to_string(),
            "cargo-home".to_string(),

            // JavaScript / TypeScript / Frontend
            "node_modules".to_string(),
            "dist".to_string(),
            "build".to_string(),
            "out".to_string(),
            ".next".to_string(),
            ".nuxt".to_string(),
            ".svelte-kit".to_string(),
            ".turbo".to_string(),
            ".vite".to_string(),
            ".parcel-cache".to_string(),
            ".cache".to_string(),
            ".vercel".to_string(),
            ".netlify".to_string(),
            ".docusaurus".to_string(),
            "coverage".to_string(),
            "bower_components".to_string(),
            "storybook-static".to_string(),

            // Python
            "__pycache__".to_string(),
            "venv".to_string(),
            ".venv".to_string(),
            "env".to_string(),
            ".env".to_string(),
            "virtualenv".to_string(),
            ".pytest_cache".to_string(),
            ".mypy_cache".to_string(),
            ".ruff_cache".to_string(),
            ".tox".to_string(),
            ".nox".to_string(),
            ".hypothesis".to_string(),
            "htmlcov".to_string(),
            ".coverage".to_string(),
            "*.pyc".to_string(),
            "*.pyo".to_string(),
            "*.pyd".to_string(),
            ".ipynb_checkpoints".to_string(),

            // Go
            "vendor".to_string(),
            "bin".to_string(),
            "pkg".to_string(),
            ".gocache".to_string(),

            // Java / Kotlin / Android
            ".gradle".to_string(),
            ".m2".to_string(),
            "*.class".to_string(),
            "*.jar".to_string(),
            "*.war".to_string(),
            "*.ear".to_string(),
            ".apt_generated".to_string(),
            ".settings".to_string(),
            ".project".to_string(),
            ".classpath".to_string(),
            "local.properties".to_string(),

            // Mobile / Others
            ".expo".to_string(),
            ".output".to_string(),
            "Pods".to_string(),
            ".dart_tool".to_string(),
        ];
        if let Some(custom) = ignore_patterns {
            ignores.extend(custom);
        }
        Self {
            dirs,
            ignore_patterns: ignores,
        }
    }

    pub fn scan(&self) -> Vec<(PathBuf, Language)> {
        let mut results = Vec::new();
        
        let mut override_builder = OverrideBuilder::new(".");
        for pattern in &self.ignore_patterns {
            let mut p = pattern.clone();
            if !p.starts_with('!') { p = format!("!{}", p); }
            override_builder.add(&p).ok();
        }
        let overrides = override_builder.build().unwrap_or(ignore::overrides::Override::empty());

        for dir in &self.dirs {
            let path = Path::new(dir);
            if !path.exists() {
                continue;
            }
            let mut builder = WalkBuilder::new(path);
            builder.hidden(false);
            builder.overrides(overrides.clone());
            for entry in builder.build().flatten() {
                if entry.file_type().is_some_and(|ft| ft.is_file()) {
                    let file_path = entry.into_path();
                    if let Some(ext) = file_path.extension().and_then(|s| s.to_str()) {
                        let lang = Language::from_extension(ext);
                        if lang != Language::Unknown {
                            results.push((file_path, lang));
                        }
                    }
                }
            }
        }
        results
    }
}
