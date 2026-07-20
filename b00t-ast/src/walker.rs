// b00t-ast/src/walker.rs
//
// Directory walker — traverses project trees, finds .rs files,
// applies .gitignore-style patterns, filters test/build directories.

use crate::CodeElement;
use crate::extract;
use anyhow::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Configuration for walking a project directory
#[derive(Debug, Clone)]
pub struct WalkConfig {
    /// Directories to exclude (relative names or full paths)
    pub exclude_dirs: Vec<String>,
    /// File extensions to parse
    pub extensions: Vec<String>,
}

impl Default for WalkConfig {
    fn default() -> Self {
        WalkConfig {
            exclude_dirs: vec![
                "target".into(),
                ".git".into(),
                "node_modules".into(),
                "vendor".into(),
                "build".into(),
                "dist".into(),
                ".hermes".into(),
                ".b00t".into(),
            ],
            extensions: vec!["rs".into()],
        }
    }
}

/// Walk a directory and collect all file paths matching the config
pub fn collect_source_files(root: &Path, config: &WalkConfig) -> Vec<PathBuf> {
    let mut files = Vec::new();

    for entry in WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            // Skip excluded directories
            if e.file_type().is_dir() {
                !config
                    .exclude_dirs
                    .iter()
                    .any(|ex| name == ex.as_str() || e.path().to_string_lossy().contains(ex))
            } else {
                true
            }
        })
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if config.extensions.iter().any(|e: &String| ext == e.as_str()) {
                files.push(path.to_path_buf());
            }
        }
    }

    files
}

/// Walk a directory, parse all .rs files, return extracted elements
pub fn walk_and_extract(root: &Path, config: &WalkConfig) -> Result<Vec<CodeElement>> {
    let files = collect_source_files(root, config);
    let mut all_elements = Vec::new();

    for file_path in &files {
        let relative = file_path
            .strip_prefix(root)
            .unwrap_or(file_path)
            .with_extension("")
            .to_string_lossy()
            .replace('/', "::")
            .replace('\\', "::");

        match extract::extract_file(file_path, &relative) {
            Ok(elements) => all_elements.extend(elements),
            Err(e) => {
                // Non-fatal: log and continue
                eprintln!("⚠️  failed to parse {}: {e}", file_path.display());
            }
        }
    }

    Ok(all_elements)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_collect_source_files_finds_rs() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("lib.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("README.md"), "").unwrap();

        let files = collect_source_files(dir.path(), &Default::default());
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("lib.rs"));
    }

    #[test]
    fn test_collect_source_files_excludes_target() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("target")).unwrap();
        fs::create_dir_all(dir.path().join("target/debug")).unwrap();
        fs::create_dir_all(dir.path().join("src")).unwrap();
        fs::write(dir.path().join("src/lib.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("target/debug/lib.rs"), "fn main() {}").unwrap();

        let files = collect_source_files(dir.path(), &Default::default());
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("src/lib.rs"));
    }

    #[test]
    fn test_collect_source_files_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let files = collect_source_files(dir.path(), &Default::default());
        assert!(files.is_empty());
    }
}
