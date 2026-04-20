use rusqlite::Connection;

use super::super::context_pack::build_lite_context_prompt;

/// Load the project path for a given project_key.
pub fn load_project_path(conn: &Connection, project_key: &str) -> Option<String> {
    conn.query_row(
        "SELECT path FROM projects WHERE key = ?1",
        [project_key],
        |row| row.get(0),
    )
    .ok()
    .flatten()
}

/// Build an enriched prompt with lite context prefix for non-Claude engines.
/// Retained for roundtable participant paths that don't carry full SendWithClaudeInput.
#[allow(dead_code)]
pub fn build_lite_enriched_prompt(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
) -> String {
    let prefix = project_path
        .map(|p| format!("Project: {}\n", p))
        .unwrap_or_default();
    format!(
        "{}{}",
        prefix,
        build_lite_context_prompt(conn, conversation_id, prompt)
    )
}

/// All data needed to assemble a ContextPack, pre-loaded from DB.
/// Separating data loading (DB-dependent) from prompt assembly (pure function)
/// enables unit testing of assembly logic and tighter DB lock scopes.
#[allow(dead_code)]
pub struct ContextData {
    pub conversation_id: String,
    pub project_path: Option<String>,
    pub prompt: String,
    pub is_branch: bool,

    // Auto mode signals
    pub has_active_plan: bool,

    // Recent context (role, content, engine, persona)
    pub current_messages: Vec<(String, String, Option<String>, Option<String>)>,
    pub parent_messages: Vec<(String, String, Option<String>, Option<String>)>,

    // Structured memory (pre-built section strings)
    pub plan_section: Option<String>,
    /// Full plan document markdown (from docs/plans/{slug}.md)
    pub plan_document: Option<String>,
    pub findings_section: Option<String>,
    pub artifacts_section: Option<String>,

    // Retrieval
    pub retrieval_chunks: Vec<crate::commands::context_queries::RetrievedChunk>,

    // Project document search results (file_path, section_title, text_preview, score)
    pub document_chunks: Vec<(String, Option<String>, String, f32)>,

    // Compressed memory
    pub compressed_memory: Option<String>,
    /// 가장 최근 compressed_memory row 의 `model_used` — 엔진 전환 시 출처 표기용.
    pub compressed_memory_source: Option<String>,

    // Cross-session (label, messages)
    pub cross_session_data: Vec<(String, Vec<(String, String)>)>,

    // Thread inheritance
    pub thread_inheritance: Option<String>,

    // Agent role document (docs/agents/{role}.md content)
    pub agent_role_doc: Option<String>,

    /// Revision 컨텍스트 — architect 가 rev.N 을 제안할 때 참고하도록 이전
    /// impl branch 의 변경 파일 / 최근 review findings 를 요약해 주입. 트리거:
    /// plan_events 에 `doom_loop_escalated` 또는 `architect_redesign_requested`
    /// 또는 `review_failed` 가 가장 최근 이벤트일 때. 그 외엔 None.
    pub previous_impl_status: Option<String>,

    // Pass-through (no DB needed)
    pub active_skills: Vec<String>,
    pub cross_session_ids: Vec<String>,
    pub persona_fragment: Option<String>,
    pub context_mode_override: Option<String>,
    pub context_budget_cap: Option<usize>,
    /// Serialized user profile JSON (from frontend settings store).
    /// Contains name, title, bio, preferredLanguages, gitName, gitEmail, githubOrg.
    pub user_profile: Option<String>,

    /// Conventions Sync Phase 2 — when true, the ContextPack skips the static
    /// layers (platform / agent-role / persona / user-profile) because they've
    /// been synced into CLAUDE.md/AGENTS.md/GEMINI.md. Default false.
    /// Toggled per-project via `set_project_conventions_sync` Tauri command.
    pub conventions_synced: bool,

    /// 같은 Claude/Codex 세션이 연속되는지 여부. True 면 `recent_context` +
    /// `compressed_memory` 섹션을 **생성하지 않는다** — Claude 자체 세션이 history 를
    /// 가지고 있어 tunaFlow prepend 가 오염원이 되기 때문. 에이전트는 필요 시
    /// `tool-request:recent_turns:N` 으로 명시 조회. False 면 정상 Full + anchor.
    pub is_session_continuation: bool,
}

/// Phase A: Load all data needed for ContextPack assembly from DB.
///
/// This function gathers all DB-dependent data into a `ContextData` struct.
/// The returned struct can then be passed to `assemble_prompt()` (Phase B)
/// which is a pure function with no DB dependency.
pub fn load_context_data(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
    context_mode_override: Option<&str>,
    context_budget_cap: Option<usize>,
    user_profile_json: Option<&str>,
) -> ContextData {
    use super::super::context_pack::*;
    use crate::commands::context_queries::{
        load_recent_messages, load_recent_messages_with_author, conversation_label,
        retrieve_relevant_chunks_with_overlap,
    };
    use crate::commands::conversation_memory::load_compressed_memory;

    let is_branch = conversation_id.starts_with("branch:");

    // Check conversation type (scratchpad inherits main chat context)
    let conv_type: String = conn.query_row(
        "SELECT COALESCE(type, 'main') FROM conversations WHERE id = ?1",
        [conversation_id], |row| row.get(0),
    ).unwrap_or_else(|_| "main".into());
    let is_scratchpad = conv_type == "scratchpad";

    // Query 1: has_active_plan (auto mode signal) — check main chat plan for scratchpads
    let plan_lookup_conv = if is_scratchpad {
        // Find main chat for this project
        conn.query_row(
            "SELECT c2.id FROM conversations c1
             JOIN conversations c2 ON c2.project_key = c1.project_key AND c2.type = 'main'
             WHERE c1.id = ?1 LIMIT 1",
            [conversation_id], |row| row.get::<_, String>(0),
        ).ok()
    } else { None };
    let has_active_plan: bool = conn.query_row(
        "SELECT COUNT(*) > 0 FROM plans WHERE conversation_id = ?1 AND status = 'active'",
        [plan_lookup_conv.as_deref().unwrap_or(conversation_id)], |row| row.get(0),
    ).unwrap_or(false);

    // Query 2: current messages — budget-based dynamic window + per-agent last-message guarantee
    let current_messages = load_recent_messages_with_author(conn, conversation_id, 20);

    // Query 3: parent messages (branch: parent conv, scratchpad: main chat)
    let parent_messages: Vec<(String, String, Option<String>, Option<String>)> = if is_branch {
        let parent_id: Option<String> = conn
            .query_row(
                "SELECT parent_id FROM conversations WHERE id = ?1",
                [conversation_id],
                |row| row.get(0),
            )
            .ok()
            .flatten();
        parent_id
            .map(|pid| load_recent_messages_with_author(conn, &pid, 4))
            .unwrap_or_default()
    } else if is_scratchpad {
        // Scratchpad inherits main chat context — load recent messages from main conversation
        plan_lookup_conv.as_ref()
            .map(|main_id| load_recent_messages_with_author(conn, main_id, 8))
            .unwrap_or_default()
    } else {
        Vec::new()
    };

    // Query 4-7: plan, findings, artifacts (scratchpad: use main chat's plan)
    let effective_conv_id = if is_scratchpad {
        plan_lookup_conv.as_deref().unwrap_or(conversation_id)
    } else { conversation_id };
    let plan_conv_id = resolve_plan_conversation_id(conn, effective_conv_id);
    let mut plan_section = build_plan_section(conn, &plan_conv_id);

    // Append completed plan titles so the agent knows what was already done
    let done_plans: Vec<String> = conn.prepare(
        "SELECT title FROM plans WHERE conversation_id = ?1 AND status = 'done' ORDER BY updated_at DESC LIMIT 10"
    ).ok().map(|mut stmt| {
        stmt.query_map([plan_lookup_conv.as_deref().unwrap_or(conversation_id)], |row| row.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok()).collect())
            .unwrap_or_default()
    }).unwrap_or_default();
    if !done_plans.is_empty() {
        let suffix = format!(
            "\n\n### Completed Plans\n{}",
            done_plans.iter().map(|t| format!("- ✅ {}", t)).collect::<Vec<_>>().join("\n")
        );
        plan_section = Some(match plan_section {
            Some(s) => format!("{}{}", s, suffix),
            None => format!("## Plans{}", suffix),
        });
    }

    // Pre-load project_key for failure lessons (used in rework phase)
    let project_key_for_failures: Option<String> = conn.query_row(
        "SELECT project_key FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| row.get(0),
    ).ok();

    // Load plan documents from filesystem (plan, result, review)
    let plan_document: Option<String> = if has_active_plan {
        if let Some(pp) = project_path {
            let plan_row = conn.query_row(
                "SELECT title, phase, slug FROM plans WHERE conversation_id = ?1 AND status = 'active' LIMIT 1",
                [plan_lookup_conv.as_deref().unwrap_or(conversation_id)],
                |row| Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                )),
            ).ok();
            plan_row.and_then(|(title, phase, slug_opt)| {
                // Prefer the canonical slug persisted in `plans.slug` (v26). Fall
                // back to title-based slugify only for pre-v26 rows. All writers
                // (generate_plan_document/review/result) use the same source, so
                // the Reviewer must match their filenames exactly.
                let slug = slug_opt.unwrap_or_else(|| crate::commands::plans::slugify_pub(&title));
                let plans_dir = std::path::Path::new(pp).join("docs").join("plans");
                let mut combined = String::new();

                // Plan document (always)
                if let Ok(doc) = std::fs::read_to_string(plans_dir.join(format!("{}.md", slug))) {
                    combined.push_str(&doc);
                }

                // Task files (review/rework phase — Reviewer needs per-subtask specs)
                if phase == "review" || phase == "rework" {
                    for i in 1..=50 {
                        let task_path = plans_dir.join(format!("{}-task-{:02}.md", slug, i));
                        if !task_path.exists() { break; }
                        if let Ok(doc) = std::fs::read_to_string(&task_path) {
                            combined.push_str(&format!("\n\n---\n\n## Task {}\n\n", i));
                            combined.push_str(&doc);
                        }
                    }
                }

                // Result report (review/rework phase — Reviewer needs implementation context)
                if phase == "review" || phase == "rework" {
                    if let Ok(doc) = std::fs::read_to_string(plans_dir.join(format!("{}-result.md", slug))) {
                        combined.push_str("\n\n---\n\n");
                        combined.push_str(&doc);
                    }
                }

                // Latest review report (rework phase — Developer needs review feedback)
                if phase == "rework" {
                    // Find latest review-r{N}.md
                    let mut round = 1;
                    let mut latest_review = None;
                    while plans_dir.join(format!("{}-review-r{}.md", slug, round)).exists() {
                        latest_review = std::fs::read_to_string(plans_dir.join(format!("{}-review-r{}.md", slug, round))).ok();
                        round += 1;
                    }
                    if let Some(doc) = latest_review {
                        combined.push_str("\n\n---\n\n");
                        combined.push_str(&doc);
                    }

                    // Failure learning: search similar past failures for rework context
                    if let Some(pk) = &project_key_for_failures {
                        let similar = search_failure_lessons_for_rework(conn, pk, &combined);
                        if !similar.is_empty() {
                            combined.push_str("\n\n---\n\n## Previous Similar Failures\n\n");
                            combined.push_str("아래는 같은 프로젝트에서 과거 발생한 유사한 실패 사례입니다. 같은 실수를 반복하지 마세요.\n\n");
                            for (i, lesson) in similar.iter().enumerate() {
                                combined.push_str(&format!("### Case {}\n", i + 1));
                                if let Some(fp) = &lesson.file_path {
                                    combined.push_str(&format!("- **File**: `{}`\n", fp));
                                }
                                combined.push_str(&format!("- **Finding**: {}\n", lesson.finding));
                                if let Some(res) = &lesson.resolution {
                                    combined.push_str(&format!("- **Resolution**: {}\n", res));
                                }
                                combined.push('\n');
                            }
                        }
                    }
                }

                if combined.is_empty() { None } else { Some(combined) }
            })
        } else { None }
    } else { None };

    let findings_section = build_findings_section(conn, &plan_conv_id);
    let artifacts_section = build_artifact_handoff_section(conn, &plan_conv_id);

    // Query 8-9: retrieval chunks (FTS5 + vector hybrid)
    let project_key: Option<String> = conn.query_row(
        "SELECT project_key FROM conversations WHERE id = ?1",
        [conversation_id],
        |row| row.get(0),
    ).ok();
    let retrieval_chunks = if let Some(pk) = &project_key {
        let recent_ids: Vec<String> = conn.prepare(
            "SELECT id FROM messages WHERE conversation_id = ?1 ORDER BY timestamp DESC LIMIT 12"
        ).ok().map(|mut stmt| {
            stmt.query_map([conversation_id], |row| row.get::<_, String>(0))
                .map(|rows| rows.filter_map(|r| r.ok()).collect())
                .unwrap_or_default()
        }).unwrap_or_default();
        let mut fts_chunks = retrieve_relevant_chunks_with_overlap(conn, pk, conversation_id, prompt, &recent_ids, 10, None);
        // Safe truncation helper for logging (respects char boundaries). In release
        // builds we only log lengths, not content previews — user prompts/messages
        // may contain API keys or other secrets pasted inline, and eprintln output
        // can land in systemd journals, shell scrollback, or screen-recorded demos.
        #[cfg(debug_assertions)]
        let safe_trunc = |s: &str, max: usize| -> String {
            if s.len() <= max { return s.to_string(); }
            let mut end = max;
            while end > 0 && !s.is_char_boundary(end) { end -= 1; }
            format!("{}…", &s[..end])
        };
        #[cfg(not(debug_assertions))]
        let safe_trunc = |_s: &str, _max: usize| -> String { format!("<{}ch>", _s.len()) };

        eprintln!("[retrieval] FTS5: {} chunks for query=\"{}\"", fts_chunks.len(), safe_trunc(prompt, 30));
        for (i, c) in fts_chunks.iter().enumerate() {
            let preview = c.messages.first().map(|(_, t, _, _)| safe_trunc(t, 40)).unwrap_or_default();
            eprintln!("[retrieval]   fts[{}] kind={} score={:.3} conv={} text=\"{}\"", i, c.kind, c.score, safe_trunc(&c.conversation_id, 8), preview);
        }

        // Boost with vector search results (best-effort, skip if rawq unavailable)
        // Skip vector search for short/simple prompts (avoid cold-start delays)
        let needs_vector = prompt.chars().count() >= 15
            && (crate::agents::embedder::is_available() || crate::agents::rawq::is_daemon_ready());
        if needs_vector {
        if let Ok(query_emb) = crate::agents::embedder::embed_text(prompt, true) {
            let vec_results = crate::commands::vector_search::search_similar(conn, &query_emb, pk, conversation_id, 5);
            eprintln!("[retrieval] Vector: {} chunks (threshold 0.3)", vec_results.len());
            for (i, vc) in vec_results.iter().enumerate() {
                eprintln!("[retrieval]   vec[{}] score={:.3} conv={} text=\"{}\"", i, vc.score, safe_trunc(&vc.conversation_id, 8), safe_trunc(&vc.text_preview, 40));
            }
            // RRF merge: add vector-only results that aren't already in FTS results
            let fts_conv_ids: std::collections::HashSet<String> = fts_chunks.iter().map(|c| c.conversation_id.clone()).collect();
            for vc in vec_results {
                if vc.score > 0.3 && !fts_conv_ids.contains(&vc.conversation_id) {
                    let text = vc.full_text.unwrap_or(vc.text_preview);
                    fts_chunks.push(crate::commands::context_queries::RetrievedChunk {
                        kind: "anchor",
                        messages: vec![("assistant".to_string(), text, None, None)],
                        conversation_id: vc.conversation_id,
                        score: vc.score as f64,
                        timestamp: 0,
                    });
                }
            }
        } else {
            eprintln!("[retrieval] Vector: skipped (rawq embed unavailable)");
        }
        } else {
            eprintln!("[retrieval] Vector: skipped (prompt too short or daemon not ready)");
        }
        eprintln!("[retrieval] Total: {} chunks after merge", fts_chunks.len());
        fts_chunks
    } else {
        Vec::new()
    };

    // Query 10b: project document search (vector, Standard+ only)
    let document_chunks: Vec<(String, Option<String>, String, f32)> = if let Some(pk) = &project_key {
        if prompt.chars().count() >= 15 && (crate::agents::embedder::is_available() || crate::agents::rawq::is_daemon_ready()) {
            if let Ok(query_emb) = crate::agents::embedder::embed_text(prompt, true) {
                let query_blob = crate::commands::vector_search::embedding_to_blob(&query_emb);
                // Search document chunks via vec0 KNN
                let sql = "
                    SELECT c.file_path, c.section_title, c.text_preview, v.distance
                    FROM vec_chunks v
                    JOIN conversation_chunks c ON c.rowid = v.rowid
                    WHERE v.embedding MATCH ?1
                      AND k = 15
                      AND c.project_key = ?2
                      AND c.source_type = 'document'
                    ORDER BY v.distance ASC
                ";
                conn.prepare(sql).ok().map(|mut stmt| {
                    stmt.query_map(rusqlite::params![query_blob, pk], |row| {
                        Ok((
                            row.get::<_, Option<String>>(0)?.unwrap_or_default(),
                            row.get::<_, Option<String>>(1)?,
                            row.get::<_, String>(2)?,
                            1.0_f32 - row.get::<_, f32>(3)?, // distance → similarity
                        ))
                    // Threshold dropped 0.5 → 0.35 (matches conversation-vec
                    // threshold 0.3 scale). bge-m3 document matches typically
                    // sit in the 0.4–0.5 band; the old 0.5 gate filtered out
                    // every real hit, so document chunks never reached the
                    // ContextPack even after the v32→v36 re-embed. See
                    // docs/reference/knownIssues_2026-04-15.md I10.
                    }).map(|rows| rows.filter_map(|r| r.ok()).filter(|r| r.3 > 0.35).take(5).collect())
                        .unwrap_or_default()
                }).unwrap_or_default()
            } else { Vec::new() }
        } else { Vec::new() }
    } else { Vec::new() };
    if !document_chunks.is_empty() {
        eprintln!("[retrieval] Document chunks: {} results", document_chunks.len());
        for (i, (fp, st, _, score)) in document_chunks.iter().enumerate() {
            eprintln!("[retrieval]   doc[{}] score={:.3} file={} section={}", i, score, fp, st.as_deref().unwrap_or("-"));
        }
    }

    // Query 10: compressed memory (scratchpad: also check main chat memory)
    let (compressed_memory, compressed_memory_source) = {
        let own = load_compressed_memory(conn, conversation_id);
        if own.is_some() {
            (own, crate::commands::memory_topics::latest_memory_source(conn, conversation_id))
        } else if is_scratchpad {
            if let Some(main_id) = plan_lookup_conv.as_deref() {
                let main_mem = load_compressed_memory(conn, main_id);
                let src = if main_mem.is_some() {
                    crate::commands::memory_topics::latest_memory_source(conn, main_id)
                } else { None };
                (main_mem, src)
            } else { (None, None) }
        } else { (None, None) }
    };

    // Query 11: cross-session data (manual IDs + auto-discovered links)
    let effective_cross_ids: Vec<String> = if cross_session_ids.is_empty() {
        // No manual selection: use auto-discovered session links
        crate::commands::session_discovery::load_active_session_ids(conn, conversation_id, 3)
    } else {
        cross_session_ids.to_vec()
    };
    let cross_session_data: Vec<(String, Vec<(String, String)>)> = effective_cross_ids
        .iter()
        .filter(|id| id.as_str() != conversation_id)
        .filter_map(|id| {
            let label = conversation_label(conn, id)?;
            let rows = load_recent_messages(conn, id, 3);
            if rows.is_empty() { None } else { Some((label, rows)) }
        })
        .collect();

    // Load agent role document from project docs/agents/
    let agent_role_doc: Option<String> = project_path.and_then(|pp| {
        let agents_dir = std::path::Path::new(pp).join("docs").join("agents");
        if !agents_dir.is_dir() { return None; }

        // Determine role:
        // - Implementation branch (linked to plan) → developer
        // - Review branch → reviewer
        // - Everything else (main chat, subtask discussion branch) → architect
        let role = if is_branch {
            let branch_id = conversation_id.strip_prefix("branch:").unwrap_or("");
            let is_impl: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM plans WHERE implementation_branch_id = ?1",
                [branch_id], |row| row.get(0),
            ).unwrap_or(false);
            let is_review: bool = conn.query_row(
                "SELECT COUNT(*) > 0 FROM plans WHERE review_branch_id = ?1",
                [branch_id], |row| row.get(0),
            ).unwrap_or(false);
            if is_impl { "developer" } else if is_review { "reviewer" } else { "architect" }
        } else { "architect" };

        let role_file = agents_dir.join(format!("{}.md", role));
        std::fs::read_to_string(&role_file).ok()
    });

    // Query 12: thread inheritance
    let thread_inheritance = if is_branch {
        build_thread_inheritance_section(conn, conversation_id)
    } else {
        None
    };

    // Query 13: previous implementation status (for architect revision context).
    // 아키텍트가 rev.N 을 제안할 때 이전 impl branch 의 변경 요약·최근 findings 를
    // 자동 주입해 "어떤 파일을 keep/modify/revert 할지" 판단 근거를 제공.
    let previous_impl_status = build_previous_impl_status(conn, conversation_id);

    // Phase 2 — conventions sync per-project toggle. We already fetched
    // `project_key` above for retrieval; reuse it here instead of re-querying.
    let conventions_synced = project_key
        .as_deref()
        .map(|pk| crate::commands::conventions_sync::is_conventions_sync_enabled(conn, pk))
        .unwrap_or(false);

    ContextData {
        conversation_id: conversation_id.to_string(),
        project_path: project_path.map(|s| s.to_string()),
        prompt: prompt.to_string(),
        is_branch,
        has_active_plan,
        current_messages,
        parent_messages,
        plan_section,
        plan_document,
        findings_section,
        artifacts_section,
        retrieval_chunks,
        document_chunks,
        compressed_memory,
        compressed_memory_source,
        cross_session_data,
        thread_inheritance,
        agent_role_doc,
        previous_impl_status,
        active_skills: active_skills.to_vec(),
        cross_session_ids: cross_session_ids.to_vec(),
        persona_fragment: persona_fragment.map(|s| s.to_string()),
        context_mode_override: context_mode_override.map(|s| s.to_string()),
        context_budget_cap,
        user_profile: user_profile_json.map(|s| s.to_string()),
        conventions_synced,
        is_session_continuation: false, // persistence.rs 에서 필요시 true 로 override
    }
}

/// 아키텍트 revision 컨텍스트 빌더. 조건:
///   - conversation 의 active plan 이 있고
///   - plan_events 의 가장 최근 이벤트가 `doom_loop_escalated` / `architect_redesign_requested`
///     / `review_failed` / `revision_requested` / `plan_full_revision_requested` 중 하나
///   - impl branch 또는 review branch 가 존재
///
/// 포함 내용:
///   - 이전 Plan 요약 (title/phase/revision)
///   - impl branch 에서 수정된 파일 목록 (artifacts 테이블의 file_refs 에서 뽑음)
///   - 최근 review findings (latest review_verdict message)
fn build_previous_impl_status(conn: &Connection, conversation_id: &str) -> Option<String> {
    // 1) active plan
    let plan_row: Option<(String, String, String, i64, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT id, title, phase, COALESCE(version_major,0), implementation_branch_id, review_branch_id
             FROM plans
             WHERE conversation_id = ?1 AND status NOT IN ('done','abandoned')
             ORDER BY updated_at DESC LIMIT 1",
            [conversation_id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
        )
        .ok();
    let (plan_id, plan_title, plan_phase, plan_major, impl_branch_id, review_branch_id) = plan_row?;

    // 2) 최근 plan_event 확인 — revision 트리거가 있어야만 주입
    let trigger_types = [
        "doom_loop_escalated", "architect_redesign_requested",
        "review_failed", "revision_requested", "plan_full_revision_requested",
    ];
    let latest_event: Option<String> = conn
        .query_row(
            "SELECT event_type FROM plan_events
             WHERE plan_id = ?1 ORDER BY created_at DESC LIMIT 1",
            [&plan_id],
            |r| r.get::<_, String>(0),
        )
        .ok();
    let trigger = latest_event.as_deref().map(|e| trigger_types.contains(&e)).unwrap_or(false);
    if !trigger { return None; }

    // 3) impl branch 변경 파일 목록 (artifacts.file_refs JSON 에서 수집)
    let mut files: Vec<String> = Vec::new();
    if let Some(ref bid) = impl_branch_id {
        let shadow = format!("branch:{}", bid);
        let mut stmt = conn.prepare(
            "SELECT file_refs FROM artifacts
             WHERE conversation_id = ?1 AND file_refs IS NOT NULL AND file_refs != ''
             ORDER BY created_at ASC",
        ).ok()?;
        let rows = stmt.query_map([&shadow], |r| r.get::<_, String>(0)).ok()?;
        for row in rows.flatten() {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&row) {
                if let Some(arr) = v.as_array() {
                    for f in arr {
                        if let Some(s) = f.as_str() {
                            if !files.contains(&s.to_string()) { files.push(s.to_string()); }
                        }
                    }
                }
            }
        }
    }

    // 4) 가장 최근 review_verdict findings
    let latest_findings: Vec<String> = if let Some(ref rid) = review_branch_id {
        let shadow = format!("branch:{}", rid);
        let mut stmt = match conn.prepare(
            "SELECT content FROM messages
             WHERE conversation_id = ?1 AND role = 'assistant' AND status = 'done'
             ORDER BY timestamp DESC LIMIT 5",
        ) { Ok(s) => s, Err(_) => return None };
        stmt.query_map([&shadow], |r| r.get::<_, String>(0))
            .map(|rows| rows.filter_map(|r| r.ok())
                .find(|c| c.contains("<!-- tunaflow:review-verdict -->") || c.contains("verdict:"))
                .map(|c| {
                    let mut out = Vec::new();
                    let mut in_findings = false;
                    for line in c.lines() {
                        if line.trim_start().to_lowercase().starts_with("findings:") { in_findings = true; continue; }
                        if line.trim_start().to_lowercase().starts_with("recommendations:") { in_findings = false; }
                        if in_findings {
                            if let Some(stripped) = line.trim_start().strip_prefix("- ") {
                                out.push(stripped.to_string());
                            } else if let Some(stripped) = line.trim_start().strip_prefix("* ") {
                                out.push(stripped.to_string());
                            }
                        }
                    }
                    out
                })
                .unwrap_or_default()
            )
            .unwrap_or_default()
    } else { Vec::new() };

    // 5) 섹션 조립
    let mut s = String::from("## Previous Implementation Status (for revision context)\n\n");
    s.push_str(&format!("- Plan: \"{}\" (phase={}, major={})\n", plan_title, plan_phase, plan_major));
    if let Some(ref bid) = impl_branch_id {
        s.push_str(&format!("- Impl branch: `branch:{}` (archive 예정 on overwrite)\n", &bid[..bid.len().min(12)]));
    }
    if let Some(ref bid) = review_branch_id {
        s.push_str(&format!("- Review branch: `branch:{}`\n", &bid[..bid.len().min(12)]));
    }
    s.push_str(&format!("- 마지막 트리거 이벤트: `{}`\n", latest_event.as_deref().unwrap_or("?")));
    if !files.is_empty() {
        s.push_str("\n**Changed files (이전 impl 에서 수정됨)**:\n");
        for f in files.iter().take(20) { s.push_str(&format!("- `{}`\n", f)); }
        if files.len() > 20 { s.push_str(&format!("- … (+{}개 생략)\n", files.len() - 20)); }
    }
    if !latest_findings.is_empty() {
        s.push_str("\n**최근 Review findings**:\n");
        for (i, f) in latest_findings.iter().take(10).enumerate() {
            let truncated: String = f.chars().take(200).collect();
            s.push_str(&format!("{}. {}\n", i + 1, truncated));
        }
    }
    s.push_str("\n> rev.N 제안 시 위 파일 각각에 대해 Keep/Modify/Revert 방침을 subtask details 에 명시하세요 (docs/agents/architect.md 의 \"Revision 작성 시 행동 요령\" 참조).\n");

    Some(s)
}

/// Search failure lessons relevant to the current rework context.
/// Uses FTS5 keyword search + file path matching from the combined plan document.
fn search_failure_lessons_for_rework(
    conn: &Connection,
    project_key: &str,
    plan_document: &str,
) -> Vec<crate::db::models::FailureLesson> {
    use rusqlite::params;

    // Extract file paths mentioned in the plan document
    let file_paths: Vec<String> = plan_document
        .split_whitespace()
        .filter_map(|w| {
            let clean = w.trim_matches(|c: char| c == '`' || c == '\'' || c == '"' || c == '(' || c == ')' || c == ',');
            if clean.contains('/') && clean.contains('.') && clean.len() > 4 && !clean.starts_with("http") {
                Some(clean.to_string())
            } else {
                None
            }
        })
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .take(10)
        .collect();

    // Extract keywords from the last review section (after "## Findings" or similar)
    let query_text = plan_document
        .rsplit_once("Finding")
        .or_else(|| plan_document.rsplit_once("finding"))
        .map(|(_, rest)| rest)
        .unwrap_or(plan_document);
    let keywords: Vec<&str> = query_text
        .split(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
        .filter(|t| t.len() >= 3)
        .take(20)
        .collect();
    let fts_query = keywords.join(" OR ");

    let mut results: Vec<(f64, crate::db::models::FailureLesson)> = Vec::new();

    // FTS5 search
    if !fts_query.is_empty() {
        let sql = "SELECT fl.id, fl.project_key, fl.plan_id, fl.file_path, fl.pattern, fl.finding, fl.resolution, fl.created_at,
                          bm25(failure_lessons_fts, 1.0, 0.5, 0.3) AS score
                   FROM failure_lessons_fts
                   JOIN failure_lessons fl ON fl.rowid = failure_lessons_fts.rowid
                   WHERE failure_lessons_fts MATCH ?1
                     AND fl.project_key = ?2
                   ORDER BY score
                   LIMIT 10";
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map(params![fts_query, project_key], |row| {
                Ok((row.get::<_, f64>(8)?, crate::db::models::FailureLesson {
                    id: row.get(0)?,
                    project_key: row.get(1)?,
                    plan_id: row.get(2)?,
                    file_path: row.get(3)?,
                    pattern: row.get(4)?,
                    finding: row.get(5)?,
                    resolution: row.get(6)?,
                    created_at: row.get(7)?,
                }))
            }) {
                for r in rows.flatten() {
                    results.push(r);
                }
            }
        }
    }

    // File path exact match
    for fp in &file_paths {
        let sql = "SELECT id, project_key, plan_id, file_path, pattern, finding, resolution, created_at
                   FROM failure_lessons WHERE project_key = ?1 AND file_path = ?2 ORDER BY created_at DESC LIMIT 3";
        if let Ok(mut stmt) = conn.prepare(sql) {
            if let Ok(rows) = stmt.query_map(params![project_key, fp], |row| {
                Ok(crate::db::models::FailureLesson {
                    id: row.get(0)?,
                    project_key: row.get(1)?,
                    plan_id: row.get(2)?,
                    file_path: row.get(3)?,
                    pattern: row.get(4)?,
                    finding: row.get(5)?,
                    resolution: row.get(6)?,
                    created_at: row.get(7)?,
                })
            }) {
                for lesson in rows.flatten() {
                    if !results.iter().any(|(_, l)| l.id == lesson.id) {
                        results.push((-10.0, lesson));
                    }
                }
            }
        }
    }

    // Sort, deduplicate, limit to 5
    results.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut seen = std::collections::HashSet::new();
    results.into_iter()
        .filter(|(_, l)| seen.insert(l.id.clone()))
        .take(5)
        .map(|(_, l)| l)
        .collect()
}
