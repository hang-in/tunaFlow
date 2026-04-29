use ignore::WalkBuilder;
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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

// ─── list_project_docs (Plan E — docs panel scope policy) ───────────────────

/// Tree entry returned to the docs panel. `children` is `Some` for dirs,
/// `None` for files. We use `BTreeMap`-derived ordering on the way up so the
/// panel renders consistently across runs.
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DocsEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Option<Vec<DocsEntry>>,
}

const DOCS_MAX_DEPTH: usize = 5;
/// Hard ceiling so that a runaway repo can't lock the panel render. Combined
/// with the frontend toast at 200, the user is warned before getting close.
const DOCS_HARD_LIMIT: usize = 5_000;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocsScanResult {
    pub entries: Vec<DocsEntry>,
    /// Total file count (.md only). Used by frontend to show a perf toast at
    /// >200 — see Task 03 of docsPanelScopePolicyPlan_2026-04-29.
    pub file_count: usize,
    /// True if traversal stopped at `DOCS_HARD_LIMIT`. Frontend can hint at
    /// switching scopes.
    pub truncated: bool,
}

/// Scan a project root for `.md` documents according to a scope policy.
///
/// - `scope = "tunaflow"` → legacy behavior: top-level `.md` files + immediate
///   children of `docs/` and `.github/` (recursive within those subtrees, max
///   depth 3 like `scanDocsDir` did before).
/// - `scope = "all"` → recursive walk under `<project_root>` honoring
///   `.gitignore` (via the `ignore` crate). Depth-capped at `DOCS_MAX_DEPTH`.
///
/// Returns a tree (dirs + files) sorted dirs-first, then alphabetical within
/// each level. Hidden entries (`.foo`) are skipped — except `.github` for the
/// tunaflow scope, kept for parity with the previous TS-side scan.
#[tauri::command]
pub fn list_project_docs(project_path: String, scope: String) -> Result<DocsScanResult, AppError> {
    let root = Path::new(&project_path);
    if !root.is_dir() {
        return Err(AppError::NotFound(format!("Not a directory: {}", project_path)));
    }
    match scope.as_str() {
        "tunaflow" => Ok(scan_tunaflow(root)),
        "all" => Ok(scan_all(root)),
        other => Err(AppError::Agent(format!("Unknown docs scope: {}", other))),
    }
}

// ─── scope='tunaflow' (legacy parity) ──────────────────────────────────────

fn scan_tunaflow(root: &Path) -> DocsScanResult {
    let mut tree: Vec<DocsEntry> = Vec::new();
    let mut file_count = 0usize;

    let Ok(read) = fs::read_dir(root) else {
        return DocsScanResult { entries: tree, file_count, truncated: false };
    };

    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let full_path = entry.path();

        if is_dir {
            // Mirror legacy TS behavior: only `docs/` and `.github/` are scanned
            // (everything else under the project root is ignored in tunaflow scope).
            if name == "docs" || name == ".github" {
                let (children, count) = scan_tunaflow_subdir(&full_path, 0);
                if !children.is_empty() {
                    file_count += count;
                    tree.push(DocsEntry {
                        name,
                        path: full_path.to_string_lossy().to_string(),
                        is_dir: true,
                        children: Some(children),
                    });
                }
            }
        } else if name.ends_with(".md") && !name.starts_with('.') {
            // Hidden files (".env.md") were filtered by the previous TS scanner
            // via `list_directory`. Preserve that for parity.
            file_count += 1;
            tree.push(DocsEntry {
                name,
                path: full_path.to_string_lossy().to_string(),
                is_dir: false,
                children: None,
            });
        }
    }

    sort_tree_inplace(&mut tree);
    DocsScanResult { entries: tree, file_count, truncated: false }
}

fn scan_tunaflow_subdir(dir: &Path, depth: usize) -> (Vec<DocsEntry>, usize) {
    if depth > 3 {
        return (Vec::new(), 0);
    }
    let mut out: Vec<DocsEntry> = Vec::new();
    let mut count = 0usize;
    let Ok(read) = fs::read_dir(dir) else { return (out, 0); };
    for entry in read.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        // Skip hidden files/dirs to mirror the legacy TS `list_directory` filter.
        if name.starts_with('.') {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let full_path = entry.path();
        if is_dir {
            let (children, sub_count) = scan_tunaflow_subdir(&full_path, depth + 1);
            if !children.is_empty() {
                count += sub_count;
                out.push(DocsEntry {
                    name,
                    path: full_path.to_string_lossy().to_string(),
                    is_dir: true,
                    children: Some(children),
                });
            }
        } else if name.ends_with(".md") {
            count += 1;
            out.push(DocsEntry {
                name,
                path: full_path.to_string_lossy().to_string(),
                is_dir: false,
                children: None,
            });
        }
    }
    sort_tree_inplace(&mut out);
    (out, count)
}

// ─── scope='all' (walkdir + ignore) ────────────────────────────────────────

/// Build a tree by inserting each `.md` file path into a nested `BTreeMap`.
/// Sub-trees are converted to `Vec<DocsEntry>` afterwards. Dirs first,
/// alphabetical within each level.
fn scan_all(root: &Path) -> DocsScanResult {
    let mut root_node = Node::default();
    let mut file_count = 0usize;
    let mut truncated = false;

    // `ignore::WalkBuilder` honors `.gitignore` files automatically. We also
    // register `.gitignore` as a custom ignore filename so that projects that
    // are *not* a git repo (no `.git/` directory) still get the same filtering
    // behavior — important because users sometimes open a worktree or extract
    // a tarball without `.git/`. `hidden(true)` skips `.git/`, `.cache/`, etc.
    let walker = WalkBuilder::new(root)
        .max_depth(Some(DOCS_MAX_DEPTH))
        .hidden(true)
        .git_ignore(true)
        .git_exclude(true)
        .git_global(true)
        .parents(true)
        .add_custom_ignore_filename(".gitignore")
        .build();

    for result in walker {
        let dent = match result {
            Ok(d) => d,
            Err(_) => continue, // permission denied / IO — silently skip
        };
        // Skip the root dir entry itself.
        let path = dent.path();
        if path == root {
            continue;
        }
        let is_file = dent.file_type().map(|t| t.is_file()).unwrap_or(false);
        if !is_file {
            continue;
        }
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if !name.ends_with(".md") {
            continue;
        }

        // Hard ceiling — defensive guard. Frontend toast warns at 200.
        if file_count >= DOCS_HARD_LIMIT {
            truncated = true;
            break;
        }
        file_count += 1;

        // Insert into tree using path components relative to root.
        let rel = match path.strip_prefix(root) {
            Ok(r) => r,
            Err(_) => continue,
        };
        let comps: Vec<String> = rel
            .components()
            .filter_map(|c| c.as_os_str().to_str().map(String::from))
            .collect();
        if comps.is_empty() {
            continue;
        }

        let mut cursor = &mut root_node;
        let last_idx = comps.len() - 1;
        for (i, comp) in comps.iter().enumerate() {
            let entry = cursor.children.entry(comp.clone()).or_default();
            if i == last_idx {
                entry.is_file = true;
                entry.full_path = Some(path.to_path_buf());
            }
            cursor = entry;
        }
    }

    let entries = node_to_entries(&root_node, root);
    DocsScanResult { entries, file_count, truncated }
}

fn node_to_entries(node: &Node, base: &Path) -> Vec<DocsEntry> {
    let mut out: Vec<DocsEntry> = Vec::new();
    for (name, child) in node.children.iter() {
        if child.is_file {
            let full = child
                .full_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| base.join(name).to_string_lossy().to_string());
            out.push(DocsEntry {
                name: name.clone(),
                path: full,
                is_dir: false,
                children: None,
            });
        } else {
            // Directory node — recurse.
            let sub_base = base.join(name);
            let children = node_to_entries(child, &sub_base);
            // Skip empty dirs (no .md descendants).
            if children.is_empty() {
                continue;
            }
            out.push(DocsEntry {
                name: name.clone(),
                path: sub_base.to_string_lossy().to_string(),
                is_dir: true,
                children: Some(children),
            });
        }
    }
    sort_tree_inplace(&mut out);
    out
}

#[derive(Default)]
struct Node {
    children: BTreeMap<String, Node>,
    is_file: bool,
    full_path: Option<PathBuf>,
}

// ─── shared helpers ────────────────────────────────────────────────────────

fn sort_tree_inplace(items: &mut Vec<DocsEntry>) {
    items.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    for it in items.iter_mut() {
        if let Some(children) = it.children.as_mut() {
            sort_tree_inplace(children);
        }
    }
}

/// Read file content as UTF-8 string. Used by Docs viewer popup.
#[tauri::command]
pub fn read_file_content(path: String) -> Result<String, AppError> {
    fs::read_to_string(&path)
        .map_err(|e| AppError::NotFound(format!("Cannot read {}: {}", path, e)))
}

/// Read a text file's content, resolved relative to a project root.
///
/// Security: only allows reading files under `project_path`.
/// Returns file content as string, or error if outside scope or not readable.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileContent {
    pub path: String,
    pub content: String,
    pub language: String,
    pub line_count: usize,
}

#[tauri::command]
pub fn read_text_file(file_path: String, project_path: String) -> Result<FileContent, AppError> {
    let project = Path::new(&project_path).canonicalize()
        .map_err(|e| AppError::NotFound(format!("Invalid project path: {}", e)))?;

    // Resolve: absolute or relative to project
    let resolved = if Path::new(&file_path).is_absolute() {
        Path::new(&file_path).to_path_buf()
    } else {
        project.join(&file_path)
    };
    let canonical = resolved.canonicalize()
        .map_err(|e| AppError::NotFound(format!("File not found: {}", e)))?;

    // Security: must be under project root
    if !canonical.starts_with(&project) {
        return Err(AppError::Agent(format!(
            "Access denied: {} is outside project scope", file_path
        )));
    }

    if !canonical.is_file() {
        return Err(AppError::NotFound(format!("Not a file: {}", file_path)));
    }

    // Size guard: max 512KB
    let metadata = fs::metadata(&canonical)
        .map_err(|e| AppError::NotFound(format!("Cannot read metadata: {}", e)))?;
    if metadata.len() > 512 * 1024 {
        return Err(AppError::Agent(format!(
            "File too large: {} bytes (max 512KB)", metadata.len()
        )));
    }

    let content = fs::read_to_string(&canonical)
        .map_err(|e| AppError::Agent(format!("Cannot read file: {}", e)))?;

    let line_count = content.lines().count();
    let ext = canonical.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    let language = match ext.as_str() {
        "rs" => "rust", "ts" | "tsx" => "typescript", "js" | "jsx" => "javascript",
        "py" => "python", "go" => "go", "java" => "java", "rb" => "ruby",
        "md" => "markdown", "json" => "json", "toml" => "toml", "yaml" | "yml" => "yaml",
        "html" => "html", "css" => "css", "sql" => "sql", "sh" | "bash" => "bash",
        "xml" => "xml", "c" | "h" => "c", "cpp" | "cc" | "hpp" => "cpp",
        _ => "text",
    }.to_string();

    Ok(FileContent {
        path: canonical.to_string_lossy().to_string(),
        content,
        language,
        line_count,
    })
}

#[cfg(test)]
mod tests {
    //! Plan E (docsPanelScopePolicyPlan_2026-04-29) — scope-aware traversal.
    //!
    //! Targets:
    //! - scope='tunaflow' parity with previous TS-side scan (docs/ + .github/
    //!   + root .md only).
    //! - scope='all' walks the project root recursively, honors .gitignore,
    //!   skips hidden dirs.
    //! - scope='all' applies max depth (5).
    //! - scope='all' file count > 0 reflects total .md count for the toast.
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut f = File::create(path).unwrap();
        writeln!(f, "# {}", path.display()).unwrap();
    }

    fn flatten_md_paths(entries: &[DocsEntry], out: &mut Vec<String>) {
        for e in entries {
            if e.is_dir {
                if let Some(children) = &e.children {
                    flatten_md_paths(children, out);
                }
            } else {
                // Normalize Windows backslash to forward-slash so assertions like
                // `ends_with("docs/foo.md")` / `contains("/.git/")` remain portable.
                // Production `e.path` keeps OS-native separators — this is test-only.
                out.push(e.path.replace('\\', "/"));
            }
        }
    }

    #[test]
    fn tunaflow_scope_returns_root_md_and_docs_only() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("README.md"));
        touch(&root.join("CHANGELOG.md"));
        touch(&root.join("docs/guide.md"));
        touch(&root.join("docs/sub/deep.md"));
        // Should NOT appear in tunaflow scope.
        touch(&root.join("packages/lib/docs/x.md"));
        touch(&root.join("scripts/build.md"));

        let result = list_project_docs(
            root.to_string_lossy().to_string(),
            "tunaflow".to_string(),
        )
        .unwrap();

        let mut paths: Vec<String> = Vec::new();
        flatten_md_paths(&result.entries, &mut paths);
        // root_prefix is normalized to forward-slash to match `paths` (already
        // normalized inside `flatten_md_paths`).
        let root_prefix = root.to_string_lossy().replace('\\', "/");
        let names: Vec<String> = paths
            .iter()
            .map(|p| p.replace(&format!("{}/", root_prefix), ""))
            .collect();

        assert!(names.contains(&"README.md".to_string()));
        assert!(names.contains(&"CHANGELOG.md".to_string()));
        assert!(names.iter().any(|n| n.ends_with("docs/guide.md")));
        assert!(names.iter().any(|n| n.ends_with("docs/sub/deep.md")));
        // Out of scope:
        assert!(!names.iter().any(|n| n.contains("packages/")));
        assert!(!names.iter().any(|n| n.contains("scripts/")));

        assert_eq!(result.file_count, 4);
        assert!(!result.truncated);
    }

    #[test]
    fn all_scope_walks_recursively_and_respects_gitignore() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        // git-tracked docs.
        touch(&root.join("README.md"));
        touch(&root.join("docs/guide.md"));
        touch(&root.join("packages/lib/docs/x.md"));
        touch(&root.join("scripts/build.md"));
        // node_modules content + .gitignore should hide it.
        touch(&root.join("node_modules/foo/README.md"));
        // Build dir excluded via .gitignore (not a built-in skip otherwise).
        touch(&root.join("dist/output.md"));
        // .gitignore tells walker to skip these:
        let mut gi = File::create(root.join(".gitignore")).unwrap();
        writeln!(gi, "node_modules/").unwrap();
        writeln!(gi, "dist/").unwrap();

        let result = list_project_docs(
            root.to_string_lossy().to_string(),
            "all".to_string(),
        )
        .unwrap();

        let mut paths: Vec<String> = Vec::new();
        flatten_md_paths(&result.entries, &mut paths);

        assert!(paths.iter().any(|p| p.ends_with("README.md")));
        assert!(paths.iter().any(|p| p.ends_with("docs/guide.md")));
        assert!(paths.iter().any(|p| p.ends_with("packages/lib/docs/x.md")));
        assert!(paths.iter().any(|p| p.ends_with("scripts/build.md")));
        // gitignored:
        assert!(!paths.iter().any(|p| p.contains("node_modules")));
        assert!(!paths.iter().any(|p| p.contains("dist")));

        assert_eq!(result.file_count, 4);
        assert!(!result.truncated);
    }

    #[test]
    fn all_scope_skips_hidden_dirs() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("docs/a.md"));
        // Hidden directory — should be skipped by the ignore walker.
        touch(&root.join(".git/HEAD.md"));
        touch(&root.join(".cache/tmp.md"));

        let result = list_project_docs(
            root.to_string_lossy().to_string(),
            "all".to_string(),
        )
        .unwrap();

        let mut paths: Vec<String> = Vec::new();
        flatten_md_paths(&result.entries, &mut paths);
        assert!(paths.iter().any(|p| p.ends_with("docs/a.md")));
        assert!(!paths.iter().any(|p| p.contains("/.git/")));
        assert!(!paths.iter().any(|p| p.contains("/.cache/")));
    }

    #[test]
    fn all_scope_caps_depth_at_5() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        // `WalkBuilder::max_depth(Some(5))` counts the root as depth 0, so a
        // file at `a/b/c/d/within.md` sits at depth 5 (4 dirs + 1 file). The
        // first file beyond the cap is `a/b/c/d/e/beyond.md` (depth 6).
        touch(&root.join("a/b/c/d/within.md"));      // depth 5 — included
        touch(&root.join("a/b/c/d/e/beyond.md"));    // depth 6 — excluded
        let result = list_project_docs(
            root.to_string_lossy().to_string(),
            "all".to_string(),
        )
        .unwrap();
        let mut paths: Vec<String> = Vec::new();
        flatten_md_paths(&result.entries, &mut paths);
        assert!(paths.iter().any(|p| p.ends_with("within.md")));
        assert!(!paths.iter().any(|p| p.ends_with("beyond.md")));
    }

    #[test]
    fn unknown_scope_returns_error() {
        let tmp = tempdir().unwrap();
        let res = list_project_docs(
            tmp.path().to_string_lossy().to_string(),
            "custom".to_string(),
        );
        assert!(res.is_err());
    }

    #[test]
    fn tunaflow_scope_includes_dot_github_and_skips_hidden_md() {
        // Regression guard for `'tunaflow'` parity with legacy TS scanner:
        // - `.github/` dir is scanned (legacy behavior).
        // - `.foo.md` (hidden file) is excluded.
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("README.md"));
        touch(&root.join(".secret.md"));               // hidden — excluded
        touch(&root.join(".github/PULL_REQUEST_TEMPLATE.md"));
        touch(&root.join("docs/.draft.md"));           // hidden inside docs — excluded

        let result = list_project_docs(
            root.to_string_lossy().to_string(),
            "tunaflow".to_string(),
        )
        .unwrap();
        let mut paths: Vec<String> = Vec::new();
        flatten_md_paths(&result.entries, &mut paths);

        assert!(paths.iter().any(|p| p.ends_with("README.md")));
        assert!(paths.iter().any(|p| p.ends_with(".github/PULL_REQUEST_TEMPLATE.md")));
        assert!(!paths.iter().any(|p| p.ends_with(".secret.md")));
        assert!(!paths.iter().any(|p| p.ends_with(".draft.md")));
    }

    #[test]
    fn dirs_sorted_before_files_alphabetical() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        touch(&root.join("zeta.md"));
        touch(&root.join("alpha.md"));
        touch(&root.join("docs/a.md"));
        touch(&root.join("docs/b.md"));

        let result = list_project_docs(
            root.to_string_lossy().to_string(),
            "tunaflow".to_string(),
        )
        .unwrap();
        // dir 'docs/' first, then 'alpha.md', then 'zeta.md'.
        assert_eq!(result.entries.len(), 3);
        assert!(result.entries[0].is_dir);
        assert_eq!(result.entries[0].name, "docs");
        assert!(!result.entries[1].is_dir);
        assert_eq!(result.entries[1].name, "alpha.md");
        assert_eq!(result.entries[2].name, "zeta.md");
    }
}
