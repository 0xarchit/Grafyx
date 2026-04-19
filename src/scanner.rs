use ignore::WalkBuilder;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    JavaScript,
    Python,
    Go,
    Rust,
    Java,
    Unknown,
}

impl Language {
    pub fn from_extension(ext: &str) -> Self {
        match ext {
            "js" | "ts" | "jsx" | "tsx" => Self::JavaScript,
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
            "node_modules".to_string(),
            "__pycache__".to_string(),
            "target".to_string(),
            "dist".to_string(),
            "build".to_string(),
            ".git".to_string(),
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
