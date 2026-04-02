//! Anthropic SDK — direct Messages API integration.
//!
//! Uses Anthropic Messages API with SSE streaming.
//! Requires ANTHROPIC_API_KEY environment variable.
//!
//! Note: Claude CLI is preferred for main chat (file editing, MCP, terminal).
//! SDK is better for workflow pipeline (Developer/Reviewer) where structured
//! output and accurate token tracking matter more than CLI features.

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;

const API_BASE: &str = "https://api.anthropic.com/v1";
const API_VERSION: &str = "2023-06-01";

/// Check if Anthropic SDK can be used (API key available)
pub fn is_available() -> bool {
    std::env::var("ANTHROPIC_API_KEY").is_ok()
}

/// Streaming Anthropic Messages API call with SSE parsing.
pub async fn stream_run<F, G>(
    input: RunInput,
    mut on_progress: G,
    mut on_chunk: F,
) -> Result<RunOutput, AppError>
where
    F: FnMut(String) + Send,
    G: FnMut(String) + Send,
{
    let api_key = std::env::var("ANTHROPIC_API_KEY")
        .map_err(|_| AppError::Agent("ANTHROPIC_API_KEY not set".into()))?;

    let model = input.model.as_deref().unwrap_or("claude-sonnet-4-6");
    let url = format!("{}/messages", API_BASE);

    let messages = vec![Message {
        role: "user".into(),
        content: input.prompt.clone(),
    }];

    let tools = crate::agents::tool_handler::workflow_tools();
    let tools_json = crate::agents::tool_handler::to_anthropic_tools(&tools);

    let body = CreateMessageRequest {
        model: model.to_string(),
        messages,
        system: input.system_prompt.clone(),
        max_tokens: 16384,
        stream: true,
        tools: Some(tools_json),
    };

    on_progress("Anthropic SDK initializing...".into());

    let client = Client::new();
    let response = client
        .post(&url)
        .header("x-api-key", &api_key)
        .header("anthropic-version", API_VERSION)
        .header("content-type", "application/json")
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Agent(format!("Anthropic API request failed: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::Agent(format!(
            "Anthropic API error {}: {}",
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

        // SSE: "event: {type}\ndata: {json}\n\n"
        while let Some(data_pos) = buffer.find("data: ") {
            let json_start = data_pos + 6;
            if let Some(end_pos) = buffer[json_start..].find("\n") {
                let data_str = buffer[json_start..json_start + end_pos].trim();

                if let Ok(event) = serde_json::from_str::<StreamEvent>(data_str) {
                    match event.event_type.as_deref() {
                        Some("content_block_delta") => {
                            if let Some(delta) = &event.delta {
                                if let Some(text) = &delta.text {
                                    full_text.push_str(text);
                                    on_chunk(full_text.clone());
                                }
                            }
                        }
                        Some("message_start") => {
                            if let Some(msg) = &event.message {
                                if let Some(usage) = &msg.usage {
                                    input_tokens = usage.input_tokens.unwrap_or(0);
                                }
                            }
                        }
                        Some("message_delta") => {
                            if let Some(usage) = &event.usage {
                                output_tokens = usage.output_tokens.unwrap_or(0);
                            }
                        }
                        _ => {}
                    }
                }

                buffer = buffer[json_start + end_pos..].to_string();
            } else {
                break;
            }
        }
    }

    if full_text.is_empty() {
        return Err(AppError::Agent("Anthropic returned empty response".into()));
    }

    Ok(RunOutput {
        content: full_text,
        cost_usd: estimate_cost(model, input_tokens, output_tokens),
        input_tokens,
        output_tokens,
        session_id: None,
    })
}

#[allow(dead_code)]
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| AppError::Agent("No tokio runtime".into()))?;
    rt.block_on(async {
        stream_run(input, |_| {}, |_| {}).await
    })
}

fn estimate_cost(model: &str, input_tokens: i64, output_tokens: i64) -> f64 {
    let (input_price, output_price) = if model.contains("opus") {
        (15.0, 75.0)
    } else if model.contains("sonnet") {
        (3.0, 15.0)
    } else if model.contains("haiku") {
        (0.80, 4.0)
    } else {
        (3.0, 15.0)
    };
    (input_tokens as f64 * input_price + output_tokens as f64 * output_price) / 1_000_000.0
}

// ─── Request/Response types ─────────────────────────────────────────────────

#[derive(Serialize)]
struct CreateMessageRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    max_tokens: u32,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<serde_json::Value>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct StreamEvent {
    #[serde(rename = "type")]
    event_type: Option<String>,
    delta: Option<ContentDelta>,
    message: Option<MessageInfo>,
    usage: Option<UsageInfo>,
}

#[derive(Deserialize)]
struct ContentDelta {
    text: Option<String>,
}

#[derive(Deserialize)]
struct MessageInfo {
    usage: Option<UsageInfo>,
}

#[derive(Deserialize)]
struct UsageInfo {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
}
