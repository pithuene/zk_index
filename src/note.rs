use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Note {
    pub rel_path: PathBuf,
    pub absolute_path: PathBuf,
    pub vault_path: PathBuf,
    pub extension: Option<String>,
}

impl Note {
    pub fn new(vault_root_path: &Path, rel_path: &Path) -> Self {
        Self {
            rel_path: rel_path.to_path_buf(),
            absolute_path: vault_root_path.join(rel_path),
            extension: rel_path
                .extension()
                .map(|ext| ext.to_str().unwrap().to_string()),
            vault_path: vault_path_from_relative_path(rel_path),
        }
    }
}

fn vault_path_from_relative_path(rel_path: &Path) -> PathBuf {
    match rel_path.extension() {
        // with_extension("") removes the extension
        Some(ext) if ext == "md" => rel_path.with_extension(""),
        _ => rel_path.to_path_buf(),
    }
}
