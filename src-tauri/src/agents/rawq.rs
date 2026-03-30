/// rawq integration — calls the real rawq CLI binary.
///
/// This module does NOT implement search logic. It delegates entirely to the
/// rawq binary and maps its JSON output to tunaFlow's `SearchResult` type.
///
/// rawq is the source of truth for search behavior, options, and output format.
///
/// If rawq is not available, this module returns explicit errors — no silent fallback.
use std::path::PathBuf;
use std::process::Command;

/// Search result mapped from rawq JSON output.
pub struct SearchResult {
    pub file: String,
    pub line: usize,
    pub snippet: String,
    pub scope: Option<String>,
    pub confidence: f64,
}

#[derive(Debug)]
pub enum RawqError {
    NotFound(String),
    ExecFailed(String),
    NonZeroExit { code: i32, stderr: String },
    ParseFailed(String),
    NoResults,
}

impl std::fmt::Display for RawqError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(msg) => write!(f, "rawq not found: {}", msg),
            Self::ExecFailed(e) => write!(f, "rawq exec failed: {}", e),
            Self::NonZeroExit { code, stderr } => write!(f, "rawq exit {}: {}", code, stderr),
            Self::ParseFailed(e) => write!(f, "rawq JSON parse: {}", e),
            Self::NoResults => write!(f, "rawq: 0 results"),
        }
    }
}

// ─── Binary resolution ───────────────────────────────────────────────────────

/// Resolve the rawq binary path.
///
/// Priority:
/// 1. `RAWQ_BIN` environment variable (explicit override)
/// 2. Bundled/development sidecar path
/// 3. Known local build path (development)
/// 4. `rawq` on PATH (development fallback)
fn resolve_rawq_bin() -> Result<PathBuf, RawqError> {
    // 1. Env override
    if let Ok(p) = std::env::var("RAWQ_BIN") {
        let path = PathBuf::from(&p);
        if path.is_file() {
            return Ok(path);
        }
        return Err(RawqError::NotFound(format!("RAWQ_BIN={} does not exist", p)));
    }

    // 2. Sidecar/dev bundle lookup
    for path in sidecar_candidates() {
        if path.is_file() {
            return Ok(path);
        }
    }

    // 3. Known local build path
    for path in known_local_builds() {
        if path.is_file() {
            return Ok(path);
        }
    }

    // 4. PATH lookup
    let status = Command::new("rawq")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if let Ok(s) = status {
        if s.success() {
            return Ok(PathBuf::from("rawq"));
        }
    }

    Err(RawqError::NotFound(
        format!(
            "rawq not found; checked RAWQ_BIN, sidecar bundle, local build paths, and PATH ({})",
            host_triple()
        ),
    ))
}

fn host_triple() -> &'static str {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "aarch64-apple-darwin"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "x86_64-apple-darwin"
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "x86_64-pc-windows-msvc"
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "aarch64") {
        "aarch64-pc-windows-msvc"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "x86_64-unknown-linux-gnu"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "aarch64-unknown-linux-gnu"
    } else {
        "unknown-target"
    }
}

fn sidecar_file_name() -> String {
    let ext = if cfg!(target_os = "windows") { ".exe" } else { "" };
    format!("rawq-{}{}", host_triple(), ext)
}

fn sidecar_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let sidecar = sidecar_file_name();

    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("src-tauri").join("binaries").join(&sidecar));
        candidates.push(cwd.join("binaries").join(&sidecar));
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            candidates.push(exe_dir.join(&sidecar));
            candidates.push(exe_dir.join("binaries").join(&sidecar));
            candidates.push(exe_dir.join("../Resources").join(&sidecar));
            candidates.push(exe_dir.join("../Resources/binaries").join(&sidecar));
        }
    }

    candidates
}

fn known_local_builds() -> Vec<PathBuf> {
    let ext = if cfg!(target_os = "windows") { ".exe" } else { "" };
    let mut candidates = Vec::new();

    if let Ok(home) = std::env::var("HOME") {
        candidates.push(PathBuf::from(&home).join(format!(
            "privateProject/_research/_util/rawq/target/release/rawq{}",
            ext
        )));
        candidates.push(PathBuf::from(&home).join(format!(
            "privateProject/tunaDish/vendor/rawq/target/release/rawq{}",
            ext
        )));
    }

    if let Ok(user_profile) = std::env::var("USERPROFILE") {
        candidates.push(PathBuf::from(&user_profile).join(format!(
            "privateProject\\_research\\_util\\rawq\\target\\release\\rawq{}",
            ext
        )));
        candidates.push(PathBuf::from(&user_profile).join(format!(
            "privateProject\\tunaDish\\vendor\\rawq\\target\\release\\rawq{}",
            ext
        )));
    }

    candidates
}

/// Check if rawq binary is available (any resolution path).
pub fn is_available() -> bool {
    resolve_rawq_bin().is_ok()
}

// ─── Daemon management ──────────────────────────────────────────────────────

/// Ensure the rawq embedding daemon is running in background.
/// The daemon pre-loads the ONNX model and serves embedding requests via IPC,
/// making subsequent index/search operations near-instant instead of 30-60s cold start.
pub fn ensure_daemon() {
    let Ok(bin) = resolve_rawq_bin() else { return; };

    // Check if already running
    let status = Command::new(&bin)
        .args(["daemon", "status"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    if let Ok(s) = status {
        if s.success() {
            eprintln!("[rawq] daemon already running");
            return;
        }
    }

    // Start daemon in background
    eprintln!("[rawq] starting daemon...");
    let result = Command::new(&bin)
        .args(["daemon", "start", "--background", "--idle-timeout", "30"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
    match result {
        Ok(_) => eprintln!("[rawq] daemon started"),
        Err(e) => eprintln!("[rawq] daemon start failed: {}", e),
    }
}

/// Stop the rawq daemon if running.
#[allow(dead_code)]
pub fn stop_daemon() {
    let Ok(bin) = resolve_rawq_bin() else { return; };
    let _ = Command::new(&bin)
        .args(["daemon", "stop"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status();
    eprintln!("[rawq] daemon stopped");
}

// ─── Index management ────────────────────────────────────────────────────────

/// Structured index info from `rawq index status --json`.
pub struct IndexInfo {
    pub files: u64,
    pub chunks: u64,
    #[allow(dead_code)]
    pub model: String,
}

/// Get index status. Returns `Ok(Some(info))` if indexed, `Ok(None)` if not, `Err` on failure.
pub fn index_status(project_path: &str) -> Result<Option<IndexInfo>, RawqError> {
    let bin = resolve_rawq_bin()?;
    let output = Command::new(&bin)
        .args(["index", "status", project_path, "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| RawqError::ExecFailed(e.to_string()))?;

    if !output.status.success() {
        return Ok(None);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| RawqError::ParseFailed(e.to_string()))?;

    let indexed = parsed.get("indexed").and_then(|v| v.as_bool()).unwrap_or(false);
    if !indexed {
        return Ok(None);
    }

    Ok(Some(IndexInfo {
        files: parsed.get("files").and_then(|v| v.as_u64()).unwrap_or(0),
        chunks: parsed.get("chunks").and_then(|v| v.as_u64()).unwrap_or(0),
        model: parsed.get("model").and_then(|v| v.as_str()).unwrap_or("unknown").to_string(),
    }))
}

/// Check if a rawq index exists for the given path.
/// Returns `true` if indexed, `false` otherwise.
///
/// CLI: `rawq index status <path> --json`
/// Output: `{ "indexed": true/false, "files": N, "chunks": N, ... }`
pub fn is_indexed(project_path: &str) -> Result<bool, RawqError> {
    let bin = resolve_rawq_bin()?;
    let output = Command::new(&bin)
        .args(["index", "status", project_path, "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .map_err(|e| RawqError::ExecFailed(e.to_string()))?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| RawqError::ParseFailed(e.to_string()))?;

    Ok(parsed.get("indexed").and_then(|v| v.as_bool()).unwrap_or(false))
}

/// Ensure a rawq index exists for the given path.
/// Checks status first; only builds if not yet indexed.
///
/// CLI: `rawq index build <path> --json`
/// Returns number of files indexed, or error.
pub fn ensure_index(project_path: &str) -> Result<u64, RawqError> {
    // Check first — skip if already indexed
    match is_indexed(project_path) {
        Ok(true) => {
            eprintln!("[rawq] index already exists for {}", project_path);
            return Ok(0);
        }
        Ok(false) => {
            eprintln!("[rawq] no index for {} — building...", project_path);
        }
        Err(e) => {
            eprintln!("[rawq] index status check failed: {} — attempting build", e);
        }
    }

    let bin = resolve_rawq_bin()?;
    // Note: rawq's WalkBuilder respects .gitignore automatically.
    // Explicit -x patterns are not needed for standard ignores.
    let child = Command::new(&bin)
        .args(["index", "build", project_path, "--json"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| RawqError::ExecFailed(e.to_string()))?;

    // Wait for completion — no timeout. Runs in background thread,
    // and daemon handles the actual work so killing the CLI is ineffective anyway.
    let t0 = std::time::Instant::now();
    let output = child.wait_with_output()
        .map_err(|e| RawqError::ExecFailed(e.to_string()))?;
    eprintln!("[rawq] index build took {}s", t0.elapsed().as_secs());

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        eprintln!("[rawq] build: {}", stderr.trim());
    }

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        return Err(RawqError::NonZeroExit { code, stderr: stderr.trim().to_string() });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = serde_json::from_str(&stdout)
        .map_err(|e| RawqError::ParseFailed(e.to_string()))?;

    let total = parsed.get("total_files").and_then(|v| v.as_u64()).unwrap_or(0);
    eprintln!("[rawq] index built: {} files", total);
    Ok(total)
}

// ─── Search ──────────────────────────────────────────────────────────────────

/// Search using the real rawq CLI. No fallback.
///
/// CLI invocation (from rawq source args.rs):
///   rawq search "<query>" <path> -n <top> --threshold 0.3 -C 2 --json
pub fn search(project_path: &str, query: &str, limit: usize) -> Result<Vec<SearchResult>, RawqError> {
    if query.trim().is_empty() || limit == 0 {
        return Err(RawqError::NoResults);
    }

    let bin = resolve_rawq_bin()?;
    let t0 = std::time::Instant::now();

    let mut child = Command::new(&bin)
        .args([
            "search",
            query,
            project_path,
            "-n", &limit.to_string(),
            "--threshold", "0.3",
            "-C", "2",
            "--json",
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| RawqError::ExecFailed(e.to_string()))?;

    // Timeout: 5 seconds max for search
    let timeout = std::time::Duration::from_secs(5);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) => {
                if t0.elapsed() > timeout {
                    let _ = child.kill();
                    eprintln!("[rawq] search timed out after {}ms", t0.elapsed().as_millis());
                    return Err(RawqError::ExecFailed("search timed out (5s)".into()));
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Err(e) => return Err(RawqError::ExecFailed(e.to_string())),
        }
    }

    let output = child.wait_with_output()
        .map_err(|e| RawqError::ExecFailed(e.to_string()))?;
    eprintln!("[rawq] search completed in {}ms", t0.elapsed().as_millis());

    if !output.status.success() {
        let code = output.status.code().unwrap_or(-1);
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        // rawq exit code 1 = no results found (not an error)
        if code == 1 {
            return Err(RawqError::NoResults);
        }
        return Err(RawqError::NonZeroExit { code, stderr });
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_json(&stdout, limit)
}

/// Parse rawq search --json output.
///
/// Schema (from rawq src/search/engine.rs):
/// ```json
/// {
///   "schema_version": 1,
///   "model": "snowflake-arctic-embed-s",
///   "results": [{
///     "file": "path/to/file.rs",
///     "lines": [23, 41],
///     "scope": "Struct.method",
///     "confidence": 0.91,
///     "content": "...",
///     "token_count": 45
///   }],
///   "query_ms": 8,
///   "total_tokens": 45
/// }
/// ```
fn parse_json(json_str: &str, limit: usize) -> Result<Vec<SearchResult>, RawqError> {
    let parsed: serde_json::Value = serde_json::from_str(json_str)
        .map_err(|e| RawqError::ParseFailed(e.to_string()))?;

    let results_arr = parsed
        .get("results")
        .and_then(|v| v.as_array())
        .ok_or_else(|| RawqError::ParseFailed("missing 'results' array".into()))?;

    let query_ms = parsed.get("query_ms").and_then(|v| v.as_u64()).unwrap_or(0);

    let mut results = Vec::new();
    for item in results_arr.iter().take(limit) {
        let file = item.get("file").and_then(|v| v.as_str()).unwrap_or("unknown");
        let line = item
            .get("lines")
            .and_then(|v| v.as_array())
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_u64())
            .unwrap_or(1) as usize;
        let scope = item.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        let confidence = item.get("confidence").and_then(|v| v.as_f64()).unwrap_or(0.0);
        let content = item.get("content").and_then(|v| v.as_str()).unwrap_or("");

        // Use full content (trimmed) as snippet, not just the first line
        let snippet = content.trim().to_string();

        if !snippet.is_empty() {
            eprintln!(
                "[rawq] {}:{} [{}] {:.0}%",
                file, line, scope, confidence * 100.0
            );
            results.push(SearchResult {
                file: file.to_string(),
                line,
                snippet,
                scope: if scope.is_empty() { None } else { Some(scope.to_string()) },
                confidence,
            });
        }
    }

    if results.is_empty() {
        return Err(RawqError::NoResults);
    }

    eprintln!("[rawq] {} results in {}ms", results.len(), query_ms);
    Ok(results)
}
