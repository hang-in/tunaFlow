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
use std::sync::Mutex;
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
            ("claude-sonnet-4-6", "Sonnet 4.6", false),
            ("claude-opus-4-6", "Opus 4.6", false),
            ("claude-haiku-4-5-20251001", "Haiku 4.5", true),
            ("sonnet", "Sonnet (latest)", false),
            ("opus", "Opus (latest)", false),
            ("haiku", "Haiku (latest)", false),
        ],
        "codex" => vec![
            ("o3-mini", "o3-mini", true),
            ("gpt-4o", "GPT-4o", false),
            ("gpt-4o-mini", "GPT-4o Mini", false),
            ("o3", "o3", false),
            ("o4-mini", "o4-mini", false),
        ],
        "gemini" => vec![
            ("gemini-2.5-pro", "Gemini 2.5 Pro", true),
            ("gemini-2.5-flash", "Gemini 2.5 Flash", false),
            ("gemini-2.0-flash", "Gemini 2.0 Flash", false),
        ],
        "opencode" => vec![
            ("anthropic:claude-sonnet-4-6", "Claude Sonnet 4.6", true),
            ("openai:gpt-4.1", "GPT-4.1", false),
        ],
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
    let script = r#"
try {
    const path = require('path');
    const appdata = process.env.APPDATA || path.join(require('os').homedir(), '.npm-global');
    const corePath = path.join(appdata, 'npm/node_modules/@google/gemini-cli/node_modules/@google/gemini-cli-core');
    const core = require(corePath);
    const models = [];
    const keys = Object.keys(core).filter(k => k.includes('GEMINI') && k.includes('MODEL') && !k.includes('ALIAS') && !k.includes('EMBEDDING') && !k.includes('AUTO'));
    keys.forEach(k => {
        const v = core[k];
        if (typeof v === 'string' && v.startsWith('gemini-') && !v.includes('lite') && !v.includes('customtools')) models.push(v);
    });
    console.log(JSON.stringify(models));
} catch(e) {
    console.log('[]');
}
"#;
    let output = std::process::Command::new("node")
        .args(["-e", script])
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

// ─── Core API ───────────────────────────────────────────────────────────────

const ENGINES: &[&str] = &["claude", "codex", "gemini", "opencode"];

fn get_models_for_engine(engine: &str, force: bool) -> (Vec<String>, String) {
    // Check cache
    if !force {
        if let Ok(cache) = MODEL_CACHE.lock() {
            if let Some(entry) = cache.get(engine) {
                if entry.at.elapsed() < CACHE_TTL {
                    return (entry.models.clone(), entry.source.clone());
                }
            }
        }
    }

    // Try discovery
    let discovered = match engine {
        "codex" => discover_codex(),
        "gemini" => discover_gemini(),
        "claude" => discover_claude(),
        _ => None,
    };

    if let Some(models) = discovered {
        let source = "discovered".to_string();
        if let Ok(mut cache) = MODEL_CACHE.lock() {
            cache.insert(engine.to_string(), CacheEntry {
                models: models.clone(), source: source.clone(), at: Instant::now(),
            });
        }
        return (models, source);
    }

    // Fallback
    let fb = fallback_models(engine);
    let models: Vec<String> = fb.iter().map(|(id, _, _)| id.to_string()).collect();
    let source = "fallback".to_string();
    if let Ok(mut cache) = MODEL_CACHE.lock() {
        cache.insert(engine.to_string(), CacheEntry {
            models: models.clone(), source: source.clone(), at: Instant::now(),
        });
    }
    (models, source)
}

fn model_label(engine: &str, id: &str) -> String {
    // Check fallback registry for label
    for (fid, label, _) in fallback_models(engine) {
        if fid == id { return label.to_string(); }
    }
    // Auto-generate from id
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
    if let Ok(mut cache) = MODEL_CACHE.lock() {
        match engine {
            Some(e) => { cache.remove(e); }
            None => { cache.clear(); }
        }
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
