use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::no_console::NoConsole;

use super::projects::detect_project_info;

// ─── Cancellation flag ───────────────────────────────────────────────────────

static CANCEL_FLAG: OnceLock<Mutex<Option<Arc<AtomicBool>>>> = OnceLock::new();

fn cancel_flag() -> &'static Mutex<Option<Arc<AtomicBool>>> {
    CANCEL_FLAG.get_or_init(|| Mutex::new(None))
}
fn set_cancel_flag(flag: Arc<AtomicBool>) {
    if let Ok(mut g) = cancel_flag().lock() { *g = Some(flag); }
}
fn clear_cancel_flag() {
    if let Ok(mut g) = cancel_flag().lock() { *g = None; }
}
fn is_cancelled(flag: &AtomicBool) -> bool {
    flag.load(Ordering::Relaxed)
}

// ─── Event payloads ──────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct OnboardingStepPayload {
    pub step: u8,
    pub label: String,
    pub done: bool,
}

#[derive(Serialize, Clone)]
pub struct OnboardingPreviewPayload {
    pub claude_md: String,
    pub ref_index: String,
    pub has_existing_claude_md: bool,
    /// Optional — meta-agent's initial setup recommendation (agent profiles,
    /// skills, workflow defaults). May be `None` if the agent omitted the
    /// [INITIAL_SETUP_*] block or if the JSON inside it was unparseable.
    /// In that case onboarding falls back to the legacy flow (claude_md +
    /// ref_index only) — see plan §7 (안전 장치).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_setup: Option<serde_json::Value>,
}

#[derive(Serialize, Clone)]
pub struct OnboardingErrorPayload {
    pub message: String,
}

// ─── Helper: scan docs folder ────────────────────────────────────────────────

fn scan_docs_files(project_path: &str) -> Vec<String> {
    let docs = std::path::Path::new(project_path).join("docs");
    if !docs.is_dir() { return vec![]; }

    let mut files = Vec::new();
    collect_md_files(&docs, &docs, &mut files, 0);
    files
}

fn collect_md_files(
    base: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<String>,
    depth: usize,
) {
    if depth > 4 { return; }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_md_files(base, &path, out, depth + 1);
        } else if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Ok(rel) = path.strip_prefix(base) {
                out.push(rel.to_string_lossy().to_string());
            }
        }
    }
}

// ─── Helper: scan project surface (README + top-level + manifests) ──────────

/// Gather enough information that the meta-agent can identify the stack even
/// when `detect_project_info` misses (e.g. Swift, Go, Zig, Elixir, Gleam,
/// Haskell, Deno, custom Makefile projects).
struct ProjectSurface {
    top_entries: Vec<String>,       // 상위 디렉토리 파일/폴더 목록
    readme_excerpt: Option<String>, // README.md 앞부분
    manifest_samples: Vec<(String, String)>, // (filename, content excerpt)
}

fn scan_project_surface(project_path: &str) -> ProjectSurface {
    let root = std::path::Path::new(project_path);
    let mut surface = ProjectSurface {
        top_entries: vec![],
        readme_excerpt: None,
        manifest_samples: vec![],
    };

    // 1) 상위 디렉토리 엔트리 (최대 40개). dotfile 대부분 제외 (signal 적음)
    if let Ok(entries) = std::fs::read_dir(root) {
        let mut names: Vec<(String, bool)> = entries
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let is_dir = e.file_type().ok().map(|t| t.is_dir()).unwrap_or(false);
                // Skip heavy noise dirs / hidden except a few meaningful ones
                let skip = matches!(
                    name.as_str(),
                    "node_modules" | "target" | ".git" | ".next" | ".svelte-kit" |
                    "dist" | "build" | ".venv" | "venv" | "__pycache__" | ".DS_Store"
                );
                if skip { return None; }
                Some((name, is_dir))
            })
            .take(40)
            .collect();
        names.sort();
        surface.top_entries = names.into_iter()
            .map(|(n, d)| if d { format!("{}/", n) } else { n })
            .collect();
    }

    // 2) README 탐지 (README.md → README.rst → README)
    for cand in ["README.md", "Readme.md", "readme.md", "README.markdown", "README.rst", "README.txt", "README"] {
        let p = root.join(cand);
        if p.is_file() {
            if let Ok(content) = std::fs::read_to_string(&p) {
                let excerpt = if content.chars().count() > 2000 {
                    let mut end = 2000;
                    while end > 0 && !content.is_char_boundary(end) { end -= 1; }
                    format!("{}...\n(README truncated)", &content[..end])
                } else {
                    content
                };
                surface.readme_excerpt = Some(format!("## {}\n{}", cand, excerpt));
                break;
            }
        }
    }

    // 3) 주요 manifest 샘플. 첫 600자 정도.
    // Rust/TS/JS/Python/Go/Swift/Ruby/Elixir/Deno/Gleam/Haskell/Makefile/Nix 등.
    let manifests = [
        "Cargo.toml", "package.json", "pyproject.toml", "requirements.txt",
        "setup.py", "go.mod", "Gemfile", "Gemfile.lock",
        "Package.swift", "Podfile", "Cartfile", "project.pbxproj",
        "mix.exs", "rebar.config",
        "deno.json", "deno.jsonc", "bun.lockb", "pnpm-workspace.yaml",
        "gleam.toml", "build.zig", "stack.yaml", "cabal.project",
        "Makefile", "CMakeLists.txt", "flake.nix", "shell.nix", "default.nix",
        "composer.json", "pubspec.yaml", ".tool-versions",
    ];
    for name in manifests {
        let p = root.join(name);
        if p.is_file() {
            if let Ok(content) = std::fs::read_to_string(&p) {
                let mut end = content.len().min(600);
                while end > 0 && !content.is_char_boundary(end) { end -= 1; }
                let sample = if content.len() > end {
                    format!("{}\n...(truncated)", &content[..end])
                } else {
                    content
                };
                surface.manifest_samples.push((name.to_string(), sample));
            }
            if surface.manifest_samples.len() >= 6 { break; }
        }
    }

    surface
}

// ─── Prompt builder ──────────────────────────────────────────────────────────

fn build_prompt(
    project_name: &str,
    project_path: &str,
    docs_files: &[String],
    existing_claude_md: &Option<String>,
) -> String {
    let info = detect_project_info(project_path);

    let stack_summary = if info.detected_stack.is_empty() {
        "알 수 없음 (manifest 파일 없음)".to_string()
    } else {
        info.detected_stack.join(", ")
    };

    let lang = info.language.as_deref().unwrap_or("Unknown");
    let framework = info.framework.as_deref().unwrap_or("");
    let test_cmd = info.test_command.as_deref().unwrap_or("");
    let build_cmd = info.build_command.as_deref().unwrap_or("");
    let type_cmd = info.type_check_command.as_deref().unwrap_or("");

    // Truncate existing CLAUDE.md to 3000 chars to stay within prompt budget
    let existing_section = match existing_claude_md {
        Some(content) => {
            let truncated = if content.len() > 3000 {
                let mut end = 3000;
                while end > 0 && !content.is_char_boundary(end) { end -= 1; }
                format!("{}...(truncated)", &content[..end])
            } else {
                content.clone()
            };
            format!("\n\n## 기존 CLAUDE.md 내용 (참고용)\n```\n{}\n```", truncated)
        }
        None => String::new(),
    };

    let docs_section = if docs_files.is_empty() {
        "없음".to_string()
    } else {
        docs_files.iter().map(|f| format!("- docs/{}", f)).collect::<Vec<_>>().join("\n")
    };

    // Project surface — README + top-level entries + manifest samples.
    // detect_project_info() only knows a short allow-list of stacks (Rust/TS/JS/...),
    // so Swift/Go/Zig/Elixir 등이 "미확인"으로 나가는 문제를 보완.
    let surface = scan_project_surface(project_path);
    let top_entries_section = if surface.top_entries.is_empty() {
        "(빈 디렉토리)".to_string()
    } else {
        surface.top_entries.join(", ")
    };
    let readme_section = surface.readme_excerpt
        .map(|e| format!("\n\n## README\n{}", e))
        .unwrap_or_default();
    let manifest_section = if surface.manifest_samples.is_empty() {
        String::new()
    } else {
        let mut s = String::from("\n\n## 매니페스트 파일 샘플 (각 상위 일부)");
        for (name, content) in &surface.manifest_samples {
            s.push_str(&format!("\n\n### {}\n```\n{}\n```", name, content));
        }
        s
    };

    format!(
        r#"아래 프로젝트 정보를 분석하여 두 가지 파일의 내용을 생성해 주세요.
감지 결과가 '미확인'이라도, **매니페스트 파일 샘플과 README, 디렉토리 구조**를
참고해서 실제 언어/프레임워크/빌드 명령을 추론하세요. 추측이라면 근거를
함께 적으세요.

## 프로젝트 정보 (자동 감지 — 보조적)

- 이름: {project_name}
- 언어(감지): {lang}
- 프레임워크(감지): {framework}
- 스택(감지): {stack_summary}
- 테스트 명령(감지): {test_cmd}
- 빌드 명령(감지): {build_cmd}
- 타입 체크(감지): {type_cmd}

## 상위 디렉토리 엔트리
{top_entries_section}{readme_section}{manifest_section}

## 기존 문서 목록
{docs_section}{existing_section}

---

## 출력 형식 (반드시 준수)

아래 섹션들을 정확한 마커와 함께 출력하세요. 다음 규칙을 어기면 응답을 사용할 수 없습니다.

**출력 규칙**:
1. 응답의 **첫 줄은 `[CLAUDE_MD_START]`** 로 시작해야 합니다. 인사말, 머리말, "다음과 같이 정리했습니다" 등의 introduction 금지.
2. 마커는 **영문 ASCII 대괄호** 그대로 출력하세요. `[ CLAUDE_MD_START ]` (공백), `**[CLAUDE_MD_START]**` (볼드), `[CLAUDE\_MD\_START]` (escape), `【CLAUDE_MD_START】` (전각괄호) 모두 금지.
3. 응답을 markdown code fence (```` ``` ````) 로 감싸지 마세요. 마커 자체가 구분자입니다.
4. 마커 사이의 본문만 작성하고, 마커 외부에는 어떤 설명/결론/맺음말도 적지 마세요.
5. 마지막 섹션이 끝난 직후 (`[INITIAL_SETUP_END]` 또는 `[REF_INDEX_END]` 다음) **즉시 응답을 종료**하세요.

[CLAUDE_MD_START]
# {project_name} — Claude Code Handoff Document

## 1. Project Overview

(프로젝트 목적과 핵심 기능을 2~4문장으로 설명. 기존 CLAUDE.md가 있으면 그 내용을 참고해서 더 정확하게 작성.)

## 2. 기술 스택

| 계층 | 기술 |
|------|------|
(감지된 스택 기반으로 채우기. 모르는 것은 "미확인"으로 표기.)

## 3. 빌드 / 테스트

```bash
(감지된 명령어로 채우기. 없으면 일반적인 패턴으로 추측.)
```

## 4. 코딩 컨벤션

(기존 CLAUDE.md에 컨벤션이 있으면 그대로 옮기기. 없으면 스택 기반 일반 컨벤션 제안.)

## 5. 다음 우선순위

- 미정 (에이전트와 상의하여 채우세요)

---

> Auto-detected by tunaFlow. 내용을 검토하고 필요하면 수정하세요.
[CLAUDE_MD_END]

[REF_INDEX_START]
# Reference

> 이 프로젝트의 문서 인덱스입니다.

(docs/ 아래 기존 문서 목록이 있으면 카테고리별로 정리. 없으면 빈 섹션만 만들기.)

## 계획 문서
- [plans/index.md](plans/index.md)

## 참고 문서
(기존 docs 파일이 있으면 여기 링크로 추가)

## 프롬프트
- [prompts/index.md](prompts/index.md)
[REF_INDEX_END]

[INITIAL_SETUP_START]
{{
  "agent_profiles": [
    {{ "role": "architect", "engine": "claude", "model": "opus",          "persona_id": "persona_architect" }},
    {{ "role": "developer", "engine": "codex",  "model": "gpt-5-codex",    "persona_id": "persona_implementer" }},
    {{ "role": "reviewer",  "engine": "gemini", "model": "gemini-2.5-pro", "persona_id": "persona_reviewer" }}
  ],
  "skills": ["rust-review", "cargo-test"],
  "workflow": {{
    "review_track": "deep",
    "context_mode": "auto",
    "rt_participants": ["claude", "codex", "gemini"]
  }},
  "rationale": "스택과 프로젝트 성격 기반 추천 근거 1~2문장"
}}
[INITIAL_SETUP_END]

### INITIAL_SETUP 작성 규칙
- **JSON만** 출력. 주석·trailing comma 금지.
- `persona_id`는 다음 값 중 하나만: `persona_general`, `persona_reviewer`, `persona_tester`, `persona_architect`, `persona_implementer`, `persona_debugger`, `persona_ux_critic`, `persona_meta`.
- `engine`은 `claude` / `codex` / `gemini` / `ollama` / `lmstudio` 중 설치 가능성이 높은 것만 추천. 확신 없으면 profile 항목에서 생략.
- `skills`는 사용자가 로드한 `~/.tunaflow/skills/` 레지스트리 이름(kebab-case, e.g. `rust-review`). 모르면 빈 배열 `[]`.
- `review_track`: `quick` (subtask ≤ 3) 또는 `deep`. `context_mode`: `auto` / `lite` / `standard` / `full`.
- `rt_participants`는 2~3개 엔진 이름. 1인 프로젝트에서 모델 하나만 추천한다면 빈 배열 허용.
- `rationale`은 1~2문장으로 간결하게. 추천 근거(스택, 프로젝트 규모 등) 언급.
- 스택을 전혀 추론하지 못했다면 이 섹션 전체를 빈 객체 `{{}}`로 출력 가능 (건너뜀 처리됨).
"#,
        project_name = project_name,
        lang = lang,
        framework = framework,
        stack_summary = stack_summary,
        test_cmd = test_cmd,
        build_cmd = build_cmd,
        type_cmd = type_cmd,
        docs_section = docs_section,
        existing_section = existing_section,
    )
}

// ─── Parse output ────────────────────────────────────────────────────────────
//
// `parse_output` 는 Codex / Gemini / Claude 의 plain-text 응답 안에서 마커로
// 감싼 3 섹션 (CLAUDE.md, REF_INDEX, INITIAL_SETUP) 을 추출한다. 모델별 응답
// 형식 차이를 흡수해야 한다 — 자세한 가설은
// docs/reference/codexGeminiOnboardingResponseAudit_2026-04-25.md 참조.
//
// 강건화 포인트 (Layer A):
//
// 1. 마커 자체가 markdown bold (`**[CLAUDE_MD_START]**`) 또는 underscore escape
//    (`[CLAUDE\_MD\_START]`) 로 변형되어도 통과.
// 2. 마커 앞뒤로 markdown code fence (` ```markdown ... ``` `) 가 있어도 통과.
// 3. 마커 좌우 공백 / newline / 마크다운 헤더 prefix 자유.
// 4. 마커 매칭 실패 시 raw 응답 앞 200자를 에러 메시지에 포함 (디버깅 단서).
// 5. 추출된 본문에서 wrapping markdown fence 자동 strip.

fn parse_output(text: &str) -> Result<(String, String, Option<serde_json::Value>), String> {
    let claude_md = extract_section(text, "CLAUDE_MD")
        .ok_or_else(|| {
            let preview = response_preview(text);
            format!(
                "AI 응답에서 CLAUDE.md 섹션을 찾을 수 없습니다 (응답 시작 부분: {preview})"
            )
        })?;
    let ref_index = extract_section(text, "REF_INDEX")
        .ok_or_else(|| {
            let preview = response_preview(text);
            format!(
                "AI 응답에서 Reference Index 섹션을 찾을 수 없습니다 (응답 시작 부분: {preview})"
            )
        })?;
    // Optional — fail-soft per plan §7. Unparseable/empty JSON → None, which
    // causes the FE to skip the Initial Setup section entirely.
    let initial_setup = extract_section(text, "INITIAL_SETUP")
        .and_then(|raw| {
            let trimmed = strip_code_fences(raw.trim());
            if trimmed.is_empty() { return None; }
            match serde_json::from_str::<serde_json::Value>(trimmed) {
                Ok(v) => {
                    // Treat empty-object sentinel `{}` as "no recommendation".
                    if v.as_object().map(|o| o.is_empty()).unwrap_or(false) { None } else { Some(v) }
                }
                Err(e) => {
                    eprintln!("[onboarding] INITIAL_SETUP parse failed (skipping): {e}");
                    None
                }
            }
        });
    Ok((
        clean_section(&claude_md),
        clean_section(&ref_index),
        initial_setup,
    ))
}

/// 강건화된 마커 추출. `name` 은 "CLAUDE_MD" 같은 베이스 이름 — 함수가 자동으로
/// `[<name>_START]` / `[<name>_END]` 마커 변형을 매칭한다.
///
/// 받아들이는 변형:
/// - `[CLAUDE_MD_START]`           — 정상
/// - `**[CLAUDE_MD_START]**`        — markdown bold (외곽 ** 는 capture 밖)
/// - `[CLAUDE\_MD\_START]`          — underscore escape (markdown 안전 처리)
/// - 마커 좌우 공백 / newline 자유
fn extract_section(text: &str, name: &str) -> Option<String> {
    use regex::Regex;
    // 마커 패턴 — 외곽 markdown bold (`**`) 와 underscore escape 를 허용.
    // `name + "_START"` / `name + "_END"` 전체를 token 화 하여 마지막 underscore 도
    // escape 허용 안에 들어오게 한다.
    let start_token = marker_token_pattern(&format!("{name}_START"));
    let end_token   = marker_token_pattern(&format!("{name}_END"));
    let pattern = format!(
        r"(?s)(?:\*\*)?\[\s*{start_token}\s*\](?:\*\*)?(.*?)(?:\*\*)?\[\s*{end_token}\s*\](?:\*\*)?",
    );
    let re = Regex::new(&pattern).ok()?;
    re.captures(text)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
}

/// 마커 이름 (`CLAUDE_MD_START` 등) 을 regex 패턴화. 언더스코어가 markdown
/// escape (`\_`) 로 들어와도 매칭하도록 허용한다.
fn marker_token_pattern(name: &str) -> String {
    let mut out = String::with_capacity(name.len() * 4);
    for ch in name.chars() {
        if ch == '_' {
            // 백슬래시 0개 또는 1개 + underscore. regex 입력에서 `\\?_` 는
            // "literal backslash optional + underscore" 를 의미.
            out.push_str(r"\\?_");
        } else {
            for esc_ch in regex::escape(&ch.to_string()).chars() {
                out.push(esc_ch);
            }
        }
    }
    out
}

/// 추출된 섹션 본문 정리: trim + 양끝 markdown fence strip + 양끝 markdown
/// emphasis (`**` / `*`) strip.
fn clean_section(body: &str) -> String {
    let t = body.trim();
    let t = strip_code_fences(t);
    let t = strip_emphasis_edges(t);
    t.trim().to_string()
}

/// 본문 양끝에 남은 markdown emphasis 토큰을 제거. 마커가 `**[X_START]**` 식
/// 으로 둘러싸여 있을 때 lazy regex 매칭이 capture 안에 잔재 emphasis 를 남기는
/// 케이스를 흡수한다.
fn strip_emphasis_edges(s: &str) -> &str {
    let mut t = s.trim();
    // 시작 emphasis (앞쪽)
    while let Some(rest) = t.strip_prefix("**").or_else(|| t.strip_prefix('*')) {
        if rest == t { break; }
        t = rest.trim_start_matches('\n').trim_start();
    }
    // 끝 emphasis (뒤쪽)
    while let Some(rest) = t.strip_suffix("**").or_else(|| t.strip_suffix('*')) {
        if rest == t { break; }
        t = rest.trim_end_matches('\n').trim_end();
    }
    t
}

/// ` ```lang\n...\n``` ` 형태의 markdown fence 가 본문 양끝을 감싸고 있으면 제거.
/// 한 번만 strip (중첩은 다루지 않음). 닫는 ``` 가 없으면 원본 유지.
fn strip_code_fences(s: &str) -> &str {
    let trimmed = s.trim();
    if !trimmed.starts_with("```") {
        return trimmed;
    }
    // 첫 줄 ``` 또는 ```lang
    let after_open = match trimmed.find('\n') {
        Some(i) => &trimmed[i + 1..],
        None => return trimmed,
    };
    // 닫는 ``` 위치
    if let Some(close_idx) = after_open.rfind("```") {
        let inner = &after_open[..close_idx];
        return inner.trim_end_matches('\n');
    }
    trimmed
}

/// 사용자 / 로그용 응답 프리뷰 (앞 200 chars, newline → space).
fn response_preview(text: &str) -> String {
    let one_line: String = text.chars().map(|c| if c == '\n' { ' ' } else { c }).collect();
    let trimmed = one_line.trim();
    if trimmed.chars().count() > 200 {
        let mut out: String = trimmed.chars().take(200).collect();
        out.push_str("...");
        out
    } else {
        trimmed.to_string()
    }
}

// ─── AI call (engine-agnostic) ───────────────────────────────────────────────

/// Poll an already-spawned CLI subprocess with cooperative cancellation.
async fn await_cli_with_cancel(
    child: tokio::process::Child,
    cancel: &AtomicBool,
    engine: &str,
) -> Result<String, String> {
    let (tx, mut rx) = tokio::sync::oneshot::channel::<std::io::Result<std::process::Output>>();
    tokio::spawn(async move {
        let _ = tx.send(child.wait_with_output().await);
    });

    let poll = tokio::time::Duration::from_millis(300);
    loop {
        tokio::time::sleep(poll).await;
        if is_cancelled(cancel) { return Err("cancelled".into()); }

        match rx.try_recv() {
            Ok(Ok(output)) => {
                if !output.status.success() {
                    let stderr_body = String::from_utf8_lossy(&output.stderr);
                    let stderr_trimmed = stderr_body.trim();
                    let detail = if stderr_trimmed.is_empty() {
                        "(no stderr)".to_string()
                    } else {
                        stderr_trimmed.to_string()
                    };
                    return Err(format!(
                        "{engine} 분석 실패 (exit: {:?}): {}",
                        output.status.code(),
                        detail
                    ));
                }
                return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
            }
            Ok(Err(e)) => return Err(format!("{engine} 실행 오류: {e}")),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => continue,
            Err(_) => return Err("프로세스 채널 오류".into()),
        }
    }
}

/// Run a CLI agent with (`bin`, model-args) convention. Prompt goes on argv
/// for claude/gemini; for codex we hand it through stdin to match existing
/// codex agent behavior.
async fn call_cli_agent(
    engine: &str,
    bin: &str,
    prompt: &str,
    model: Option<&str>,
    cancel: &AtomicBool,
) -> Result<String, String> {
    use std::process::Stdio;

    let mut cmd = tokio::process::Command::new(bin);
    // Windows 에서 CREATE_NO_WINDOW flag 미적용 시 .cmd wrapper (npm 글로벌
    // 설치 시 표준 형태) 가 새 cmd.exe console 창에 attach 되어 (a) 사용자
    // 에게 보이고 (b) child stdout 이 부모 pipe 로 routing 되지 않아 즉시
    // exit 1 + stderr 빈 채로 실패. no_console.rs 헤더의 invariant ("모든
    // Command::new 직후 .no_console() chain 호출") 누락 회귀.
    cmd.no_console();
    match engine {
        "claude" => {
            cmd.args(["-p", prompt, "--max-turns", "1", "--output-format", "text"]);
            if let Some(m) = model { cmd.args(["--model", m]); }
        }
        "gemini" => {
            cmd.args(["-p", prompt]);
            if let Some(m) = model { cmd.args(["-m", m]); }
        }
        "codex" => {
            cmd.args(["exec", "--full-auto", "-"]);
            if let Some(m) = model { cmd.args(["--model", m]); }
            cmd.stdin(Stdio::piped());
        }
        _ => return Err(format!("지원하지 않는 CLI 엔진: {engine}")),
    }
    // stderr is piped so failure diagnostics surface to the user/log via
    // await_cli_with_cancel (Plan B Task 02 follow-up — codex exit 1 root cause
    // identification). wait_with_output() drains the pipe automatically; success
    // path behavior is unchanged (stderr discarded).
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let spawn_res = cmd.spawn();
    let mut child = spawn_res
        .map_err(|e| format!("{engine} CLI 실행 실패: {e}. {engine}가 설치되어 있는지 확인하세요."))?;

    // codex만 stdin 파이프
    if engine == "codex" {
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let prompt_owned = prompt.to_string();
            tokio::spawn(async move {
                let _ = stdin.write_all(prompt_owned.as_bytes()).await;
                drop(stdin);
            });
        }
    }

    await_cli_with_cancel(child, cancel, engine).await
}

/// Call an OpenAI-compatible HTTP chat completions endpoint (LMStudio default,
/// or Ollama which also exposes /v1/chat/completions).
async fn call_openai_compat(
    prompt: &str,
    model: &str,
    endpoint: &str,   // e.g. http://localhost:1234/v1
    cancel: &AtomicBool,
) -> Result<String, String> {
    let base = endpoint.trim_end_matches('/');
    let url = format!("{}/chat/completions", base);

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "temperature": 0.2,
        "stream": false,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("HTTP 클라이언트 오류: {e}"))?;

    // Spawn HTTP call in a task so we can poll cancel flag.
    let url_c = url.clone();
    let (tx, mut rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    tokio::spawn(async move {
        let res = async {
            let resp = client.post(&url_c).json(&body).send().await
                .map_err(|e| format!("요청 실패: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("HTTP {}", resp.status()));
            }
            let v: serde_json::Value = resp.json().await.map_err(|e| format!("응답 파싱 실패: {e}"))?;
            v.get("choices").and_then(|c| c.get(0)).and_then(|c| c.get("message"))
                .and_then(|m| m.get("content")).and_then(|s| s.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| "choices[0].message.content 필드 누락".into())
        }.await;
        let _ = tx.send(res);
    });

    let poll = tokio::time::Duration::from_millis(300);
    loop {
        tokio::time::sleep(poll).await;
        if is_cancelled(cancel) { return Err("cancelled".into()); }
        match rx.try_recv() {
            Ok(Ok(text)) => return Ok(text),
            Ok(Err(e)) => return Err(e),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => continue,
            Err(_) => return Err("HTTP 채널 오류".into()),
        }
    }
}

/// Ollama-specific native endpoint (non-OpenAI-compat) /api/chat.
async fn call_ollama(
    prompt: &str,
    model: &str,
    endpoint: &str,   // e.g. http://localhost:11434
    cancel: &AtomicBool,
) -> Result<String, String> {
    let base = endpoint.trim_end_matches('/');
    let url = format!("{}/api/chat", base);

    let body = serde_json::json!({
        "model": model,
        "messages": [{ "role": "user", "content": prompt }],
        "stream": false,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| format!("HTTP 클라이언트 오류: {e}"))?;

    let url_c = url.clone();
    let (tx, mut rx) = tokio::sync::oneshot::channel::<Result<String, String>>();
    tokio::spawn(async move {
        let res = async {
            let resp = client.post(&url_c).json(&body).send().await
                .map_err(|e| format!("요청 실패: {e}"))?;
            if !resp.status().is_success() {
                return Err(format!("HTTP {}", resp.status()));
            }
            let v: serde_json::Value = resp.json().await.map_err(|e| format!("응답 파싱 실패: {e}"))?;
            v.get("message").and_then(|m| m.get("content")).and_then(|s| s.as_str())
                .map(|s| s.to_string())
                .ok_or_else(|| "message.content 필드 누락".into())
        }.await;
        let _ = tx.send(res);
    });

    let poll = tokio::time::Duration::from_millis(300);
    loop {
        tokio::time::sleep(poll).await;
        if is_cancelled(cancel) { return Err("cancelled".into()); }
        match rx.try_recv() {
            Ok(Ok(text)) => return Ok(text),
            Ok(Err(e)) => return Err(e),
            Err(tokio::sync::oneshot::error::TryRecvError::Empty) => continue,
            Err(_) => return Err("HTTP 채널 오류".into()),
        }
    }
}

/// Top-level dispatcher: pick CLI vs HTTP path by engine name.
#[allow(clippy::too_many_arguments)]
async fn call_agent(
    engine: &str,
    model: Option<&str>,
    endpoint: Option<&str>,
    prompt: &str,
    cancel: &AtomicBool,
) -> Result<(String, String, Option<serde_json::Value>), String> {
    let text = match engine {
        "claude" | "codex" | "gemini" => {
            call_cli_agent(engine, engine, prompt, model, cancel).await?
        }
        "ollama" => {
            let ep = endpoint.unwrap_or("http://localhost:11434");
            let m = model.ok_or("Ollama 모델이 지정되지 않았습니다")?;
            call_ollama(prompt, m, ep, cancel).await?
        }
        "lmstudio" => {
            let ep = endpoint.unwrap_or("http://localhost:1234/v1");
            let m = model.ok_or("LM Studio 모델이 지정되지 않았습니다")?;
            call_openai_compat(prompt, m, ep, cancel).await?
        }
        other => return Err(format!("지원하지 않는 엔진: {other}")),
    };
    parse_output(&text)
}

// ─── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn analyze_project_for_onboarding(
    project_path: String,
    project_name: String,
    engine: Option<String>,            // default "claude" for backward compat
    model: Option<String>,
    endpoint: Option<String>,          // for ollama/lmstudio
    app: AppHandle,
) -> Result<(), String> {
    let cancel = Arc::new(AtomicBool::new(false));
    set_cancel_flag(cancel.clone());
    let engine = engine.unwrap_or_else(|| "claude".into());

    let emit_step = |step: u8, label: &str, done: bool| {
        app.emit("project:onboarding:step", OnboardingStepPayload {
            step, label: label.to_string(), done,
        }).ok();
    };

    // Step 1: project scan
    emit_step(1, "프로젝트 스캔 중...", false);
    if is_cancelled(&cancel) { clear_cancel_flag(); return Ok(()); }

    let docs_files = scan_docs_files(&project_path);
    let claude_md_path = std::path::Path::new(&project_path).join("CLAUDE.md");
    let existing_claude_md = std::fs::read_to_string(&claude_md_path).ok();
    let has_existing = existing_claude_md.is_some();

    emit_step(1, "프로젝트 스캔 완료", true);

    // Step 2: document analysis
    emit_step(2, "기존 문서 분석 중...", false);
    if is_cancelled(&cancel) { clear_cancel_flag(); return Ok(()); }

    // Small pause so UI can show the step
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
    emit_step(2, "기존 문서 분석 완료", true);

    // Step 3: AI analysis
    emit_step(3, "AI가 정리 중...", false);
    if is_cancelled(&cancel) { clear_cancel_flag(); return Ok(()); }

    let prompt = build_prompt(&project_name, &project_path, &docs_files, &existing_claude_md);

    let agent_result = call_agent(
        &engine,
        model.as_deref(),
        endpoint.as_deref(),
        &prompt,
        &cancel,
    ).await;

    match agent_result {
        Ok((claude_md, ref_index, initial_setup)) => {
            if is_cancelled(&cancel) { clear_cancel_flag(); return Ok(()); }
            emit_step(3, "분석 완료", true);
            app.emit("project:onboarding:preview", OnboardingPreviewPayload {
                claude_md,
                ref_index,
                has_existing_claude_md: has_existing,
                initial_setup,
            }).ok();
        }
        Err(e) if e == "cancelled" => { /* no-op */ }
        Err(e) => {
            app.emit("project:onboarding:error", OnboardingErrorPayload { message: e }).ok();
        }
    }

    clear_cancel_flag();
    Ok(())
}

#[tauri::command]
pub fn cancel_project_onboarding() {
    if let Ok(g) = cancel_flag().lock() {
        if let Some(ref flag) = *g {
            flag.store(true, Ordering::Relaxed);
        }
    }
}

#[tauri::command]
pub fn apply_project_onboarding(
    project_path: String,
    claude_md_content: String,
    ref_index_content: String,
) -> Result<(), String> {
    use std::path::Path;
    let root = Path::new(&project_path);

    std::fs::write(root.join("CLAUDE.md"), &claude_md_content)
        .map_err(|e| format!("CLAUDE.md 쓰기 실패: {e}"))?;

    let ref_path = root.join("docs/reference/index.md");
    if let Some(p) = ref_path.parent() { std::fs::create_dir_all(p).ok(); }
    std::fs::write(&ref_path, &ref_index_content)
        .map_err(|e| format!("docs/reference/index.md 쓰기 실패: {e}"))?;

    Ok(())
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{parse_output, strip_code_fences};

    #[test]
    fn parse_output_legacy_without_initial_setup() {
        let text = r#"
[CLAUDE_MD_START]
# foo
[CLAUDE_MD_END]
[REF_INDEX_START]
# ref
[REF_INDEX_END]
"#;
        let (md, idx, init) = parse_output(text).expect("parse ok");
        assert_eq!(md, "# foo");
        assert_eq!(idx, "# ref");
        assert!(init.is_none());
    }

    #[test]
    fn parse_output_with_initial_setup_json() {
        let text = r#"
[CLAUDE_MD_START]
# foo
[CLAUDE_MD_END]
[REF_INDEX_START]
# ref
[REF_INDEX_END]
[INITIAL_SETUP_START]
{
  "agent_profiles": [
    { "role": "developer", "engine": "codex", "model": "gpt-5-codex", "persona_id": "persona_implementer" }
  ],
  "skills": ["rust-review"],
  "workflow": { "review_track": "deep", "context_mode": "auto", "rt_participants": ["claude", "codex"] },
  "rationale": "Rust 프로젝트"
}
[INITIAL_SETUP_END]
"#;
        let (_md, _idx, init) = parse_output(text).expect("parse ok");
        let v = init.expect("initial_setup present");
        assert_eq!(v["skills"][0], "rust-review");
        assert_eq!(v["workflow"]["review_track"], "deep");
        assert_eq!(v["agent_profiles"][0]["role"], "developer");
    }

    #[test]
    fn parse_output_initial_setup_empty_object_returns_none() {
        let text = r#"
[CLAUDE_MD_START]
a
[CLAUDE_MD_END]
[REF_INDEX_START]
b
[REF_INDEX_END]
[INITIAL_SETUP_START]
{}
[INITIAL_SETUP_END]
"#;
        let (_m, _r, init) = parse_output(text).expect("parse ok");
        assert!(init.is_none(), "empty object should be treated as skip");
    }

    #[test]
    fn parse_output_initial_setup_bad_json_is_ignored() {
        let text = r#"
[CLAUDE_MD_START]
a
[CLAUDE_MD_END]
[REF_INDEX_START]
b
[REF_INDEX_END]
[INITIAL_SETUP_START]
{ this is not json,,, }
[INITIAL_SETUP_END]
"#;
        let (md, idx, init) = parse_output(text).expect("parse still ok because markers OK");
        assert_eq!(md, "a");
        assert_eq!(idx, "b");
        assert!(init.is_none(), "bad JSON should fail-soft to None");
    }

    #[test]
    fn parse_output_missing_claude_md_errors() {
        let text = "[REF_INDEX_START]\nx\n[REF_INDEX_END]\n";
        assert!(parse_output(text).is_err());
    }

    // ─── Layer A 강건화 fixture (codexGeminiOnboardingResponseAudit_2026-04-25) ──

    /// Gemini-style: 마커 앞에 introduction + 응답 전체를 markdown fence 로
    /// 감싸기. 본문 안에 마커가 그대로 남아 있으면 통과해야 한다.
    #[test]
    fn parse_output_gemini_markdown_fence_with_intro() {
        let text = "다음과 같이 정리했습니다:\n\n```markdown\n[CLAUDE_MD_START]\n# foo\n[CLAUDE_MD_END]\n\n[REF_INDEX_START]\n# bar\n[REF_INDEX_END]\n```\n\n추가 도움이 필요하면 말씀해 주세요.";
        let (md, idx, init) = parse_output(text).expect("should parse with intro + fence");
        assert_eq!(md, "# foo");
        assert_eq!(idx, "# bar");
        assert!(init.is_none());
    }

    /// Codex / Claude 가 마커 자체를 markdown bold 로 처리한 케이스.
    #[test]
    fn parse_output_marker_with_bold_emphasis() {
        let text = "**[CLAUDE_MD_START]**\n# foo\n**[CLAUDE_MD_END]**\n\n**[REF_INDEX_START]**\n# bar\n**[REF_INDEX_END]**";
        let (md, idx, _init) = parse_output(text).expect("bold marker should parse");
        assert_eq!(md, "# foo");
        assert_eq!(idx, "# bar");
    }

    /// 한국어 모델이 underscore 를 markdown escape (`\_`) 로 변환한 케이스.
    #[test]
    fn parse_output_marker_with_underscore_escape() {
        let text = r#"[CLAUDE\_MD\_START]
# foo
[CLAUDE\_MD\_END]
[REF\_INDEX\_START]
# bar
[REF\_INDEX\_END]
"#;
        let (md, idx, _init) = parse_output(text).expect("escaped underscore should parse");
        assert_eq!(md, "# foo");
        assert_eq!(idx, "# bar");
    }

    /// 마커 양옆에 공백 포함 (`[ CLAUDE_MD_START ]`).
    #[test]
    fn parse_output_marker_with_inner_whitespace() {
        let text = "[ CLAUDE_MD_START ]\n# foo\n[ CLAUDE_MD_END ]\n[ REF_INDEX_START ]\n# bar\n[ REF_INDEX_END ]";
        let (md, idx, _init) = parse_output(text).expect("padded marker should parse");
        assert_eq!(md, "# foo");
        assert_eq!(idx, "# bar");
    }

    /// 마커 매칭 실패 시 raw 응답의 앞 200자가 에러 메시지에 포함되어야 한다.
    #[test]
    fn parse_output_failure_includes_response_preview() {
        let text = "Sure! Here is the answer to your question:\n\nThe project looks like a Rust app...";
        let err = parse_output(text).expect_err("should fail without markers");
        assert!(
            err.contains("Sure! Here is the answer"),
            "preview missing in error: {err}"
        );
    }

    /// 추출된 INITIAL_SETUP JSON 본문이 ```json fence 안에 들어 있어도 통과해야 한다.
    #[test]
    fn parse_output_initial_setup_inside_fence() {
        let text = r#"[CLAUDE_MD_START]
a
[CLAUDE_MD_END]
[REF_INDEX_START]
b
[REF_INDEX_END]
[INITIAL_SETUP_START]
```json
{
  "agent_profiles": [],
  "skills": ["rust-review"],
  "workflow": { "review_track": "deep", "context_mode": "auto", "rt_participants": [] },
  "rationale": "fence 안에 든 JSON"
}
```
[INITIAL_SETUP_END]
"#;
        let (_md, _idx, init) = parse_output(text).expect("fenced JSON should parse");
        let v = init.expect("initial_setup present");
        assert_eq!(v["skills"][0], "rust-review");
        assert_eq!(v["workflow"]["review_track"], "deep");
    }

    /// `strip_code_fences` 단위 동작.
    #[test]
    fn strip_code_fences_basic() {
        assert_eq!(strip_code_fences("plain"), "plain");
        assert_eq!(strip_code_fences("```\nhello\n```"), "hello");
        assert_eq!(strip_code_fences("```json\n{\"a\":1}\n```"), "{\"a\":1}");
        // 닫는 ``` 가 없으면 원본 유지
        assert_eq!(strip_code_fences("```\nhello"), "```\nhello");
    }
}
