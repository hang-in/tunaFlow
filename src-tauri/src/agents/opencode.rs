use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;

use super::claude::resolve_cwd;

/// Resolve the opencode binary path.
///
/// Tauri subprocesses may not inherit the full shell PATH.
/// Search order:
/// 1. %APPDATA%\npm\opencode.cmd  (Windows npm global install default)
/// 2. %USERPROFILE%\.npm-global\bin\opencode.cmd
/// 3. /usr/local/bin/opencode, /usr/bin/opencode, /opt/homebrew/bin/opencode (Unix)
/// 4. Bare "opencode" — OS PATH fallback
fn resolve_opencode_path() -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let candidate = PathBuf::from(&appdata).join("npm").join("opencode.cmd");
            if candidate.exists() {
                return candidate;
            }
        }
        if let Ok(home) = std::env::var("USERPROFILE") {
            let candidate = PathBuf::from(&home)
                .join(".npm-global")
                .join("bin")
                .join("opencode.cmd");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // Check ~/.opencode/bin first (official installer default)
        if let Ok(home) = std::env::var("HOME") {
            let candidate = PathBuf::from(&home).join(".opencode").join("bin").join("opencode");
            if candidate.exists() {
                return candidate;
            }
        }
        for prefix in &["/usr/local/bin", "/usr/bin", "/opt/homebrew/bin"] {
            let candidate = PathBuf::from(prefix).join("opencode");
            if candidate.exists() {
                return candidate;
            }
        }
        if let Ok(home) = std::env::var("HOME") {
            let candidate = PathBuf::from(&home)
                .join(".npm-global")
                .join("bin")
                .join("opencode");
            if candidate.exists() {
                return candidate;
            }
        }
    }

    PathBuf::from("opencode")
}

/// Execute `opencode run <prompt>` as a one-shot non-interactive subprocess.
///
/// Uses the `run` subcommand for non-interactive execution of the OpenCode CLI.
/// Fields not supported by opencode (resume_token, system_prompt) are silently ignored.
/// Cost and token fields are unavailable from opencode stdout; returned as 0.
///
/// Error surface:
/// - spawn failure   → Err (opencode not installed / PATH issue)
/// - non-zero exit   → Err with stderr (or stdout) detail
/// - zero exit, stdout empty, stderr has content → Err (soft error)
/// - zero exit, stdout empty, no stderr → Ok("") — caller decides how to display
/// On Windows, `.cmd` files must be invoked via `cmd.exe /C` — direct spawn fails.
#[cfg(target_os = "windows")]
fn build_command(bin: &std::path::PathBuf) -> Command {
    if bin.extension().and_then(|e| e.to_str()) == Some("cmd") {
        let mut c = Command::new("cmd");
        c.arg("/C").arg(bin);
        c
    } else {
        Command::new(bin)
    }
}

#[cfg(not(target_os = "windows"))]
fn build_command(bin: &std::path::PathBuf) -> Command {
    Command::new(bin)
}

pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let opencode_bin = resolve_opencode_path();
    let mut cmd = build_command(&opencode_bin);
    cmd.arg("run").arg(&input.prompt);

    if let Some(model) = &input.model {
        cmd.arg("--model").arg(model);
    }

    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(resolve_cwd(input.project_path.as_deref()));

    let mut child = cmd.spawn().map_err(|e| {
        AppError::Agent(format!("Failed to spawn opencode (tried: {}): {}", opencode_bin.display(), e))
    })?;

    // Drain stderr in a background thread to prevent pipe-buffer deadlock
    let mut stderr_pipe = child
        .stderr
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture opencode stderr".into()))?;
    let stderr_handle = thread::spawn(move || {
        let mut buf = String::new();
        let _ = stderr_pipe.read_to_string(&mut buf);
        buf
    });

    // Read stdout synchronously (process is writing both; stderr thread prevents deadlock)
    let mut stdout_pipe = child
        .stdout
        .take()
        .ok_or_else(|| AppError::Agent("Failed to capture opencode stdout".into()))?;
    let mut stdout_raw = String::new();
    stdout_pipe
        .read_to_string(&mut stdout_raw)
        .map_err(|e| AppError::Agent(format!("Failed to read opencode stdout: {}", e)))?;

    let exit_status = child.wait()?;
    let stderr_content = stderr_handle.join().unwrap_or_default();

    // Non-zero exit → surface real error message
    if !exit_status.success() {
        let detail = if !stderr_content.trim().is_empty() {
            stderr_content.trim().to_string()
        } else if !stdout_raw.trim().is_empty() {
            stdout_raw.trim().to_string()
        } else {
            format!("exit code {:?}", exit_status.code())
        };
        return Err(AppError::Agent(format!("opencode failed: {}", detail)));
    }

    let content = stdout_raw.trim().to_string();

    // Zero exit but stdout empty while stderr has content → soft error
    if content.is_empty() && !stderr_content.trim().is_empty() {
        return Err(AppError::Agent(format!(
            "opencode produced no output: {}",
            stderr_content.trim()
        )));
    }

    Ok(RunOutput {
        content,
        cost_usd: 0.0,
        input_tokens: 0,
        output_tokens: 0,
        session_id: None,
    })
}
