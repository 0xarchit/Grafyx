use ignore::WalkBuilder;
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
    // Node / JS / TS
    "node_modules".to_string(),
    "dist".to_string(),
    "build".to_string(),
    ".next".to_string(),
    ".nuxt".to_string(),
    ".cache".to_string(),
    ".parcel-cache".to_string(),
    ".svelte-kit".to_string(),
    "out".to_string(),
    "coverage".to_string(),
    ".turbo".to_string(),
    ".vite".to_string(),

    // Python
    "__pycache__".to_string(),
    ".pytest_cache".to_string(),
    ".mypy_cache".to_string(),
    ".ruff_cache".to_string(),
    ".hypothesis".to_string(),
    ".tox".to_string(),
    ".nox".to_string(),
    ".venv".to_string(),
    "venv".to_string(),
    "env".to_string(),
    ".env".to_string(),

    // Java / JVM
    "target".to_string(),     // Maven / Rust overlap
    "build".to_string(),      // Gradle
    ".gradle".to_string(),
    "out".to_string(),
    "*.class".to_string(),

    // Go
    "bin".to_string(),
    "pkg".to_string(),
    "*.test".to_string(),
    "vendor".to_string(), // sometimes not waste, use carefully

    // Rust
    "target".to_string(),
    "**/*.rs.bk".to_string(),

    // General
    ".git".to_string(),
    ".idea".to_string(),
    ".vscode".to_string(),
    "*.log".to_string(),
    "*.tmp".to_string(),
    "*.swp".to_string(),
    "*.lock".to_string(),
    "tmp".to_string(),
    "temp".to_string(),
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
        for dir in &self.dirs {
            let path = Path::new(dir);
            if !path.exists() {
                continue;
            }
            let mut builder = WalkBuilder::new(path);
            builder.hidden(false);
            for ignore in &self.ignore_patterns {
                builder.add_custom_ignore_filename(ignore);
            }
            for result in builder.build() {
                if let Ok(entry) = result {
                    if entry.file_type().map_or(false, |ft| ft.is_file()) {
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
        }
        results
    }
}
