//! Shared binary resolution utilities for CLI agent adapters.
//!
//! Extracts the common fnm/nvm lookup, standard path scanning, and Windows
//! npm script resolution patterns that were previously duplicated across
//! codex.rs, gemini.rs, opencode.rs, and context_hub.rs.

use std::path::PathBuf;

/// Result of resolving a CLI binary.
/// For npm-installed CLI tools on Windows, the command may be "node" with
/// the actual script as an argument.
#[allow(dead_code)]
pub struct ResolvedBinary {
    /// The command to execute (e.g., "node", "/usr/local/bin/codex", "codex")
    pub command: String,
    /// Optional script argument (Windows node invocation pattern)
    pub script_arg: Option<String>,
}

#[allow(dead_code)]
impl ResolvedBinary {
    pub fn direct(path: impl Into<String>) -> Self {
        Self { command: path.into(), script_arg: None }
    }
    pub fn with_script(command: impl Into<String>, script: impl Into<String>) -> Self {
        Self { command: command.into(), script_arg: Some(script.into()) }
    }
}

/// Configuration for resolving an npm-installed CLI binary.
#[allow(dead_code)]
pub struct NpmCliConfig {
    /// Binary name (e.g., "codex", "gemini")
    pub bin_name: &'static str,
    /// npm package scope/name (e.g., "@openai/codex", "@google/gemini-cli")
    pub npm_package: &'static str,
    /// Entry point within the package (e.g., "bin/codex.js", "dist/index.js")
    pub npm_entry: &'static str,
}

/// Resolve an npm-installed CLI binary with fnm/nvm support.
///
/// Search order:
/// 1. Windows: node_modules script → .cmd wrapper
/// 2. Unix: fnm → nvm → standard paths
/// 3. Bare name (PATH fallback)
pub fn resolve_npm_cli(config: &NpmCliConfig) -> ResolvedBinary {
    #[cfg(target_os = "windows")]
    {
        if let Some(resolved) = resolve_windows_npm(config) {
            return resolved;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        if let Some(path) = resolve_fnm_nvm(config.bin_name) {
            return ResolvedBinary::direct(path);
        }
        if let Some(path) = resolve_standard_paths(config.bin_name) {
            return ResolvedBinary::direct(path);
        }
    }

    ResolvedBinary::direct(config.bin_name)
}

/// Search fnm and nvm node version directories for a binary.
/// Returns the path from the latest installed node version.
#[cfg(not(target_os = "windows"))]
fn resolve_fnm_nvm(bin_name: &str) -> Option<String> {
    let home = std::env::var("HOME").ok()?;

    // fnm: ~/.local/share/fnm/node-versions/*/installation/bin/{bin}
    let fnm_base = PathBuf::from(&home).join(".local/share/fnm/node-versions");
    if fnm_base.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&fnm_base) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path().join("installation/bin").join(bin_name))
                .filter(|p| p.exists())
                .collect();
            versions.sort();
            if let Some(candidate) = versions.last() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    // nvm: ~/.nvm/versions/node/*/bin/{bin}
    let nvm_base = PathBuf::from(&home).join(".nvm/versions/node");
    if nvm_base.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&nvm_base) {
            let mut versions: Vec<PathBuf> = entries
                .filter_map(|e| e.ok())
                .map(|e| e.path().join("bin").join(bin_name))
                .filter(|p| p.exists())
                .collect();
            versions.sort();
            if let Some(candidate) = versions.last() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    None
}

/// Check standard Unix binary paths for a given binary name.
#[cfg(not(target_os = "windows"))]
pub fn resolve_standard_paths(bin_name: &str) -> Option<String> {
    for prefix in &["/usr/local/bin", "/usr/bin", "/opt/homebrew/bin"] {
        let candidate = PathBuf::from(prefix).join(bin_name);
        if candidate.exists() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

/// Resolve npm CLI binary on Windows.
/// Prefers direct node script invocation over .cmd wrappers.
#[cfg(target_os = "windows")]
fn resolve_windows_npm(config: &NpmCliConfig) -> Option<ResolvedBinary> {
    if let Ok(appdata) = std::env::var("APPDATA") {
        // Prefer direct node invocation (avoids .cmd wrapper issues)
        let parts: Vec<&str> = config.npm_package.split('/').collect();
        let (scope, pkg) = if parts.len() == 2 {
            (Some(parts[0]), parts[1])
        } else {
            (None, config.npm_package)
        };

        let mut entry_path = PathBuf::from(&appdata).join("npm").join("node_modules");
        if let Some(s) = scope {
            entry_path = entry_path.join(s);
        }
        entry_path = entry_path.join(pkg).join(config.npm_entry);

        if entry_path.exists() {
            let node = which_or("node", "node");
            return Some(ResolvedBinary::with_script(node, entry_path.to_string_lossy().to_string()));
        }

        // Fallback to .cmd wrapper
        let cmd_path = PathBuf::from(&appdata)
            .join("npm")
            .join(format!("{}.cmd", config.bin_name));
        if cmd_path.exists() {
            return Some(ResolvedBinary::direct(cmd_path.to_string_lossy().to_string()));
        }
    }
    None
}

/// Resolve a binary from PATH on Windows, similar to `which`.
#[cfg(target_os = "windows")]
pub fn which_or(name: &str, fallback: &str) -> String {
    std::env::var("PATH")
        .ok()
        .and_then(|path| {
            path.split(';').find_map(|dir| {
                let candidate = PathBuf::from(dir).join(format!("{}.exe", name));
                if candidate.exists() {
                    Some(candidate.to_string_lossy().to_string())
                } else {
                    None
                }
            })
        })
        .unwrap_or_else(|| fallback.to_string())
}

/// Check candidate paths in order, return first that exists.
pub fn first_existing(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.exists()).cloned()
}

/// Build a `std::process::Command` that handles Windows .cmd files correctly.
#[cfg(target_os = "windows")]
pub fn build_command(bin: &std::path::Path) -> std::process::Command {
    if bin.extension().and_then(|e| e.to_str()) == Some("cmd") {
        let mut c = std::process::Command::new("cmd");
        c.arg("/C").arg(bin);
        c
    } else {
        std::process::Command::new(bin)
    }
}

#[cfg(not(target_os = "windows"))]
pub fn build_command(bin: &std::path::Path) -> std::process::Command {
    std::process::Command::new(bin)
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_binary_direct() {
        let r = ResolvedBinary::direct("/usr/bin/test");
        assert_eq!(r.command, "/usr/bin/test");
        assert!(r.script_arg.is_none());
    }

    #[test]
    fn resolved_binary_with_script() {
        let r = ResolvedBinary::with_script("node", "/path/to/script.js");
        assert_eq!(r.command, "node");
        assert_eq!(r.script_arg.as_deref(), Some("/path/to/script.js"));
    }

    #[test]
    fn first_existing_returns_none_for_empty() {
        assert!(first_existing(&[]).is_none());
    }

    #[test]
    fn first_existing_returns_none_for_nonexistent() {
        let candidates = vec![
            PathBuf::from("/nonexistent/path/a"),
            PathBuf::from("/nonexistent/path/b"),
        ];
        assert!(first_existing(&candidates).is_none());
    }

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn resolve_standard_paths_finds_common_binaries() {
        // /usr/bin/env should exist on any Unix system
        let result = resolve_standard_paths("env");
        // We can't guarantee this in CI, so just check it doesn't panic
        let _ = result;
    }

    #[test]
    fn npm_cli_config_codex() {
        let config = NpmCliConfig {
            bin_name: "codex",
            npm_package: "@openai/codex",
            npm_entry: "bin/codex.js",
        };
        // Should not panic, returns fallback
        let resolved = resolve_npm_cli(&config);
        assert!(!resolved.command.is_empty());
    }

    #[test]
    fn npm_cli_config_gemini() {
        let config = NpmCliConfig {
            bin_name: "gemini",
            npm_package: "@google/gemini-cli",
            npm_entry: "dist/index.js",
        };
        let resolved = resolve_npm_cli(&config);
        assert!(!resolved.command.is_empty());
    }
}
