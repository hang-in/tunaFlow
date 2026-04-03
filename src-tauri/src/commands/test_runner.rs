use serde::Serialize;
use std::process::Command;
use std::path::Path;
use std::time::Instant;

use crate::errors::AppError;

/// Strip ANSI escape codes (colors, bold, etc.) from terminal output.
fn strip_ansi(s: &str) -> String {
    let re = regex::Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
    re.replace_all(s, "").to_string()
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TestRunResult {
    pub test_type: String,
    pub passed: i32,
    pub failed: i32,
    pub skipped: i32,
    pub duration_ms: i64,
    pub output: String,
    pub success: bool,
}

/// Detect and run project tests. Auto-detects test runner if test_type is None.
#[tauri::command]
pub fn run_project_tests(
    project_path: String,
    test_type: Option<String>,
) -> Result<TestRunResult, AppError> {
    let path = Path::new(&project_path);
    if !path.exists() {
        return Err(AppError::NotFound(format!("Project path not found: {}", project_path)));
    }

    let runner = match test_type.as_deref() {
        Some(t) => t.to_string(),
        None => detect_test_runner(path)?,
    };

    let start = Instant::now();
    let (output, exit_success) = execute_tests(path, &runner)?;
    let duration_ms = start.elapsed().as_millis() as i64;

    let (passed, failed, skipped) = parse_test_counts(&output, &runner);

    Ok(TestRunResult {
        test_type: runner,
        passed,
        failed,
        skipped,
        duration_ms,
        output,
        success: exit_success,
    })
}

fn detect_test_runner(path: &Path) -> Result<String, AppError> {
    if path.join("Cargo.toml").exists() {
        return Ok("cargo".into());
    }
    if path.join("package.json").exists() {
        // Check for vitest config
        if path.join("vitest.config.ts").exists()
            || path.join("vitest.config.js").exists()
            || path.join("vitest.config.mts").exists()
        {
            return Ok("vitest".into());
        }
        // Check for jest config
        if path.join("jest.config.ts").exists()
            || path.join("jest.config.js").exists()
            || path.join("jest.config.mjs").exists()
        {
            return Ok("jest".into());
        }
        // Fallback: vitest (most npm projects with vite)
        return Ok("vitest".into());
    }
    Err(AppError::NotFound("No supported test runner detected".into()))
}

fn execute_tests(path: &Path, runner: &str) -> Result<(String, bool), AppError> {
    let output = match runner {
        "cargo" => Command::new("cargo")
            .args(["test", "--lib"])
            .current_dir(path)
            .output(),
        "vitest" => Command::new("npx")
            .args([
                "vitest", "run",
                "--exclude", "**/data/**",
                "--exclude", "**/node_modules/**",
                "--exclude", "**/dist/**",
                "--exclude", "**/vendor/**",
                "--exclude", "**/.git/**",
                "--exclude", "**/target/**",
            ])
            .current_dir(path)
            .output(),
        "jest" => Command::new("npx")
            .args(["jest", "--no-coverage", "--forceExit"])
            .current_dir(path)
            .output(),
        _ => return Err(AppError::NotFound(format!("Unknown test runner: {}", runner))),
    };

    let output = output.map_err(|e| AppError::NotFound(format!("Failed to execute {}: {}", runner, e)))?;
    let stdout = strip_ansi(&String::from_utf8_lossy(&output.stdout));
    let stderr = strip_ansi(&String::from_utf8_lossy(&output.stderr));
    let combined = if stderr.is_empty() {
        stdout
    } else {
        format!("{}\n{}", stdout, stderr)
    };

    // "No test files found" is not a failure — project simply has no tests
    let success = if !output.status.success() && combined.contains("No test files found") {
        true
    } else {
        output.status.success()
    };

    Ok((combined, success))
}

fn parse_test_counts(output: &str, runner: &str) -> (i32, i32, i32) {
    match runner {
        "cargo" => parse_cargo_counts(output),
        "vitest" => parse_vitest_counts(output),
        "jest" => parse_jest_counts(output),
        _ => (0, 0, 0),
    }
}

fn parse_jest_counts(output: &str) -> (i32, i32, i32) {
    // "Tests:  2 failed, 53 passed, 55 total"
    let mut passed = 0i32;
    let mut failed = 0i32;
    let mut skipped = 0i32;
    if let Some(re) = regex::Regex::new(r"(\d+) passed").ok() {
        if let Some(caps) = re.captures(output) {
            passed = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        }
    }
    if let Some(re) = regex::Regex::new(r"(\d+) failed").ok() {
        if let Some(caps) = re.captures(output) {
            failed = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        }
    }
    if let Some(re) = regex::Regex::new(r"(\d+) skipped").ok() {
        if let Some(caps) = re.captures(output) {
            skipped = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        }
    }
    (passed, failed, skipped)
}

fn parse_cargo_counts(output: &str) -> (i32, i32, i32) {
    // "test result: ok. 57 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    let re = regex::Regex::new(r"(\d+) passed; (\d+) failed; (\d+) ignored").ok();
    if let Some(re) = re {
        if let Some(caps) = re.captures(output) {
            let passed = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let failed = caps.get(2).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            let ignored = caps.get(3).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
            return (passed, failed, ignored);
        }
    }
    (0, 0, 0)
}

fn parse_vitest_counts(output: &str) -> (i32, i32, i32) {
    // "Tests  55 passed (55)"  or  "Tests  2 failed | 53 passed (55)"
    let mut passed = 0i32;
    let mut failed = 0i32;
    let mut skipped = 0i32;

    if let Some(re) = regex::Regex::new(r"(\d+) passed").ok() {
        if let Some(caps) = re.captures(output) {
            passed = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        }
    }
    if let Some(re) = regex::Regex::new(r"(\d+) failed").ok() {
        if let Some(caps) = re.captures(output) {
            failed = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        }
    }
    if let Some(re) = regex::Regex::new(r"(\d+) skipped").ok() {
        if let Some(caps) = re.captures(output) {
            skipped = caps.get(1).and_then(|m| m.as_str().parse().ok()).unwrap_or(0);
        }
    }

    (passed, failed, skipped)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cargo_output() {
        let output = "test result: ok. 57 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out";
        let (p, f, s) = parse_cargo_counts(output);
        assert_eq!(p, 57);
        assert_eq!(f, 0);
        assert_eq!(s, 2);
    }

    #[test]
    fn parse_vitest_output() {
        let output = " Tests  55 passed (55)\n Start at 10:32:07";
        let (p, f, s) = parse_vitest_counts(output);
        assert_eq!(p, 55);
        assert_eq!(f, 0);
        assert_eq!(s, 0);
    }

    #[test]
    fn parse_vitest_with_failures() {
        let output = " Tests  2 failed | 53 passed (55)";
        let (p, f, _) = parse_vitest_counts(output);
        assert_eq!(p, 53);
        assert_eq!(f, 2);
    }
}
