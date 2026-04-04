//! code-review-graph sidecar — structural code analysis via CLI subprocess.
//!
//! Provides callers_of, tests_for, impact, and detect_changes queries.
//! Falls back gracefully if the binary is not installed.

#![allow(dead_code)] // query/impact/detect_changes are used by ContextPack and tool-request handlers

use serde::Deserialize;
use std::process::Command;

#[derive(Debug)]
pub enum CrgError {
    NotFound,
    ExecFailed(String),
    ParseFailed(String),
}

/// Resolve the code-review-graph binary path.
fn resolve_bin() -> Result<String, CrgError> {
    // Check common locations
    let home = std::env::var("HOME").unwrap_or_default();
    let candidates = [
        format!("{home}/.local/bin/code-review-graph"),
        "/opt/homebrew/bin/code-review-graph".to_string(),
        "/usr/local/bin/code-review-graph".to_string(),
    ];
    for c in &candidates {
        if std::path::Path::new(c).exists() {
            return Ok(c.clone());
        }
    }
    // Fallback: PATH lookup via `which`
    let output = Command::new("which")
        .arg("code-review-graph")
        .output()
        .map_err(|_| CrgError::NotFound)?;
    if output.status.success() {
        let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !path.is_empty() {
            return Ok(path);
        }
    }
    Err(CrgError::NotFound)
}

/// Check if code-review-graph is available.
pub fn is_available() -> bool {
    resolve_bin().is_ok()
}

// ─── Query types ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct QueryResult {
    pub status: String,
    pub pattern: Option<String>,
    pub target: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub results: Vec<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ImpactResult {
    pub status: String,
    pub summary: Option<String>,
    #[serde(default)]
    pub changed_nodes: Vec<serde_json::Value>,
    #[serde(default)]
    pub impacted_nodes: Vec<serde_json::Value>,
    #[serde(default)]
    pub impacted_files: Vec<String>,
    #[serde(default)]
    pub truncated: bool,
    #[serde(default)]
    pub total_impacted: usize,
}

// ─── CLI wrappers ─────────────────────────────────────────────────────────────

fn run_command(args: &[&str], project_path: &str) -> Result<String, CrgError> {
    let bin = resolve_bin()?;
    let output = Command::new(&bin)
        .args(args)
        .args(["--repo", project_path])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .map_err(|e| CrgError::ExecFailed(e.to_string()))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(CrgError::ExecFailed(stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run a graph query (callers_of, tests_for, etc.)
pub fn query(project_path: &str, pattern: &str, target: &str) -> Result<QueryResult, CrgError> {
    let stdout = run_command(&["query", pattern, target], project_path)?;
    serde_json::from_str(&stdout)
        .map_err(|e| CrgError::ParseFailed(format!("query parse: {}", e)))
}

/// Analyze blast radius of changed files.
pub fn impact(project_path: &str, changed_files: &[String], depth: u32) -> Result<ImpactResult, CrgError> {
    let depth_str = depth.to_string();
    let mut args: Vec<&str> = vec!["impact", "--depth", &depth_str];
    for f in changed_files {
        args.push(f.as_str());
    }
    let stdout = run_command(&args, project_path)?;
    serde_json::from_str(&stdout)
        .map_err(|e| CrgError::ParseFailed(format!("impact parse: {}", e)))
}

/// Run detect-changes (risk-scored change analysis).
pub fn detect_changes(project_path: &str, base: &str) -> Result<serde_json::Value, CrgError> {
    let stdout = run_command(&["detect-changes", "--base", base], project_path)?;
    serde_json::from_str(&stdout)
        .map_err(|e| CrgError::ParseFailed(format!("detect-changes parse: {}", e)))
}

/// Run incremental graph update (after agent completes).
pub fn update(project_path: &str) -> Result<(), CrgError> {
    let _ = run_command(&["update"], project_path)?;
    Ok(())
}
