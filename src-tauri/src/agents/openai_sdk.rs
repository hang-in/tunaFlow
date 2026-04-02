//! OpenAI SDK — direct HTTP API integration (replaces Codex CLI subprocess).
//!
//! Uses OpenAI Chat Completions API with SSE streaming.
//! Requires OPENAI_API_KEY environment variable.

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;

const API_BASE: &str = "https://api.openai.com/v1";

/// Check if OpenAI SDK can be used (API key available)
pub fn is_available() -> bool {
    std::env::var("OPENAI_API_KEY").is_ok()
}

/// Streaming OpenAI Chat Completions API call with SSE parsing.
pub async fn stream_run<F, G>(
    input: RunInput,
    mut on_progress: G,
    mut on_chunk: F,
) -> Result<RunOutput, AppError>
where
    F: FnMut(String) + Send,
    G: FnMut(String) + Send,
{
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| AppError::Agent("OPENAI_API_KEY not set".into()))?;

    let model = input.model.as_deref().unwrap_or("gpt-4o");
    let url = format!("{}/chat/completions", API_BASE);

    // Build messages
    let mut messages: Vec<ChatMessage> = Vec::new();

    if let Some(sp) = &input.system_prompt {
        messages.push(ChatMessage {
            role: "system".into(),
            content: sp.clone(),
        });
    }

    messages.push(ChatMessage {
        role: "user".into(),
        content: input.prompt.clone(),
    });

    let tools = crate::agents::tool_handler::workflow_tools();
    let tools_json = crate::agents::tool_handler::to_openai_tools(&tools);

    let body = ChatCompletionRequest {
        model: model.to_string(),
        messages,
        stream: true,
        max_tokens: Some(16384),
        temperature: Some(1.0),
        tools: Some(tools_json),
    };

    on_progress("OpenAI SDK initializing...".into());

    let client = Client::new();
    let response = client
        .post(&url)
        .header("Authorization", format!("Bearer {}", api_key))
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Agent(format!("OpenAI API request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::Agent(format!(
            "OpenAI API error {}: {}",
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

        // SSE format: "data: {json}\n\n" or "data: [DONE]\n\n"
        while let Some(data_pos) = buffer.find("data: ") {
            let json_start = data_pos + 6;
            if let Some(end_pos) = buffer[json_start..].find("\n") {
                let data_str = buffer[json_start..json_start + end_pos].trim();

                if data_str == "[DONE]" {
                    buffer = buffer[json_start + end_pos..].to_string();
                    continue;
                }

                if let Ok(chunk) = serde_json::from_str::<StreamChunk>(data_str) {
                    for choice in &chunk.choices {
                        if let Some(content) = &choice.delta.content {
                            full_text.push_str(content);
                            on_chunk(full_text.clone());
                        }
                    }
                    // Usage comes in the final chunk
                    if let Some(usage) = &chunk.usage {
                        input_tokens = usage.prompt_tokens;
                        output_tokens = usage.completion_tokens;
                    }
                }

                buffer = buffer[json_start + end_pos..].to_string();
            } else {
                break; // incomplete line
            }
        }
    }

    if full_text.is_empty() {
        return Err(AppError::Agent("OpenAI returned empty response".into()));
    }

    Ok(RunOutput {
        content: full_text,
        cost_usd: estimate_cost(model, input_tokens, output_tokens),
        input_tokens,
        output_tokens,
        session_id: None,
    })
}

/// Synchronous wrapper
#[allow(dead_code)]
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| AppError::Agent("No tokio runtime".into()))?;
    rt.block_on(async {
        stream_run(input, |_| {}, |_| {}).await
    })
}

fn estimate_cost(model: &str, input_tokens: i64, output_tokens: i64) -> f64 {
    let (input_price, output_price) = if model.contains("gpt-4o-mini") {
        (0.15, 0.60)
    } else if model.contains("gpt-4o") {
        (2.50, 10.0)
    } else if model.contains("o3") || model.contains("o4") {
        (2.0, 8.0)
    } else {
        (1.0, 4.0)
    };
    (input_tokens as f64 * input_price + output_tokens as f64 * output_price) / 1_000_000.0
}

// ─── Request/Response types ─────────────────────────────────────────────────

#[derive(Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct StreamChunk {
    choices: Vec<StreamChoice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct StreamChoice {
    delta: Delta,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: i64,
    completion_tokens: i64,
}
