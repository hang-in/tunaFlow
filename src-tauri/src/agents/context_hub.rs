/// context-hub integration — calls the context-hub CLI binary.
///
/// context-hub is an external knowledge source runtime. tunaFlow treats it as
/// a sidecar/CLI tool, NOT as an internal module.
///
/// Source policy: bundled/local/private only. Public auto-fetch is forbidden.
///
/// If context-hub is not available, this module returns explicit errors — no silent fallback.

use std::path::PathBuf;
use std::process::Command;

use crate::no_console::NoConsole;

/// Health check result.
#[derive(Debug)]
pub struct HealthStatus {
    pub available: bool,
    pub version: Option<String>,
    pub message: String,
}

/// Search result from context-hub.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub id: String,
    pub title: String,
    pub source: String,
    pub snippet: String,
    pub score: f64,
}

/// Full document content from context-hub get.
#[derive(Debug)]
pub struct Document {
    pub id: String,
    pub title: String,
    pub content: String,
    pub source: String,
}

#[derive(Debug)]
pub enum HubError {
    NotFound,
    ExecFailed(String),
    ParseFailed(String),
    PolicyViolation(String),
    NoResults,
}

impl std::fmt::Display for HubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HubError::NotFound => write!(f, "context-hub not found"),
            HubError::ExecFailed(e) => write!(f, "context-hub exec failed: {}", e),
            HubError::ParseFailed(e) => write!(f, "context-hub parse failed: {}", e),
            HubError::PolicyViolation(e) => write!(f, "source policy violation: {}", e),
            HubError::NoResults => write!(f, "no results"),
        }
    }
}

/// Allowed source prefixes. Public registries are blocked.
const ALLOWED_SOURCE_PREFIXES: &[&str] = &[
    "local:",
    "bundled:",
    "private:",
    "file:",
    "./",
    "../",
    "/",
];

/// Check if a source identifier is allowed by policy.
fn is_source_allowed(source: &str) -> bool {
    if source.is_empty() {
        return true; // empty = default sources (local/bundled)
    }
    ALLOWED_SOURCE_PREFIXES.iter().any(|prefix| source.starts_with(prefix))
}

/// Build candidate paths for chub on Windows npm global install dirs.
/// Pure function — env values are passed in so tests don't mutate process env.
#[cfg(target_os = "windows")]
fn windows_chub_candidates(appdata: Option<&str>, userprofile: Option<&str>) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(a) = appdata {
        out.push(PathBuf::from(a).join("npm").join("chub.cmd"));
    }
    if let Some(u) = userprofile {
        out.push(
            PathBuf::from(u)
                .join("AppData")
                .join("Roaming")
                .join("npm")
                .join("chub.cmd"),
        );
    }
    out
}

/// Resolve the context-hub binary path.
/// Searches for both "context-hub" and "chub" (the actual CLI binary name).
fn resolve_bin() -> Result<PathBuf, HubError> {
    // Check common locations — both "context-hub" and "chub" names
    let candidates: Vec<PathBuf> = {
        let mut c = Vec::new();
        if let Ok(home) = std::env::var("HOME") {
            // chub (actual binary name from @aisuite/chub package)
            c.push(PathBuf::from(&home).join(".npm-global").join("bin").join("chub"));
            c.push(PathBuf::from(&home).join(".local").join("bin").join("chub"));
            // context-hub (legacy name)
            c.push(PathBuf::from(&home).join(".context-hub").join("bin").join("context-hub"));
            c.push(PathBuf::from(&home).join(".npm-global").join("bin").join("context-hub"));
            c.push(PathBuf::from(&home).join(".cargo").join("bin").join("context-hub"));
            // fnm/nvm paths
            if let Ok(fnm) = std::env::var("FNM_MULTISHELL_PATH") {
                c.push(PathBuf::from(&fnm).join("bin").join("chub"));
            }
        }
        #[cfg(target_os = "windows")]
        {
            // Windows native process: HOME is usually unset, npm globals live under
            // %APPDATA%\npm. Add explicit candidates so we don't rely solely on PATH.
            let appdata = std::env::var("APPDATA").ok();
            let userprofile = std::env::var("USERPROFILE").ok();
            c.extend(windows_chub_candidates(
                appdata.as_deref(),
                userprofile.as_deref(),
            ));
        }
        c.push(PathBuf::from("/usr/local/bin/chub"));
        c.push(PathBuf::from("/opt/homebrew/bin/chub"));
        c.push(PathBuf::from("/usr/local/bin/context-hub"));
        c.push(PathBuf::from("/opt/homebrew/bin/context-hub"));
        c
    };

    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }

    // Try bare names (PATH fallback) — chub first, then context-hub
    for name in ["chub", "context-hub"] {
        if Command::new(name)
            .no_console()
            .arg("--help")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(PathBuf::from(name));
        }
    }

    Err(HubError::NotFound)
}

/// Check if context-hub is available and return version info.
pub fn health() -> HealthStatus {
    match resolve_bin() {
        Err(_) => HealthStatus {
            available: false,
            version: None,
            message: "context-hub not installed".into(),
        },
        Ok(bin) => {
            match Command::new(&bin).no_console().arg("--cli-version").output() {
                Ok(output) if output.status.success() => {
                    let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
                    HealthStatus {
                        available: true,
                        version: Some(version.clone()),
                        message: format!("context-hub {} ready", version),
                    }
                }
                Ok(output) => HealthStatus {
                    available: false,
                    version: None,
                    message: format!("context-hub error: {}", String::from_utf8_lossy(&output.stderr).trim()),
                },
                Err(e) => HealthStatus {
                    available: false,
                    version: None,
                    message: format!("context-hub exec failed: {}", e),
                },
            }
        }
    }
}

/// Search knowledge sources within policy bounds.
///
/// `source_filter`: optional source prefix to restrict search (e.g., "local:", "file:./docs")
/// `query`: search query string
/// `limit`: max results
pub fn search(query: &str, source_filter: Option<&str>, limit: usize) -> Result<Vec<SearchResult>, HubError> {
    // Policy check
    if let Some(src) = source_filter {
        if !is_source_allowed(src) {
            return Err(HubError::PolicyViolation(format!("source '{}' not in allowed list", src)));
        }
    }

    let bin = resolve_bin()?;
    let mut cmd = Command::new(&bin);
    cmd.no_console();
    cmd.args(["search", query, "--limit", &limit.to_string(), "--json"]);
    if let Some(src) = source_filter {
        cmd.args(["--source", src]);
    }

    let output = cmd.output().map_err(|e| HubError::ExecFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.contains("no results") || output.status.code() == Some(1) {
            return Err(HubError::NoResults);
        }
        return Err(HubError::ExecFailed(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_search_results(&stdout)
}

/// Get a document by ID.
pub fn get(document_id: &str) -> Result<Document, HubError> {
    let bin = resolve_bin()?;
    let output = Command::new(&bin)
        .no_console()
        .args(["get", document_id, "--json"])
        .output()
        .map_err(|e| HubError::ExecFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(HubError::ExecFailed(stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_document(&stdout)
}

fn parse_search_results(json_str: &str) -> Result<Vec<SearchResult>, HubError> {
    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| HubError::ParseFailed(e.to_string()))?;

    let results_arr = parsed
        .get("results")
        .and_then(|v| v.as_array())
        .ok_or_else(|| HubError::ParseFailed("missing 'results' array".into()))?;

    let mut results = Vec::new();
    for item in results_arr {
        let id = item.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let title = item.get("title").or_else(|| item.get("name")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let source = item.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string();
        let snippet = item.get("snippet").or_else(|| item.get("description")).and_then(|v| v.as_str()).unwrap_or("").to_string();
        let score = item.get("score").and_then(|v| v.as_f64()).unwrap_or(0.0);

        // Policy enforcement: skip results from disallowed sources
        if !is_source_allowed(&source) {
            eprintln!("[context-hub] skipping result from disallowed source: {}", source);
            continue;
        }

        if !id.is_empty() {
            results.push(SearchResult { id, title, source, snippet, score });
        }
    }

    if results.is_empty() {
        return Err(HubError::NoResults);
    }

    Ok(results)
}

fn parse_document(json_str: &str) -> Result<Document, HubError> {
    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| HubError::ParseFailed(e.to_string()))?;

    let id = parsed.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let title = parsed.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let content = parsed.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let source = parsed.get("source").and_then(|v| v.as_str()).unwrap_or("").to_string();

    if content.is_empty() {
        return Err(HubError::ParseFailed("empty content".into()));
    }

    Ok(Document { id, title, content, source })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_policy_allows_local() {
        assert!(is_source_allowed("local:my-docs"));
        assert!(is_source_allowed("bundled:skills"));
        assert!(is_source_allowed("private:team-wiki"));
        assert!(is_source_allowed("file:./docs"));
        assert!(is_source_allowed("/Users/alice/docs"));
        assert!(is_source_allowed("./relative/path"));
        assert!(is_source_allowed("")); // empty = default
    }

    #[test]
    fn source_policy_blocks_public() {
        assert!(!is_source_allowed("https://registry.example.com"));
        assert!(!is_source_allowed("npm:react"));
        assert!(!is_source_allowed("public:pypi"));
        assert!(!is_source_allowed("registry:crates.io"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_candidates_include_appdata_npm_chub() {
        let v = windows_chub_candidates(Some(r"C:\Users\u\AppData\Roaming"), None);
        assert_eq!(v.len(), 1);
        let s = v[0].to_string_lossy().to_string();
        assert!(s.ends_with(r"npm\chub.cmd"), "got: {}", s);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_candidates_include_userprofile_appdata_roaming_npm_chub() {
        let v = windows_chub_candidates(None, Some(r"C:\Users\u"));
        assert_eq!(v.len(), 1);
        let s = v[0].to_string_lossy().to_string();
        assert!(s.contains("AppData"), "got: {}", s);
        assert!(s.contains("Roaming"), "got: {}", s);
        assert!(s.ends_with("chub.cmd"), "got: {}", s);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_candidates_empty_when_no_env() {
        assert!(windows_chub_candidates(None, None).is_empty());
    }
}
