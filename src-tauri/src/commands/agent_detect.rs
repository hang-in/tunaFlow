//! Detect which AI agents are available on this machine.
//!
//! Used by the Meta Agent Selector modal shown during project onboarding.
//! - CLI agents (claude / codex / gemini): probe via `which` + `--version`
//! - HTTP agents (ollama / lmstudio): probe endpoint + list models live
//! - 모든 탐지는 병렬로 수행. 각 항목은 1.5s timeout.

use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

const PROBE_TIMEOUT_MS: u64 = 1500;

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AgentDetection {
    pub engine: String,
    pub kind: &'static str,        // "cli" or "http"
    pub installed: bool,
    pub version: Option<String>,   // CLI 에이전트의 `<cmd> --version`
    pub path: Option<String>,      // CLI: `which` 결과
    pub endpoint: Option<String>,  // HTTP: 확인/호출된 베이스 URL
    pub models: Vec<String>,       // HTTP: /api/tags / /v1/models
    pub note: Option<String>,      // 실패/에러 요약 (사용자에게 보여줄 수 있음)
}

// ─── CLI probing ─────────────────────────────────────────────────────────────

async fn probe_cli(engine: &str, bin: &str, version_args: &[&str]) -> AgentDetection {
    let mut det = AgentDetection {
        engine: engine.to_string(),
        kind: "cli",
        installed: false,
        version: None,
        path: None,
        endpoint: None,
        models: vec![],
        note: None,
    };

    // `which <bin>`
    let which_fut = Command::new("which").arg(bin).output();
    let which_out = match timeout(Duration::from_millis(PROBE_TIMEOUT_MS), which_fut).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => { det.note = Some(format!("which error: {e}")); return det; }
        Err(_) => { det.note = Some("which timeout".into()); return det; }
    };
    if !which_out.status.success() {
        det.note = Some("not found in PATH".into());
        return det;
    }
    let path = String::from_utf8_lossy(&which_out.stdout).trim().to_string();
    if path.is_empty() {
        det.note = Some("which returned empty".into());
        return det;
    }
    det.path = Some(path.clone());
    det.installed = true;

    // `<bin> --version` (optional — 실패해도 installed 유지)
    let ver_fut = Command::new(&path).args(version_args).output();
    if let Ok(Ok(out)) = timeout(Duration::from_millis(PROBE_TIMEOUT_MS), ver_fut).await {
        if out.status.success() {
            let v = String::from_utf8_lossy(&out.stdout).lines().next().unwrap_or("").trim().to_string();
            if !v.is_empty() { det.version = Some(v); }
        }
    }
    det
}

// ─── HTTP probing ────────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct OllamaTagsResponse {
    #[serde(default)]
    models: Vec<OllamaTagModel>,
}
#[derive(Deserialize)]
struct OllamaTagModel { name: String }

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    #[serde(default)]
    data: Vec<OpenAiModel>,
}
#[derive(Deserialize)]
struct OpenAiModel { id: String }

async fn probe_ollama(endpoint: &str) -> AgentDetection {
    let base = endpoint.trim_end_matches('/');
    let url = format!("{}/api/tags", base);
    let mut det = AgentDetection {
        engine: "ollama".into(),
        kind: "http",
        installed: false,
        version: None,
        path: None,
        endpoint: Some(base.to_string()),
        models: vec![],
        note: None,
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(PROBE_TIMEOUT_MS))
        .build()
    {
        Ok(c) => c,
        Err(e) => { det.note = Some(format!("reqwest build error: {e}")); return det; }
    };

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<OllamaTagsResponse>().await {
                Ok(body) => {
                    det.installed = true;
                    det.models = body.models.into_iter().map(|m| m.name).collect();
                }
                Err(e) => det.note = Some(format!("parse error: {e}")),
            }
        }
        Ok(resp) => det.note = Some(format!("status {}", resp.status())),
        Err(_) => det.note = Some("not reachable".into()),
    }
    det
}

async fn probe_lmstudio(endpoint: &str) -> AgentDetection {
    // LMStudio는 보통 .../v1 로 base. 끝에 /v1 붙어있지 않으면 붙여서 접근.
    let base_raw = endpoint.trim_end_matches('/');
    let base = if base_raw.ends_with("/v1") { base_raw.to_string() } else { format!("{}/v1", base_raw) };
    let url = format!("{}/models", base);

    let mut det = AgentDetection {
        engine: "lmstudio".into(),
        kind: "http",
        installed: false,
        version: None,
        path: None,
        endpoint: Some(base.clone()),
        models: vec![],
        note: None,
    };

    let client = match reqwest::Client::builder()
        .timeout(Duration::from_millis(PROBE_TIMEOUT_MS))
        .build()
    {
        Ok(c) => c,
        Err(e) => { det.note = Some(format!("reqwest build error: {e}")); return det; }
    };

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            match resp.json::<OpenAiModelsResponse>().await {
                Ok(body) => {
                    det.installed = true;
                    det.models = body.data.into_iter().map(|m| m.id).collect();
                }
                Err(e) => det.note = Some(format!("parse error: {e}")),
            }
        }
        Ok(resp) => det.note = Some(format!("status {}", resp.status())),
        Err(_) => det.note = Some("not reachable".into()),
    }
    det
}

// ─── Tauri command ───────────────────────────────────────────────────────────

#[tauri::command]
pub async fn detect_available_agents(
    ollama_endpoint: Option<String>,
    lmstudio_endpoint: Option<String>,
) -> Vec<AgentDetection> {
    let ollama_ep = ollama_endpoint.unwrap_or_else(|| "http://localhost:11434".into());
    let lmstudio_ep = lmstudio_endpoint.unwrap_or_else(|| "http://localhost:1234/v1".into());

    // CLI probes — 병렬
    let (claude, codex, gemini, ollama, lmstudio) = tokio::join!(
        probe_cli("claude", "claude", &["--version"]),
        probe_cli("codex",  "codex",  &["--version"]),
        probe_cli("gemini", "gemini", &["--version"]),
        probe_ollama(&ollama_ep),
        probe_lmstudio(&lmstudio_ep),
    );

    vec![claude, codex, gemini, ollama, lmstudio]
}
