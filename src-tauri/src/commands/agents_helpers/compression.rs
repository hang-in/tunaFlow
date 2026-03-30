use crate::agents::claude;
use crate::guardrail;

/// Summarise a long context section via a direct claude subprocess call.
///
/// `target_chars` controls the compression target size.
/// `preserve_hint` tells Claude what to prioritize in the summary.
///
/// Returns `Ok(summary)` when claude produces non-empty output.
/// Returns `Err(())` on any failure.
///
/// Recursion safety: calls claude::run() directly with no system_prompt
/// and no resume_token — ContextPack assembly is never entered again.
pub fn compress_context_with_claude(text: &str, target_chars: usize, preserve_hint: &str) -> Result<String, ()> {
    let prompt = format!(
        "Summarise the following context in plain text, under {} characters.\n\
        Preserve: {}.\n\
        No markdown headers. No filler. Just the essential facts.\n\n\
        ---\n\n{}",
        target_chars, preserve_hint, text
    );
    claude::run(claude::RunInput {
        prompt,
        model: None,
        system_prompt: None,
        resume_token: None,
        project_path: None,
    })
    .ok()
    .map(|out| out.content)
    .filter(|s| !s.trim().is_empty())
    .ok_or(())
}

/// Section-type hints for smarter compression.
pub struct CompressionHint {
    pub target_chars: usize,
    pub preserve: &'static str,
}

/// Default compression hints by section type.
pub fn hint_for_section(section_type: &str) -> CompressionHint {
    match section_type {
        "context" => CompressionHint {
            target_chars: 800,
            preserve: "the user's most recent question, decisions already made, key constraints, and anything needed for the next reply",
        },
        "cross-session" => CompressionHint {
            target_chars: 600,
            preserve: "which session each fact came from, key conclusions, and unresolved questions",
        },
        "findings" => CompressionHint {
            target_chars: 400,
            preserve: "specific findings, recommendations, and action items",
        },
        _ => CompressionHint {
            target_chars: 600,
            preserve: "what the user is working on, decisions already made, key constraints, and anything needed for the next reply",
        },
    }
}

/// Return the section as-is if within `limit`.
/// If over `limit`, attempt claude compression first; fall back to truncation.
#[allow(dead_code)]
pub fn maybe_compress_section(section: Option<String>, limit: usize) -> Option<String> {
    maybe_compress_section_typed(section, limit, None)
}

/// Like `maybe_compress_section` but with section-type aware compression hints.
pub fn maybe_compress_section_typed(section: Option<String>, limit: usize, section_type: Option<&str>) -> Option<String> {
    let s = section?;
    if s.len() <= limit {
        return Some(s);
    }

    let hint = section_type
        .map(hint_for_section)
        .unwrap_or_else(|| hint_for_section("default"));

    match compress_context_with_claude(&s, hint.target_chars, hint.preserve) {
        Ok(compressed) if compressed.len() <= limit => {
            eprintln!(
                "[compress] ok: {} → {} chars (type={:?})",
                s.len(),
                compressed.len(),
                section_type,
            );
            Some(compressed)
        }
        Ok(compressed) => {
            eprintln!(
                "[compress] still over limit after compression ({} chars), truncating (type={:?})",
                compressed.len(),
                section_type,
            );
            guardrail::truncate_section(Some(compressed), limit)
        }
        Err(()) => {
            eprintln!("[compress] failed, falling back to truncate ({} chars, type={:?})", s.len(), section_type);
            guardrail::truncate_section(Some(s), limit)
        }
    }
}
