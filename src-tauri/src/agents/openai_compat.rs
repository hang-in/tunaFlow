//! OpenAI-Compatible Engine — generic HTTP client for any OpenAI-compatible API.
//!
//! Supports: Ollama, LM Studio, vLLM, Together AI, Groq, and OpenAI itself.
//! Uses the same OpenAI Chat Completions protocol (`POST /v1/chat/completions`).
//!
//! Configuration:
//! - `OLLAMA_HOST` env var (default `http://localhost:11434`)
//! - API key is optional for local backends (Ollama, LM Studio, vLLM)

use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::agents::claude::{RunInput, RunOutput};
use crate::errors::AppError;

/// Default Ollama base URL
fn ollama_base_url() -> String {
    std::env::var("OLLAMA_HOST")
        .unwrap_or_else(|_| "http://localhost:11434".into())
}

/// Check if Ollama is reachable (TCP connect check, no reqwest::blocking needed).
pub fn is_available() -> bool {
    use std::net::TcpStream;
    // Parse host:port from OLLAMA_HOST
    let base = ollama_base_url();
    let addr = base
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .trim_end_matches('/');
    let addr = if addr.contains(':') { addr.to_string() } else { format!("{}:11434", addr) };
    TcpStream::connect_timeout(
        &addr.parse().unwrap_or_else(|_| "127.0.0.1:11434".parse().unwrap()),
        std::time::Duration::from_secs(1),
    ).is_ok()
}

/// Discover installed Ollama models via `ollama list` CLI command.
pub fn discover_models() -> Option<Vec<String>> {
    let output = std::process::Command::new("ollama")
        .arg("list")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let models: Vec<String> = stdout
        .lines()
        .skip(1) // Skip header line "NAME  ID  SIZE  MODIFIED"
        .filter_map(|line| {
            let name = line.split_whitespace().next()?;
            if name.is_empty() { None } else { Some(name.to_string()) }
        })
        .collect();

    if models.is_empty() { None } else { Some(models) }
}

/// Streaming OpenAI-compatible Chat Completions API call.
///
/// Uses Ollama's OpenAI-compatible endpoint (`/v1/chat/completions`).
/// Function calling (tools) is included when supported.
pub async fn stream_run<F, G>(
    input: RunInput,
    mut on_progress: G,
    mut on_chunk: F,
) -> Result<RunOutput, AppError>
where
    F: FnMut(String) + Send,
    G: FnMut(String) + Send,
{
    let base = ollama_base_url();
    let model = input.model.as_deref().unwrap_or("qwen3:8b");
    let url = format!("{}/v1/chat/completions", base);

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

    // Include workflow tools for function calling (Ollama supports this for capable models)
    let tools = crate::agents::tool_handler::workflow_tools();
    let tools_json = crate::agents::tool_handler::to_openai_tools(&tools);

    let body = ChatCompletionRequest {
        model: model.to_string(),
        messages,
        stream: true,
        max_tokens: None, // Local models manage their own limits
        temperature: Some(0.7),
        tools: Some(tools_json),
    };

    on_progress(format!("Ollama ({}) initializing...", model));

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(600)) // Local models can be slow
        .build()
        .map_err(|e| AppError::Agent(format!("HTTP client build failed: {}", e)))?;

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() {
                AppError::Agent(format!(
                    "Ollama에 연결할 수 없습니다 ({}). Ollama가 실행 중인지 확인하세요: `ollama serve`",
                    base
                ))
            } else {
                AppError::Agent(format!("OpenAI-compatible API 요청 실패: {}", e))
            }
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        // If tools aren't supported, retry without tools
        if status.as_u16() == 400 && error_text.contains("tools") {
            eprintln!("[openai_compat] Model {} does not support tools, retrying without", model);
            return stream_run_no_tools(input, on_progress, on_chunk).await;
        }
        return Err(AppError::Agent(format!(
            "OpenAI-compatible API error {}: {}",
            status,
            &error_text[..error_text.len().min(500)]
        )));
    }

    parse_sse_stream(response, &input, &mut on_progress, &mut on_chunk, model).await
}

/// Fallback: stream without tool definitions (for models that don't support function calling).
async fn stream_run_no_tools<F, G>(
    input: RunInput,
    mut on_progress: G,
    mut on_chunk: F,
) -> Result<RunOutput, AppError>
where
    F: FnMut(String) + Send,
    G: FnMut(String) + Send,
{
    let base = ollama_base_url();
    let model = input.model.as_deref().unwrap_or("qwen3:8b");
    let url = format!("{}/v1/chat/completions", base);

    let mut messages: Vec<ChatMessage> = Vec::new();
    if let Some(sp) = &input.system_prompt {
        messages.push(ChatMessage { role: "system".into(), content: sp.clone() });
    }
    messages.push(ChatMessage { role: "user".into(), content: input.prompt.clone() });

    let body = ChatCompletionRequest {
        model: model.to_string(),
        messages,
        stream: true,
        max_tokens: None,
        temperature: Some(0.7),
        tools: None, // No tools
    };

    on_progress(format!("Ollama ({}) running (no tools)...", model));

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| AppError::Agent(format!("HTTP client build failed: {}", e)))?;

    let response = client
        .post(&url)
        .json(&body)
        .send()
        .await
        .map_err(|e| AppError::Agent(format!("OpenAI-compatible API 요청 실패: {}", e)))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_default();
        return Err(AppError::Agent(format!(
            "OpenAI-compatible API error {}: {}",
            status,
            &error_text[..error_text.len().min(500)]
        )));
    }

    parse_sse_stream(response, &input, &mut on_progress, &mut on_chunk, model).await
}

/// Parse SSE stream from OpenAI-compatible endpoint.
async fn parse_sse_stream<F, G>(
    response: reqwest::Response,
    input: &RunInput,
    on_progress: &mut G,
    on_chunk: &mut F,
    model: &str,
) -> Result<RunOutput, AppError>
where
    F: FnMut(String) + Send,
    G: FnMut(String) + Send,
{
    let mut full_text = String::new();
    let mut input_tokens: i64 = 0;
    let mut output_tokens: i64 = 0;

    // Tool call accumulation (OpenAI streams tool calls in fragments)
    let mut tool_name_buf = String::new();
    let mut tool_args_buf = String::new();

    let mut stream = response.bytes_stream();
    let mut buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result
            .map_err(|e| AppError::Agent(format!("Stream read error: {}", e)))?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        buffer.push_str(&chunk_str);

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
                        // Accumulate tool call fragments
                        if let Some(tool_calls) = &choice.delta.tool_calls {
                            for tc in tool_calls {
                                if let Some(func) = &tc.function {
                                    if let Some(name) = &func.name {
                                        // New tool call — flush previous if any
                                        if !tool_name_buf.is_empty() {
                                            execute_accumulated_tool(
                                                &tool_name_buf, &tool_args_buf,
                                                input, on_progress, on_chunk,
                                                &mut full_text,
                                            );
                                            tool_args_buf.clear();
                                        }
                                        tool_name_buf = name.clone();
                                    }
                                    if let Some(args) = &func.arguments {
                                        tool_args_buf.push_str(args);
                                    }
                                }
                            }
                        }
                        // Check if this choice is finished
                        if choice.finish_reason.as_deref() == Some("tool_calls") {
                            if !tool_name_buf.is_empty() {
                                execute_accumulated_tool(
                                    &tool_name_buf, &tool_args_buf,
                                    input, on_progress, on_chunk,
                                    &mut full_text,
                                );
                                tool_name_buf.clear();
                                tool_args_buf.clear();
                            }
                        }
                    }
                    if let Some(usage) = &chunk.usage {
                        input_tokens = usage.prompt_tokens;
                        output_tokens = usage.completion_tokens;
                    }
                }

                buffer = buffer[json_start + end_pos..].to_string();
            } else {
                break;
            }
        }
    }

    // Flush any remaining tool call
    if !tool_name_buf.is_empty() {
        execute_accumulated_tool(
            &tool_name_buf, &tool_args_buf,
            input, &mut *on_progress, &mut *on_chunk,
            &mut full_text,
        );
    }

    if full_text.is_empty() {
        return Err(AppError::Agent(format!(
            "Ollama ({}) returned empty response. 모델이 설치되어 있는지 확인: `ollama list`",
            model
        )));
    }

    Ok(RunOutput {
        content: full_text,
        cost_usd: 0.0, // Local models are free
        input_tokens,
        output_tokens,
        session_id: None,
    })
}

/// Execute an accumulated tool call.
fn execute_accumulated_tool<F, G>(
    name: &str, args_str: &str,
    input: &RunInput,
    on_progress: &mut G,
    on_chunk: &mut F,
    full_text: &mut String,
)
where
    F: FnMut(String) + Send,
    G: FnMut(String) + Send,
{
    let args = serde_json::from_str::<serde_json::Value>(args_str)
        .unwrap_or(serde_json::Value::Null);
    let ctx = crate::agents::tool_handler::ToolContext {
        conversation_id: String::new(),
        plan_id: None,
        project_path: input.project_path.clone(),
    };
    let result = crate::agents::tool_handler::execute_tool_call(name, &args, &ctx);
    on_progress(format!("🔧 {} → {}", name, result.output));
    full_text.push_str(&format!("\n\n[Tool: {}] {}", name, result.output));
    on_chunk(full_text.clone());
}

/// Synchronous wrapper for RT participant execution.
pub fn run(input: RunInput) -> Result<RunOutput, AppError> {
    let rt = tokio::runtime::Handle::try_current()
        .map_err(|_| AppError::Agent("No tokio runtime available for openai_compat".into()))?;
    rt.block_on(async {
        stream_run(input, |_| {}, |_| {}).await
    })
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
    finish_reason: Option<String>,
}

#[derive(Deserialize)]
struct Delta {
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Deserialize)]
struct ToolCallDelta {
    function: Option<ToolCallFunction>,
}

#[derive(Deserialize)]
struct ToolCallFunction {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: i64,
    completion_tokens: i64,
}
