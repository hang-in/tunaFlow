//! Engine model discovery — dynamic detection + fallback registry.
//!
//! Discovers available models from each engine's local sources:
//! - **Codex**: reads `~/.codex/models_cache.json`
//! - **Gemini**: reads constants from installed `@google/gemini-cli-core` npm package
//! - **Claude**: fallback static list (no local model cache)
//! - **OpenCode**: fallback static list
//!
//! Results are cached in-process with TTL. Invalidated by `refresh_engine_models`.

use serde::Serialize;
use std::collections::HashMap;
use parking_lot::Mutex;
use std::time::{Duration, Instant};

// ─── Types ───────────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct EngineModel {
    pub id: String,
    pub label: String,
    pub engine: String,
    pub recommended: bool,
    pub source: String,
}

struct CacheEntry {
    models: Vec<String>,
    source: String,
    at: Instant,
}

// ─── Global cache ────────────────────────────────────────────────────────────

static CACHE_TTL: Duration = Duration::from_secs(3600);

lazy_static::lazy_static! {
    static ref MODEL_CACHE: Mutex<HashMap<String, CacheEntry>> = Mutex::new(HashMap::new());
}

// ─── Fallback registry ──────────────────────────────────────────────────────

fn fallback_models(engine: &str) -> Vec<(&'static str, &'static str, bool)> {
    match engine {
        "claude" => vec![
            ("claude-sonnet-4-6", "Sonnet 4.6", true),
            ("claude-opus-4-6", "Opus 4.6", false),
            ("claude-haiku-4-5-20251001", "Haiku 4.5", false),
            ("sonnet", "Sonnet (latest)", false),
            ("opus", "Opus (latest)", false),
            ("haiku", "Haiku (latest)", false),
        ],
        "codex" => vec![
            ("gpt-5.4-mini", "GPT-5.4 Mini", true),
            ("gpt-5.4", "GPT-5.4", false),
            ("gpt-5.3-codex", "GPT-5.3 Codex", false),
            ("gpt-5.2-codex", "GPT-5.2 Codex", false),
            ("gpt-5.1-codex-mini", "GPT-5.1 Codex Mini", false),
            ("o3-mini", "o3-mini", false),
        ],
        "gemini" => vec![
            ("auto", "Auto (Gemini CLI default)", true),
            ("gemini-2.5-pro", "Gemini 2.5 Pro", false),
            ("gemini-2.5-flash", "Gemini 2.5 Flash", false),
            ("gemini-2.5-flash-lite", "Gemini 2.5 Flash Lite", false),
            ("gemini-3-pro-preview", "Gemini 3 Pro (preview, 용량 미보장)", false),
            ("gemini-3-flash-preview", "Gemini 3 Flash (preview, 용량 미보장)", false),
            ("gemini-3.1-pro-preview", "Gemini 3.1 Pro (preview, 용량 미보장)", false),
            ("gemini-3.1-flash-lite-preview", "Gemini 3.1 Flash Lite (preview, 용량 미보장)", false),
        ],
        "ollama" => vec![
            ("qwen3:8b", "Qwen 3 8B", true),
            ("llama3.3:latest", "Llama 3.3", false),
            ("gemma3:12b", "Gemma 3 12B", false),
            ("phi-4:latest", "Phi-4", false),
        ],
        "lmstudio" => vec![],  // LM Studio models are always discovered live
        _ => vec![],
    }
}

// ─── Discovery functions ────────────────────────────────────────────────────

/// Codex: read `~/.codex/models_cache.json`
fn discover_codex() -> Option<Vec<String>> {
    let cache_path = dirs::home_dir()?.join(".codex").join("models_cache.json");
    if !cache_path.exists() {
        return None;
    }
    let text = std::fs::read_to_string(&cache_path).ok()?;
    let data: serde_json::Value = serde_json::from_str(&text).ok()?;
    let models_arr = data.get("models")?.as_array()?;
    let mut models = Vec::new();
    for m in models_arr {
        let slug = m.get("slug").and_then(|v| v.as_str()).unwrap_or("");
        let vis = m.get("visibility").and_then(|v| v.as_str()).unwrap_or("");
        if !slug.is_empty() && vis != "hide" {
            models.push(slug.to_string());
        }
    }
    if models.is_empty() { None } else { Some(models) }
}

/// Gemini: read constants from installed npm package via node
fn discover_gemini() -> Option<Vec<String>> {
    // Step 1: find the global node_modules root via `npm root -g`
    let npm_root = std::process::Command::new("npm")
        .args(["root", "-g"])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
            } else {
                None
            }
        });

    // Build candidate paths: npm root -g result, then legacy fallback
    let mut candidates = Vec::new();
    if let Some(root) = &npm_root {
        candidates.push(format!(
            "{root}/@google/gemini-cli/node_modules/@google/gemini-cli-core"
        ));
    }
    // Legacy fallback paths
    if let Some(home) = dirs::home_dir() {
        let home = home.display();
        candidates.push(format!(
            "{home}/.npm-global/npm/node_modules/@google/gemini-cli/node_modules/@google/gemini-cli-core"
        ));
        #[cfg(target_os = "windows")]
        if let Ok(appdata) = std::env::var("APPDATA") {
            candidates.push(format!(
                "{appdata}/npm/node_modules/@google/gemini-cli/node_modules/@google/gemini-cli-core"
            ));
        }
    }

    let paths_json = serde_json::to_string(&candidates).unwrap_or_else(|_| "[]".to_string());

    let script = format!(r#"
const paths = {paths_json};
for (const p of paths) {{
    try {{
        const core = require(p);
        const models = [];
        const keys = Object.keys(core).filter(k =>
            k.includes('GEMINI') && k.includes('MODEL') &&
            !k.includes('ALIAS') && !k.includes('EMBEDDING') && !k.includes('AUTO')
        );
        keys.forEach(k => {{
            const v = core[k];
            if (typeof v === 'string' && v.startsWith('gemini-') && !v.includes('customtools'))
                models.push(v);
        }});
        if (models.length > 0) {{
            console.log(JSON.stringify([...new Set(models)]));
            process.exit(0);
        }}
    }} catch(_) {{}}
}}
console.log('[]');
"#);

    let output = std::process::Command::new("node")
        .args(["-e", &script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let models: Vec<String> = serde_json::from_str(stdout.trim()).ok()?;
    if models.is_empty() { None } else { Some(models) }
}

/// Claude: no local model cache — always fallback
fn discover_claude() -> Option<Vec<String>> {
    None
}

/// LMStudio: query OpenAI-compatible `/v1/models` endpoint.
fn discover_lmstudio() -> Option<Vec<String>> {
    let endpoint = std::env::var("LMSTUDIO_ENDPOINT")
        .unwrap_or_else(|_| "http://localhost:1234".into());
    let url = format!("{}/v1/models", endpoint.trim_end_matches('/'));

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .ok()?;

    let mut req = client.get(&url);
    if let Ok(token) = std::env::var("LMSTUDIO_API_KEY") {
        req = req.header("Authorization", format!("Bearer {}", token));
    }
    let resp = req.send().ok()?;
    if !resp.status().is_success() {
        eprintln!("[model_discovery] lmstudio {} → {}", url, resp.status());
        return None;
    }

    let body: serde_json::Value = resp.json().ok()?;
    let data = body.get("data")?.as_array()?;
    let models: Vec<String> = data.iter()
        .filter_map(|m| m.get("id").and_then(|v| v.as_str()).map(String::from))
        .collect();

    if models.is_empty() { None } else { Some(models) }
}

// OpenCode discovery removed — engine dropped from active ENGINES list.

// ─── Core API ───────────────────────────────────────────────────────────────

const ENGINES: &[&str] = &["claude", "codex", "gemini", "ollama", "lmstudio"];

fn get_models_for_engine(engine: &str, force: bool) -> (Vec<String>, String) {
    // Check cache
    if !force {
        let cache = MODEL_CACHE.lock();
        if let Some(entry) = cache.get(engine) {
            if entry.at.elapsed() < CACHE_TTL {
                return (entry.models.clone(), entry.source.clone());
            }
        }
    }

    // Try discovery
    let discovered = match engine {
        "codex" => discover_codex(),
        "gemini" => discover_gemini(),
        "claude" => discover_claude(),
        "ollama" => crate::agents::openai_compat::discover_models(),
        "lmstudio" => discover_lmstudio(),
        _ => None,
    };

    if let Some(mut models) = discovered {
        // Gemini: prepend "auto" option for CLI default routing
        if engine == "gemini" && !models.contains(&"auto".to_string()) {
            models.insert(0, "auto".to_string());
        }
        let source = "discovered".to_string();
        let mut cache = MODEL_CACHE.lock();
        cache.insert(engine.to_string(), CacheEntry {
            models: models.clone(), source: source.clone(), at: Instant::now(),
        });
        return (models, source);
    }

    // Fallback
    let fb = fallback_models(engine);
    let models: Vec<String> = fb.iter().map(|(id, _, _)| id.to_string()).collect();
    let source = "fallback".to_string();
    let mut cache = MODEL_CACHE.lock();
    cache.insert(engine.to_string(), CacheEntry {
        models: models.clone(), source: source.clone(), at: Instant::now(),
    });
    (models, source)
}

fn model_label(engine: &str, id: &str) -> String {
    // Check fallback registry for label
    for (fid, label, _) in fallback_models(engine) {
        if fid == id { return label.to_string(); }
    }
    id.to_string()
}

fn model_recommended(engine: &str, id: &str) -> bool {
    for (fid, _, rec) in fallback_models(engine) {
        if fid == id { return rec; }
    }
    false
}

/// Invalidate cache for all engines or a specific one.
pub fn invalidate_cache(engine: Option<&str>) {
    let mut cache = MODEL_CACHE.lock();
    match engine {
        Some(e) => { cache.remove(e); }
        None => { cache.clear(); }
    }
}

// ─── Tauri commands ─────────────────────────────────────────────────────────

/// Return all engine models — discovery + fallback.
#[tauri::command]
pub fn list_engine_models() -> Vec<EngineModel> {
    let mut catalog = Vec::new();
    for engine in ENGINES {
        let (models, source) = get_models_for_engine(engine, false);
        for id in &models {
            catalog.push(EngineModel {
                id: id.clone(),
                label: model_label(engine, id),
                engine: engine.to_string(),
                recommended: model_recommended(engine, id),
                source: source.clone(),
            });
        }
    }
    catalog
}

/// Invalidate model cache and re-discover.
#[tauri::command]
pub fn refresh_engine_models() -> Vec<EngineModel> {
    invalidate_cache(None);
    let mut catalog = Vec::new();
    for engine in ENGINES {
        let (models, source) = get_models_for_engine(engine, true);
        for id in &models {
            catalog.push(EngineModel {
                id: id.clone(),
                label: model_label(engine, id),
                engine: engine.to_string(),
                recommended: model_recommended(engine, id),
                source: source.clone(),
            });
        }
    }
    catalog
}
