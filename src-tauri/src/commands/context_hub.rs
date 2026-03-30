//! Tauri commands for context-hub integration.
//!
//! Exposes health, search, and get as Tauri commands.
//! Source policy is enforced at the agent layer — public auto-fetch is blocked.

use serde::Serialize;
use crate::agents::context_hub;
use crate::errors::AppError;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubHealthResult {
    pub available: bool,
    pub version: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubSearchResult {
    pub id: String,
    pub title: String,
    pub source: String,
    pub snippet: String,
    pub score: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HubDocument {
    pub id: String,
    pub title: String,
    pub content: String,
    pub source: String,
}

/// Check context-hub availability.
#[tauri::command]
pub fn context_hub_health() -> HubHealthResult {
    let status = context_hub::health();
    HubHealthResult {
        available: status.available,
        version: status.version,
        message: status.message,
    }
}

/// Search context-hub knowledge sources (policy-constrained).
#[tauri::command]
pub fn context_hub_search(
    query: String,
    source_filter: Option<String>,
    limit: Option<usize>,
) -> Result<Vec<HubSearchResult>, AppError> {
    let results = context_hub::search(&query, source_filter.as_deref(), limit.unwrap_or(10))
        .map_err(|e| AppError::Agent(e.to_string()))?;

    Ok(results.into_iter().map(|r| HubSearchResult {
        id: r.id,
        title: r.title,
        source: r.source,
        snippet: r.snippet,
        score: r.score,
    }).collect())
}

/// Get a document from context-hub by ID.
#[tauri::command]
pub fn context_hub_get(document_id: String) -> Result<HubDocument, AppError> {
    let doc = context_hub::get(&document_id)
        .map_err(|e| AppError::Agent(e.to_string()))?;

    Ok(HubDocument {
        id: doc.id,
        title: doc.title,
        content: doc.content,
        source: doc.source,
    })
}
