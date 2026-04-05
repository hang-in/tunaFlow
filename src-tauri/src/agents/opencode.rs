use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;

use super::claude::resolve_cwd;

/// Resolve the opencode binary path via shared resolve utilities.
fn resolve_opencode_path() -> PathBuf {
    use super::resolve::first_existing;

    #[cfg(target_os = "windows")]
    {
        let mut candidates = Vec::new();
        if let Ok(appdata) = std::env::var("APPDATA") {
            candidates.push(PathBuf::from(&appdata).join("npm").join("opencode.cmd"));
        }
        if let Ok(home) = std::env::var("USERPROFILE") {
            candidates.push(PathBuf::from(&home).join(".npm-global").join("bin").join("opencode.cmd"));
        }
        if let Some(found) = first_existing(&candidates) {
            return found;
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        let mut candidates = Vec::new();
        if let Ok(home) = std::env::var("HOME") {
            candidates.push(PathBuf::from(&home).join(".opencode").join("bin").join("opencode"));
        }
        candidates.extend([
            PathBuf::from("/usr/local/bin/opencode"),
            PathBuf::from("/usr/bin/opencode"),
            PathBuf::from("/opt/homebrew/bin/opencode"),
        ]);
        if let Ok(home) = std::env::var("HOME") {
            candidates.push(PathBuf::from(&home).join(".npm-global").join("bin").join("opencode"));
        }
        if let Some(found) = first_existing(&candidates) {
            return found;
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
/// Build command with Windows .cmd handling via shared resolve module.
fn build_command(bin: &std::path::PathBuf) -> Command {
    super::resolve::build_command(bin)
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
