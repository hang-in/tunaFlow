//! First-run dependency consent install (T4 / windowsDependencyBootstrapPlan_2026-04-29).
//!
//! Detects optional sidecars (context-hub via npm, code-review-graph via pip) and,
//! after explicit user consent, runs `npm install -g` / `pip install` with timeout.
//!
//! Invariants
//! - INV-DEP-A: silent global install 금지. user consent (frontend) 후에만 호출.
//! - INV-DEP-B: timeout (npm 60s / pip 120s) + graceful failure with manual command hint.
//! - Q-3: VIRTUAL_ENV 가 set 되어 있고 그 안의 pip 가 실제로 존재하면 venv 안에 설치;
//!        그렇지 않으면 system pip. silent venv 생성 금지.

use serde::Serialize;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tauri::Emitter;

use crate::no_console::NoConsole;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DependencyStatus {
    pub name: String,
    pub available: bool,
    pub installer_command: String,
    pub requires: String,
    pub version: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct InstallResult {
    pub name: String,
    pub success: bool,
    pub message: String,
    pub manual_command: Option<String>,
}

const NPM_TIMEOUT: Duration = Duration::from_secs(60);
const PIP_TIMEOUT: Duration = Duration::from_secs(120);

const NAME_CONTEXT_HUB: &str = "context-hub";
const NAME_CRG: &str = "code-review-graph";
const CHUB_INSTALL_CMD: &str = "npm install -g @aisuite/chub";

/// Probe both sidecars and report installer hints (called from first-run dialog
/// + Settings → Runtime "수동 설치" 버튼).
#[tauri::command]
pub async fn list_dependencies() -> Result<Vec<DependencyStatus>, String> {
    tokio::task::spawn_blocking(move || {
        let chub = crate::agents::context_hub::health();
        let crg_avail = crate::agents::crg::is_available();
        vec![
            DependencyStatus {
                name: NAME_CONTEXT_HUB.into(),
                available: chub.available,
                installer_command: CHUB_INSTALL_CMD.into(),
                requires: "Node.js + npm".into(),
                version: chub.version,
            },
            DependencyStatus {
                name: NAME_CRG.into(),
                available: crg_avail,
                installer_command: pip_install_hint(std::env::var("VIRTUAL_ENV").ok().as_deref()),
                requires: "Python 3 + pip".into(),
                version: None,
            },
        ]
    })
    .await
    .map_err(|e| format!("spawn_blocking: {}", e))
}

/// Run the install for one named dependency. Always emits
/// `dependency:install_result` so the dialog can react even when the
/// invocation Promise is dropped.
#[tauri::command]
pub async fn install_dependency(
    app: tauri::AppHandle,
    name: String,
) -> Result<InstallResult, String> {
    let result = tokio::task::spawn_blocking(move || run_install(&name))
        .await
        .map_err(|e| format!("spawn_blocking: {}", e))?;
    let _ = app.emit("dependency:install_result", &result);
    Ok(result)
}

fn run_install(name: &str) -> InstallResult {
    match name {
        NAME_CONTEXT_HUB => run_with_timeout(
            NAME_CONTEXT_HUB,
            "npm",
            &["install", "-g", "@aisuite/chub"],
            NPM_TIMEOUT,
            CHUB_INSTALL_CMD,
        ),
        NAME_CRG => {
            let venv = std::env::var("VIRTUAL_ENV").ok();
            let pip = pip_executable_for(venv.as_deref());
            let manual = pip_install_hint(venv.as_deref());
            run_with_timeout(
                NAME_CRG,
                &pip,
                &["install", "code-review-graph"],
                PIP_TIMEOUT,
                &manual,
            )
        }
        other => InstallResult {
            name: other.into(),
            success: false,
            message: format!("unknown dependency: {}", other),
            manual_command: None,
        },
    }
}

/// Resolve pip executable, preferring an *active* venv when its pip is
/// actually present. Pure helper so tests don't mutate process env.
fn pip_executable_for(virtual_env: Option<&str>) -> String {
    if let Some(venv) = virtual_env.filter(|s| !s.is_empty()) {
        let candidate: PathBuf = if cfg!(windows) {
            PathBuf::from(venv).join("Scripts").join("pip.exe")
        } else {
            PathBuf::from(venv).join("bin").join("pip")
        };
        if candidate.exists() {
            return candidate.to_string_lossy().to_string();
        }
    }
    "pip".into()
}

/// Human-readable install command hint; reflects venv presence.
fn pip_install_hint(virtual_env: Option<&str>) -> String {
    if virtual_env.map(|s| !s.is_empty()).unwrap_or(false) {
        "pip install code-review-graph (active venv)".into()
    } else {
        "pip install code-review-graph".into()
    }
}

fn run_with_timeout(
    name: &str,
    program: &str,
    args: &[&str],
    timeout: Duration,
    manual: &str,
) -> InstallResult {
    let mut cmd = Command::new(program);
    cmd.no_console();
    cmd.args(args);
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return InstallResult {
                name: name.into(),
                success: false,
                message: format!("spawn failed: {}", e),
                manual_command: Some(manual.into()),
            };
        }
    };

    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                if status.success() {
                    return InstallResult {
                        name: name.into(),
                        success: true,
                        message: format!("{} 설치 완료", name),
                        manual_command: None,
                    };
                }
                let mut stderr_buf = String::new();
                if let Some(mut e) = child.stderr.take() {
                    use std::io::Read;
                    let _ = e.read_to_string(&mut stderr_buf);
                }
                let trimmed = stderr_buf.trim();
                let detail = if trimmed.is_empty() {
                    format!("exit {}", status)
                } else {
                    format!("exit {}: {}", status, truncate(trimmed, 400))
                };
                return InstallResult {
                    name: name.into(),
                    success: false,
                    message: detail,
                    manual_command: Some(manual.into()),
                };
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return InstallResult {
                        name: name.into(),
                        success: false,
                        message: format!("timeout after {}s", timeout.as_secs()),
                        manual_command: Some(manual.into()),
                    };
                }
                std::thread::sleep(Duration::from_millis(200));
            }
            Err(e) => {
                return InstallResult {
                    name: name.into(),
                    success: false,
                    message: format!("wait failed: {}", e),
                    manual_command: Some(manual.into()),
                };
            }
        }
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.into()
    } else {
        let mut out = String::with_capacity(max + 3);
        out.push_str(&s[..max]);
        out.push_str("...");
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pip_executable_returns_pip_when_no_venv() {
        assert_eq!(pip_executable_for(None), "pip");
        assert_eq!(pip_executable_for(Some("")), "pip");
    }

    #[test]
    fn pip_executable_returns_pip_when_venv_path_doesnt_exist() {
        assert_eq!(
            pip_executable_for(Some("/definitely/nonexistent/venv/path/__claude_t4__")),
            "pip"
        );
    }

    #[test]
    fn pip_install_hint_reflects_venv_presence() {
        assert_eq!(pip_install_hint(None), "pip install code-review-graph");
        assert_eq!(pip_install_hint(Some("")), "pip install code-review-graph");
        assert_eq!(
            pip_install_hint(Some("/some/path")),
            "pip install code-review-graph (active venv)"
        );
    }

    #[test]
    fn run_install_unknown_dependency_returns_failure() {
        let r = run_install("not-a-real-thing");
        assert!(!r.success);
        assert!(r.message.contains("unknown"));
        assert_eq!(r.name, "not-a-real-thing");
    }

    #[test]
    fn truncate_long_string_appends_ellipsis() {
        let s = "a".repeat(500);
        let t = truncate(&s, 400);
        assert_eq!(t.len(), 403);
        assert!(t.ends_with("..."));
    }

    #[test]
    fn truncate_short_string_unchanged() {
        assert_eq!(truncate("short", 100), "short");
    }
}
