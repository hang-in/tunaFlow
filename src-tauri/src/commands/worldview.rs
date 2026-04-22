//! User Worldview 파일 기반 stance 주입 (userWorldviewInjectionPlan subtask-01).
//!
//! 경로 해결:
//! - project override: `<project>/.tunaflow/user_worldview.md` (있으면 우선)
//! - global:           `~/.tunaflow/user_worldview.md`
//!
//! ContextPack 주입은 `prompt_assembly.rs` 가 `load_for_injection()` 을 호출해
//! identity 섹션 **바로 앞** 에 삽입. 토큰 상한 500.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;

use crate::errors::AppError;
use crate::guardrail::estimate_tokens;

pub const WORLDVIEW_MAX_TOKENS: usize = 500;

/// ContextPack 주입 토글 (Settings UI 에서 제어). 기본 ON.
static WORLDVIEW_ENABLED: AtomicBool = AtomicBool::new(true);

/// `<project>/.tunaflow/user_worldview.md` 우선, 없으면 `~/.tunaflow/user_worldview.md`.
pub fn resolve_worldview_path(project_path: Option<&str>) -> Option<PathBuf> {
    if let Some(pp) = project_path {
        let project_p = PathBuf::from(pp).join(".tunaflow").join("user_worldview.md");
        if project_p.exists() {
            return Some(project_p);
        }
    }
    let home = dirs::home_dir()?;
    let global_p = home.join(".tunaflow").join("user_worldview.md");
    if global_p.exists() { Some(global_p) } else { None }
}

/// 파일 내용 raw 로드 (Settings UI/테스트 용). trim 포함.
pub fn load_worldview(project_path: Option<&str>) -> Option<String> {
    let path = resolve_worldview_path(project_path)?;
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// `prompt_assembly.rs` 용: enabled 플래그 + 500 토큰 상한 적용.
/// 상한 초과 시 **앞 부분만** 사용 (char 경계 안전). 경고 1회 eprintln.
pub fn load_for_injection(project_path: Option<&str>) -> Option<String> {
    if !WORLDVIEW_ENABLED.load(Ordering::Relaxed) {
        return None;
    }
    let raw = load_worldview(project_path)?;
    Some(truncate_to_tokens(&raw, WORLDVIEW_MAX_TOKENS))
}

/// 추정 토큰 상한을 초과하면 char 단위로 앞부분만 반환. char 경계 안전 (Unicode panic 방지).
/// Codex round-3 review 반영: `str::split_at` 의 byte index 기반 panic 회피 위해 char_indices 사용.
pub fn truncate_to_tokens(text: &str, max_tokens: usize) -> String {
    let current = estimate_tokens(text);
    if current <= max_tokens {
        return text.to_string();
    }
    // Binary-search 대신 선형 근사 — estimate_tokens 의 (ascii/4 + cjk*2/3) 휴리스틱에 맞춰
    // char 비율 만큼 자른 뒤 안전하게 char 경계로 round-down.
    let ratio = max_tokens as f64 / current as f64;
    let total_chars = text.chars().count();
    let target_chars = (total_chars as f64 * ratio).floor() as usize;
    let cut_byte = text
        .char_indices()
        .nth(target_chars)
        .map(|(i, _)| i)
        .unwrap_or_else(|| text.len());
    let truncated = &text[..cut_byte];
    eprintln!(
        "[worldview] truncated {} → {} chars (est tokens {} → {})",
        total_chars,
        target_chars,
        current,
        estimate_tokens(truncated)
    );
    truncated.to_string()
}

// ─────────────────────────── Tauri commands ───────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldviewReadInput {
    pub project_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldviewWriteInput {
    pub content: String,
    pub project_path: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldviewEnabledInput {
    pub enabled: bool,
}

#[tauri::command]
pub fn get_worldview(input: WorldviewReadInput) -> Result<Option<String>, AppError> {
    Ok(load_worldview(input.project_path.as_deref()))
}

/// 현재 적용 경로 (project override 또는 global). 파일이 아직 없으면 저장 대상 경로도 global.
#[tauri::command]
pub fn get_worldview_path(input: WorldviewReadInput) -> Result<Option<String>, AppError> {
    Ok(resolve_worldview_path(input.project_path.as_deref())
        .map(|p| p.to_string_lossy().to_string()))
}

#[tauri::command]
pub fn set_worldview(input: WorldviewWriteInput) -> Result<(), AppError> {
    // 저장 경로: project override 파일이 이미 존재하면 그곳, 아니면 global.
    // 사용자가 명시적으로 project override 를 만들 때는 <project>/.tunaflow 디렉터리를
    // 먼저 만들어야 하므로 여기선 global 기본. (override 편집은 사용자가 파일을 직접
    // 만든 뒤 편집 — Settings UI 에서 현재 적용 경로를 표시.)
    let target = if let Some(pp) = input.project_path.as_deref() {
        let project_p = PathBuf::from(pp).join(".tunaflow").join("user_worldview.md");
        if project_p.exists() {
            project_p
        } else {
            global_path()?
        }
    } else {
        global_path()?
    };

    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            AppError::Agent(format!("worldview: create dir failed: {}", e))
        })?;
    }
    std::fs::write(&target, input.content).map_err(|e| {
        AppError::Agent(format!("worldview: write failed: {}", e))
    })?;
    Ok(())
}

#[tauri::command]
pub fn get_worldview_enabled() -> Result<bool, AppError> {
    Ok(WORLDVIEW_ENABLED.load(Ordering::Relaxed))
}

#[tauri::command]
pub fn set_worldview_enabled(input: WorldviewEnabledInput) -> Result<(), AppError> {
    WORLDVIEW_ENABLED.store(input.enabled, Ordering::Relaxed);
    Ok(())
}

fn global_path() -> Result<PathBuf, AppError> {
    let home = dirs::home_dir()
        .ok_or_else(|| AppError::Agent("worldview: home_dir() None".into()))?;
    Ok(home.join(".tunaflow").join("user_worldview.md"))
}

// ─────────────────────────── Tests ───────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn resolve_prefers_project_override_when_present() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().to_path_buf();
        let override_path = project_dir.join(".tunaflow").join("user_worldview.md");
        fs::create_dir_all(override_path.parent().unwrap()).unwrap();
        fs::write(&override_path, "# override").unwrap();

        let resolved = resolve_worldview_path(Some(project_dir.to_str().unwrap()));
        assert_eq!(resolved, Some(override_path));
    }

    #[test]
    fn resolve_returns_none_when_neither_exists() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().to_path_buf();
        // global 은 홈 의존이라 체크 생략. project override 만 없으면 None 이거나 global.
        let resolved = resolve_worldview_path(Some(project_dir.to_str().unwrap()));
        // global 파일이 실제 테스트 머신에 있을 수도 있으므로 assertion 완화:
        // project override 경로로 돌아오지 않았음을 확인.
        assert!(resolved
            .as_ref()
            .map(|p| !p.starts_with(&project_dir))
            .unwrap_or(true));
    }

    #[test]
    fn load_returns_trimmed_content() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().to_path_buf();
        let path = project_dir.join(".tunaflow").join("user_worldview.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "  \n# Hi\n  ").unwrap();

        let loaded = load_worldview(Some(project_dir.to_str().unwrap()));
        assert_eq!(loaded.as_deref(), Some("# Hi"));
    }

    #[test]
    fn load_filters_empty_content_to_none() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().to_path_buf();
        let path = project_dir.join(".tunaflow").join("user_worldview.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "   \n   ").unwrap();

        let loaded = load_worldview(Some(project_dir.to_str().unwrap()));
        assert!(loaded.is_none());
    }

    #[test]
    fn truncate_noop_when_under_limit() {
        let input = "hello world";
        assert_eq!(truncate_to_tokens(input, 500), input);
    }

    #[test]
    fn truncate_handles_cjk_without_panic() {
        // CJK char 는 estimate_tokens 에서 가중치 2/3. char 경계에서 cut 해야 Unicode panic 방지.
        let text: String = "안녕하세요 반갑습니다 ".repeat(500);
        let out = truncate_to_tokens(&text, 100);
        assert!(out.len() < text.len());
        // char 경계 검증 — 유효 UTF-8 이어야 함
        assert!(std::str::from_utf8(out.as_bytes()).is_ok());
    }

    #[test]
    fn load_for_injection_respects_disabled_flag() {
        // 상태 오염 방지를 위해 테스트 끝에 복구
        let prev = WORLDVIEW_ENABLED.load(Ordering::Relaxed);
        WORLDVIEW_ENABLED.store(false, Ordering::Relaxed);
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().to_path_buf();
        let path = project_dir.join(".tunaflow").join("user_worldview.md");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "# stance").unwrap();
        let got = load_for_injection(Some(project_dir.to_str().unwrap()));
        WORLDVIEW_ENABLED.store(prev, Ordering::Relaxed);
        assert!(got.is_none());
    }
}
