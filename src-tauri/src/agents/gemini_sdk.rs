//! Gemini SDK — direct HTTP API integration (replaces CLI subprocess).
//!
//! Uses Google AI Gemini REST API with SSE streaming.
//! Requires GEMINI_API_KEY environment variable.

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;

const API_BASE: &str = "https://generativelanguage.googleapis.com/v1beta";

/// Check if Gemini SDK can be used (API key available)
pub fn is_available() -> bool {
    std::env::var("GEMINI_API_KEY").is_ok()
}

/// Streaming Gemini API call with SSE parsing.
pub async fn stream_run<F, G>(
    input: RunInput,
    mut on_progress: G,
    mut on_chunk: F,
) -> Result<RunOutput, AppError>
where
    F: FnMut(String) + Send,
    G: FnMut(String) + Send,
{
    let api_key = std::env::var("GEMINI_API_KEY")
        .map_err(|_| AppError::Agent("GEMINI_API_KEY not set".into()))?;

    let model = input.model.as_deref().unwrap_or("gemini-2.5-flash");
    let url = format!(
        "{}/models/{}:streamGenerateContent?key={}&alt=sse",
        API_BASE, model, api_key
    );

    // Build request body
    let contents = vec![Content {
        role: "user".into(),
        parts: vec![Part { text: Some(input.prompt.clone()) }],
    }];

    let system_instruction = input.system_prompt.as_ref().map(|sp| Content {
        role: "user".into(),
        parts: vec![Part { text: Some(sp.clone()) }],
    });

    // Add workflow tools for function calling
    let tools = crate::agents::tool_handler::workflow_tools();
    let tools_json = crate::agents::tool_handler::to_gemini_tools(&tools);

    let body = GenerateContentRequest {
        contents,
        system_instruction,
        generation_config: Some(GenerationConfig {
            temperature: Some(1.0),
            max_output_tokens: Some(65536),
        }),
        tools: Some(tools_json),
    };

    on_progress("Gemini SDK initializing...".into());

    let client = Client::new();
    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Agent(format!("Gemini API request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::Agent(format!(
            "Gemini API error {}: {}",
            status,
            &error_text[..error_text.len().min(500)]
        )));
    }

    // Parse SSE stream
    let mut full_text = String::new();
    let mut input_tokens: i64 = 0;
    let mut output_tokens: i64 = 0;

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result
            .map_err(|e| AppError::Agent(format!("Stream read error: {}", e)))?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_str);

        // SSE format: "data: {json}\n\n"
        while let Some(data_pos) = buffer.find("data: ") {
            let json_start = data_pos + 6;
            if let Some(end_pos) = buffer[json_start..].find("\n") {
                let json_str = &buffer[json_start..json_start + end_pos];

                if let Ok(response) = serde_json::from_str::<StreamResponse>(json_str) {
                    // Extract text from candidates
                    if let Some(candidates) = &response.candidates {
                        for candidate in candidates {
                            if let Some(content) = &candidate.content {
                                for part in &content.parts {
                                    if let Some(text) = &part.text {
                                        full_text.push_str(text);
                                        on_chunk(full_text.clone());
                                    }
                                }
                            }
                        }
                    }

                    // Extract usage metadata
                    if let Some(usage) = &response.usage_metadata {
                        if let Some(pt) = usage.prompt_token_count {
                            input_tokens = pt;
                        }
                        if let Some(ct) = usage.candidates_token_count {
                            output_tokens = ct;
                        }
                    }
                }

                buffer = buffer[json_start + end_pos..].to_string();
            } else {
                break; // incomplete SSE line, wait for more data
            }
        }
    }

    if full_text.is_empty() {
        return Err(AppError::Agent("Gemini returned empty response".into()));
    }

    Ok(RunOutput {
        content: full_text,
        cost_usd: estimate_cost(model, input_tokens, output_tokens),
        input_tokens,
        output_tokens,
        session_id: None,
    })
}

/// Synchronous wrapper for non-streaming use (eval, etc.)
#[allow(dead_code)]
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| AppError::Agent("No tokio runtime".into()))?;

    rt.block_on(async {
        let mut last_text = String::new();
        stream_run(
            input,
            |_| {},
            |t| { last_text = t; },
        ).await
    })
}

/// Rough cost estimate based on model and token counts
fn estimate_cost(model: &str, input_tokens: i64, output_tokens: i64) -> f64 {
    // Pricing per 1M tokens (approximate, as of 2026)
    let (input_price, output_price) = if model.contains("pro") {
        (1.25, 5.0)    // Gemini Pro
    } else if model.contains("flash") {
        (0.075, 0.30)  // Gemini Flash
    } else {
        (0.50, 1.50)   // Default estimate
    };
    (input_tokens as f64 * input_price + output_tokens as f64 * output_price) / 1_000_000.0
}

// ─── Request/Response types ─────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerateContentRequest {
    contents: Vec<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system_instruction: Option<Content>,
    #[serde(skip_serializing_if = "Option::is_none")]
    generation_config: Option<GenerationConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize)]
struct Content {
    role: String,
    parts: Vec<Part>,
}

#[derive(Serialize, Deserialize)]
struct Part {
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GenerationConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_output_tokens: Option<u32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct StreamResponse {
    candidates: Option<Vec<Candidate>>,
    usage_metadata: Option<UsageMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Candidate {
    content: Option<Content>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct UsageMetadata {
    prompt_token_count: Option<i64>,
    candidates_token_count: Option<i64>,
    total_token_count: Option<i64>,
    cached_content_token_count: Option<i64>,
}
