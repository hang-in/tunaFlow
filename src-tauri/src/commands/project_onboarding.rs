use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

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
                format!("{}...(truncated)", &content[..3000])
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

## 출력 형식

아래 두 섹션을 정확한 마커와 함께 출력하세요. 마커 외에 다른 텍스트는 추가하지 마세요.

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

fn parse_output(text: &str) -> Result<(String, String), String> {
    let claude_md = extract_between(text, "[CLAUDE_MD_START]", "[CLAUDE_MD_END]")
        .ok_or("AI 응답에서 CLAUDE.md 섹션을 찾을 수 없습니다")?;
    let ref_index = extract_between(text, "[REF_INDEX_START]", "[REF_INDEX_END]")
        .ok_or("AI 응답에서 Reference Index 섹션을 찾을 수 없습니다")?;
    Ok((claude_md.trim().to_string(), ref_index.trim().to_string()))
}

fn extract_between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let s = text.find(start)? + start.len();
    let e = text[s..].find(end)? + s;
    Some(&text[s..e])
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
                    return Err(format!("{engine} 분석 실패 (exit: {:?})", output.status.code()));
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
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());

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
async fn call_agent(
    engine: &str,
    model: Option<&str>,
    endpoint: Option<&str>,
    prompt: &str,
    cancel: &AtomicBool,
) -> Result<(String, String), String> {
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
        Ok((claude_md, ref_index)) => {
            if is_cancelled(&cancel) { clear_cancel_flag(); return Ok(()); }
            emit_step(3, "분석 완료", true);
            app.emit("project:onboarding:preview", OnboardingPreviewPayload {
                claude_md,
                ref_index,
                has_existing_claude_md: has_existing,
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
