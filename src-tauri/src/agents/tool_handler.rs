//! Tool Call Handler — processes function calls from SDK-based agents.
//!
//! Defines workflow tools that agents can invoke via function calling,
//! replacing HTML comment markers with structured JSON responses.

#![allow(dead_code)]

use serde::Serialize;
use serde_json::{json, Value};

/// Tool definition for SDK function calling
#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value, // JSON Schema
}

/// Result of executing a tool call
#[derive(Debug, Serialize)]
pub struct ToolCallResult {
    pub success: bool,
    pub output: String,
}

/// Context for tool execution
pub struct ToolContext {
    pub conversation_id: String,
    pub plan_id: Option<String>,
    pub project_path: Option<String>,
}

/// Returns the workflow tool definitions for SDK function calling
pub fn workflow_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "submit_plan_proposal".into(),
            description: "구현 계획을 제안합니다. title, description, subtasks를 포함하세요.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "Plan 제목" },
                    "description": { "type": "string", "description": "Plan 설명" },
                    "expected_outcome": { "type": "string", "description": "기대 결과" },
                    "subtasks": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "title": { "type": "string" },
                                "details": { "type": "string" }
                            },
                            "required": ["title"]
                        }
                    },
                    "constraints": { "type": "array", "items": { "type": "string" } },
                    "non_goals": { "type": "array", "items": { "type": "string" } }
                },
                "required": ["title", "description", "subtasks"]
            }),
        },
        ToolDefinition {
            name: "mark_subtask_done".into(),
            description: "Subtask 완료를 보고합니다.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "subtask_number": { "type": "integer", "description": "완료된 subtask 번호 (1부터)" },
                    "summary": { "type": "string", "description": "완료 요약" }
                },
                "required": ["subtask_number"]
            }),
        },
        ToolDefinition {
            name: "mark_implementation_complete".into(),
            description: "전체 구현 완료를 보고합니다.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "summary": { "type": "string", "description": "구현 결과 요약" }
                },
                "required": ["summary"]
            }),
        },
        ToolDefinition {
            name: "submit_review_verdict".into(),
            description: "리뷰 결과를 제출합니다. verdict, findings, recommendations를 포함하세요.".into(),
            parameters: json!({
                "type": "object",
                "properties": {
                    "verdict": {
                        "type": "string",
                        "enum": ["pass", "fail", "conditional"],
                        "description": "리뷰 판정"
                    },
                    "findings": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "description": { "type": "string" },
                                "file": { "type": "string" },
                                "severity": { "type": "string", "enum": ["critical", "major", "minor"] }
                            },
                            "required": ["description"]
                        },
                        "description": "발견 사항"
                    },
                    "recommendations": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "권장 사항"
                    }
                },
                "required": ["verdict", "findings"]
            }),
        },
    ]
}

/// Convert workflow tools to Gemini functionDeclarations format
pub fn to_gemini_tools(tools: &[ToolDefinition]) -> Value {
    json!([{
        "functionDeclarations": tools.iter().map(|t| json!({
            "name": t.name,
            "description": t.description,
            "parameters": t.parameters,
        })).collect::<Vec<_>>()
    }])
}

/// Convert workflow tools to OpenAI tools format
pub fn to_openai_tools(tools: &[ToolDefinition]) -> Value {
    json!(tools.iter().map(|t| json!({
        "type": "function",
        "function": {
            "name": t.name,
            "description": t.description,
            "parameters": t.parameters,
        }
    })).collect::<Vec<_>>())
}

/// Convert workflow tools to Anthropic tools format
pub fn to_anthropic_tools(tools: &[ToolDefinition]) -> Value {
    json!(tools.iter().map(|t| json!({
        "name": t.name,
        "description": t.description,
        "input_schema": t.parameters,
    })).collect::<Vec<_>>())
}

/// Execute a tool call and return the result
pub fn execute_tool_call(
    tool_name: &str,
    input: &Value,
    _ctx: &ToolContext,
) -> ToolCallResult {
    match tool_name {
        "submit_plan_proposal" => {
            // 마커 호환: plan-proposal 마커와 동일한 데이터를 생성
            let title = input.get("title").and_then(|v| v.as_str()).unwrap_or("Untitled");
            ToolCallResult {
                success: true,
                output: format!("Plan proposal '{}' received. Use the Plan tab to review.", title),
            }
        }
        "mark_subtask_done" => {
            let num = input.get("subtask_number").and_then(|v| v.as_i64()).unwrap_or(0);
            ToolCallResult {
                success: true,
                output: format!("Subtask {} marked as done.", num),
            }
        }
        "mark_implementation_complete" => {
            ToolCallResult {
                success: true,
                output: "Implementation marked as complete. Review can now begin.".into(),
            }
        }
        "submit_review_verdict" => {
            let verdict = input.get("verdict").and_then(|v| v.as_str()).unwrap_or("conditional");
            ToolCallResult {
                success: true,
                output: format!("Review verdict '{}' submitted.", verdict),
            }
        }
        _ => {
            ToolCallResult {
                success: false,
                output: format!("Unknown tool: {}", tool_name),
            }
        }
    }
}
