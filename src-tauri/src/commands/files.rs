use serde::Serialize;
use std::fs;
use std::path::Path;

use crate::errors::AppError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub path: String,
}

/// List immediate children of a directory (1 level deep).
/// Skips hidden files/folders (starting with `.`) and common noise dirs.
#[tauri::command]
pub fn list_directory(path: String) -> Result<Vec<DirEntry>, AppError> {
    let dir = Path::new(&path);
    if !dir.is_dir() {
        return Err(AppError::NotFound(format!("Not a directory: {}", path)));
    }

    let skip = [
        "node_modules", "target", "dist", ".git", ".next",
        "__pycache__", ".venv", "venv", ".idea", ".vscode",
    ];

    let mut entries: Vec<DirEntry> = Vec::new();
    let Ok(read) = fs::read_dir(dir) else {
        return Ok(entries);
    };

    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') { continue; }
        if skip.contains(&name.as_str()) { continue; }

        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let full_path = entry.path().to_string_lossy().to_string();
        entries.push(DirEntry { name, is_dir, path: full_path });
    }

    // Dirs first, then files, alphabetical within each group
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(entries)
}
