mod context_loading;
mod prompt_assembly;
mod persistence;
pub mod session_freshness;
pub mod agent_session_tx;

// Re-export everything so external `use crate::commands::agents_helpers::send_common::*` still works
#[allow(unused_imports)]
pub use context_loading::{ContextData, load_context_data, load_project_path, build_lite_enriched_prompt};
#[allow(unused_imports)]
pub use prompt_assembly::{assemble_prompt, build_normalized_prompt, build_normalized_prompt_with_budget};
#[allow(unused_imports)]
pub use persistence::{persist_user_message, persist_system_message, PreparedRun, prepare_engine_run, finalize_engine_run, spawn_post_completion_tasks, AgentRunResult, persist_assistant_message, persist_assistant_message_with_id};

pub use super::identity::*;
#[allow(unused_imports)]
pub use super::trace_log::ContextPackMeta;

#[cfg(test)]
mod tests {
    use super::*;

    // ─── assemble_prompt (pure function) ────────────────────────────────

    fn empty_context_data() -> ContextData {
        ContextData {
            conversation_id: "test-conv".into(),
            project_path: Some("/tmp/test".into()),
            prompt: "hello".into(),
            is_branch: false,
            has_active_plan: false,
            current_messages: vec![],
            parent_messages: vec![],
            plan_section: None,
            plan_document: None,
            findings_section: None,
            artifacts_section: None,
            retrieval_chunks: vec![],
            document_chunks: vec![],
            compressed_memory: None,
            compressed_memory_source: None,
            cross_session_data: vec![],
            previous_impl_status: None,
            thread_inheritance: None,
            agent_role_doc: None,
            active_skills: vec![],
            cross_session_ids: vec![],
            persona_fragment: None,
            context_mode_override: None,
            context_budget_cap: None,
            user_profile: None,
            conventions_synced: false,
            is_session_continuation: false,
        }
    }

    #[test]
    fn assemble_empty_data_returns_prompt_only() {
        let data = empty_context_data();
        let (assembled, _sys_ctx, meta) = assemble_prompt(&data, None);
        assert!(assembled.contains("hello"));
        // project section should be present
        assert!(meta.sections.contains(&"project".to_string()));
    }

    #[test]
    fn assemble_with_plan_includes_plan_section() {
        let mut data = empty_context_data();
        data.plan_section = Some("## Active Plan\n\n### Migration\n\n**Progress:** 2/5 done".into());
        data.context_mode_override = Some("standard".into());
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"plan".to_string()));
    }

    #[test]
    fn auto_mode_short_prompt_selects_lite() {
        let mut data = empty_context_data();
        data.prompt = "ㅇㅇ".into();
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Lite"), "expected Lite mode, got: {}", meta.mode);
    }

    #[test]
    fn auto_mode_with_skills_pushes_toward_full() {
        let mut data = empty_context_data();
        data.active_skills = vec!["a".into(), "b".into(), "c".into()]; // +2
        data.cross_session_ids = vec!["other-conv".into()];            // +1  → total ≥ 3
        data.prompt = "코드를 리팩토링해주세요. 이 함수가 너무 길어요.".into();
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Full"), "expected Full mode, got: {}", meta.mode);
    }

    // ─── Mode override tests ─────────────────────────────────────────────

    #[test]
    fn mode_override_full_ignores_auto_scoring() {
        let mut data = empty_context_data();
        data.prompt = "hi".into(); // normally Lite (short)
        data.context_mode_override = Some("full".into());
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Full"), "override should force Full, got: {}", meta.mode);
    }

    #[test]
    fn mode_override_lite_ignores_skills() {
        let mut data = empty_context_data();
        data.active_skills = vec!["a".into(), "b".into(), "c".into(), "d".into()];
        data.context_mode_override = Some("lite".into());
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Lite"), "override should force Lite, got: {}", meta.mode);
    }

    #[test]
    fn mode_override_standard_explicit() {
        let mut data = empty_context_data();
        data.context_mode_override = Some("standard".into());
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Standard"), "got: {}", meta.mode);
    }

    // ─── Auto mode scoring edge cases ────────────────────────────────────

    #[test]
    fn auto_mode_branch_plus_plan_reaches_standard() {
        let mut data = empty_context_data();
        data.is_branch = true;        // +1
        data.has_active_plan = true;   // +1 → total=2, Standard
        data.prompt = "이 브랜치 작업을 계속해주세요.".into();
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Standard"), "expected Standard, got: {}", meta.mode);
    }

    #[test]
    fn auto_mode_persona_contributes() {
        let mut data = empty_context_data();
        data.persona_fragment = Some("## Persona\n\nYou are a security reviewer.".into());
        data.prompt = "코드 리뷰를 진행해주세요.".into();
        // persona +1, Standard baseline
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(!meta.mode.contains("Lite"), "persona should prevent Lite, got: {}", meta.mode);
    }

    #[test]
    fn auto_mode_long_conversation_floors_at_standard() {
        let mut data = empty_context_data();
        data.prompt = "ㅇ".into(); // very short → normally -2 (Lite)
        // 20+ messages → floor at Standard
        for i in 0..22 {
            data.current_messages.push((
                if i % 2 == 0 { "user".into() } else { "assistant".into() },
                format!("message {}", i),
                Some("claude".into()),
                None,
            ));
        }
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Standard"), "long conv should floor at Standard, got: {}", meta.mode);
    }

    #[test]
    fn auto_mode_history_signal_word_boosts() {
        let mut data = empty_context_data();
        data.prompt = "이전 논의를 정리해줘".into(); // contains history signal words
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(!meta.mode.contains("Lite"), "history signal should prevent Lite, got: {}", meta.mode);
    }

    // ─── Section inclusion / exclusion ───────────────────────────────────

    #[test]
    fn lite_mode_skips_plan_findings_artifacts() {
        let mut data = empty_context_data();
        data.prompt = "ㅇ".into();
        data.plan_section = Some("## Plan\n\ntask list".into());
        data.findings_section = Some("## Findings\n\nbug report".into());
        data.artifacts_section = Some("## Artifacts\n\ncode snippet".into());
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Lite"));
        assert!(!meta.sections.contains(&"plan".to_string()), "Lite should skip plan");
        assert!(!meta.sections.contains(&"findings".to_string()), "Lite should skip findings");
        assert!(!meta.sections.contains(&"artifacts".to_string()), "Lite should skip artifacts");
    }

    #[test]
    fn standard_mode_includes_plan_findings_artifacts() {
        let mut data = empty_context_data();
        data.context_mode_override = Some("standard".into());
        data.plan_section = Some("## Plan\n\ntask list".into());
        data.findings_section = Some("## Findings\n\nbug report".into());
        data.artifacts_section = Some("## Artifacts\n\ncode snippet".into());
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"plan".to_string()));
        assert!(meta.sections.contains(&"findings".to_string()));
        assert!(meta.sections.contains(&"artifacts".to_string()));
        assert!(assembled.contains("task list"));
    }

    #[test]
    fn platform_section_always_included() {
        let data = empty_context_data();
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"platform".to_string()));
    }

    #[test]
    fn thread_inheritance_only_for_branches() {
        let mut data = empty_context_data();
        data.thread_inheritance = Some("Parent context: ...".into());
        data.is_branch = false;
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(!meta.sections.contains(&"thread-inheritance".to_string()));

        data.is_branch = true;
        let (assembled, _, meta2) = assemble_prompt(&data, None);
        assert!(meta2.sections.contains(&"thread-inheritance".to_string()));
        assert!(assembled.contains("Parent context"));
    }

    #[test]
    fn plan_document_included_at_standard() {
        let mut data = empty_context_data();
        data.context_mode_override = Some("standard".into());
        data.plan_document = Some("# Migration Plan\n\n## Steps\n\n1. backup\n2. migrate".into());
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"plan-document".to_string()));
        assert!(assembled.contains("Migration Plan"));
    }

    #[test]
    fn agent_role_doc_injected() {
        let mut data = empty_context_data();
        data.agent_role_doc = Some("You are an Architect. Plan before implementing.".into());
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"agent-role".to_string()));
        assert!(assembled.contains("Agent Role Instructions"));
    }

    // ─── Participants meta section ───────────────────────────────────────

    #[test]
    fn participants_meta_tracks_agents() {
        let mut data = empty_context_data();
        data.current_messages = vec![
            ("user".into(), "질문".into(), None, None),
            ("assistant".into(), "답변 A".into(), Some("claude".into()), Some("Architect".into())),
            ("assistant".into(), "답변 B".into(), Some("gemini".into()), None),
        ];
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"participants".to_string()));
        assert!(assembled.contains("Architect (claude)"));
        assert!(assembled.contains("(gemini)"));
    }

    #[test]
    fn participants_meta_empty_when_no_assistants() {
        let mut data = empty_context_data();
        data.current_messages = vec![
            ("user".into(), "질문".into(), None, None),
        ];
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(!meta.sections.contains(&"participants".to_string()));
    }

    // ─── Author attribution in context ───────────────────────────────────

    #[test]
    fn recent_context_preserves_author_attribution() {
        let mut data = empty_context_data();
        data.context_mode_override = Some("standard".into());
        data.current_messages = vec![
            ("user".into(), "explain X".into(), None, None),
            ("assistant".into(), "X is a framework for...".into(), Some("claude".into()), Some("DevBot".into())),
        ];
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"context".to_string()));
        // Author attribution should appear in the assembled prompt
        assert!(assembled.contains("DevBot") || assembled.contains("claude"),
            "author attribution missing in context section");
    }

    // ─── Identity block injection ────────────────────────────────────────

    #[test]
    fn identity_fragment_creates_identity_section() {
        let data = empty_context_data();
        // parse_identity_and_persona splits on "\n\n## Persona"
        let fragment = "## Identity\nYou are Claude.\n\n## Persona\nYou review code.";
        let (assembled, _, meta) = assemble_prompt(&data, Some(fragment));
        assert!(meta.sections.contains(&"identity".to_string()));
        assert!(meta.sections.contains(&"persona".to_string()));
        assert!(assembled.contains("You are Claude"));
    }

    #[test]
    fn no_identity_when_fragment_is_none() {
        let data = empty_context_data();
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(!meta.sections.contains(&"identity".to_string()));
        assert!(!meta.sections.contains(&"persona".to_string()));
    }

    // ─── Compressed memory ───────────────────────────────────────────────

    #[test]
    fn compressed_memory_included_when_present() {
        let mut data = empty_context_data();
        data.compressed_memory = Some("Topic: Auth\nDecision: JWT tokens for session management".into());
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"compressed-memory".to_string()));
        assert!(assembled.contains("JWT tokens"));
    }

    // ─── Cross-session data ──────────────────────────────────────────────

    #[test]
    fn cross_session_included_when_present() {
        let mut data = empty_context_data();
        data.context_mode_override = Some("full".into()); // Tiering: cross-session is Tier 2, requires Full mode
        data.cross_session_data = vec![
            ("Other Chat".into(), vec![("user".into(), "context info".into())]),
        ];
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"cross-session".to_string()));
        assert!(assembled.contains("context info"));
    }

    // ─── Retrieval chunks ────────────────────────────────────────────────

    #[test]
    fn retrieval_chunks_included_at_standard() {
        let mut data = empty_context_data();
        data.context_mode_override = Some("standard".into());
        data.retrieval_chunks = vec![
            crate::commands::context_queries::RetrievedChunk {
                kind: "pair",
                messages: vec![
                    ("user".into(), "이전 질문".into(), None, None),
                    ("assistant".into(), "이전 답변".into(), Some("claude".into()), None),
                ],
                conversation_id: "old-conv".into(),
                score: 0.8,
                timestamp: 100,
            },
        ];
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"retrieval".to_string()));
        assert!(assembled.contains("이전 답변"));
    }

    #[test]
    fn retrieval_skipped_when_budget_low() {
        let mut data = empty_context_data();
        data.context_mode_override = Some("standard".into());
        data.context_budget_cap = Some(1000); // very tight budget
        data.retrieval_chunks = vec![
            crate::commands::context_queries::RetrievedChunk {
                kind: "pair",
                messages: vec![("user".into(), "q".into(), None, None)],
                conversation_id: "c".into(),
                score: 0.5,
                timestamp: 1,
            },
        ];
        // Fill up budget with lots of sections
        data.plan_section = Some("x".repeat(800));
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(
            meta.sections.contains(&"retrieval:skipped".to_string()) || meta.sections.contains(&"retrieval".to_string()),
            "retrieval should be attempted or skipped, sections: {:?}", meta.sections
        );
    }

    // ─── Budget cap enforcement ──────────────────────────────────────────

    #[test]
    fn custom_budget_cap_respected() {
        let mut data = empty_context_data();
        data.context_budget_cap = Some(500); // very small
        data.prompt = "short question".into();
        let (assembled, _, meta) = assemble_prompt(&data, None);
        // The assembled prompt should not wildly exceed the budget
        // (some overhead is expected from the prompt itself being appended)
        assert!(meta.truncated || assembled.len() < 1500, "budget should constrain output, len={}", assembled.len());
    }

    // ─── Meta correctness ────────────────────────────────────────────────

    #[test]
    fn meta_hash_is_valid_json() {
        let data = empty_context_data();
        let (_, _, meta) = assemble_prompt(&data, None);
        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&meta.hash);
        assert!(parsed.is_ok(), "meta.hash should be valid JSON: {}", meta.hash);
    }

    #[test]
    fn meta_length_matches_assembled() {
        let data = empty_context_data();
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert_eq!(meta.length, assembled.len());
    }

    #[test]
    fn meta_sections_non_empty() {
        let data = empty_context_data();
        let (_, _, meta) = assemble_prompt(&data, None);
        // At minimum: project + platform
        assert!(meta.sections.len() >= 2, "sections should have at least project + platform: {:?}", meta.sections);
    }

    // ─── Conventions Sync Phase 2 — static layer skip ────────────────────

    #[test]
    fn conventions_synced_skips_platform_section() {
        let mut data = empty_context_data();
        data.conventions_synced = true;
        let (assembled, _, meta) = assemble_prompt(&data, None);
        // Skipped — marker present, content absent
        assert!(meta.sections.contains(&"platform:skipped".to_string()), "should mark platform skipped: {:?}", meta.sections);
        assert!(!meta.sections.contains(&"platform".to_string()));
        // PLATFORM_TIER0 content should NOT appear in assembled output
        assert!(!assembled.contains("tunaFlow platform"), "platform text leaked");
    }

    #[test]
    fn conventions_synced_skips_agent_role_section() {
        let mut data = empty_context_data();
        data.conventions_synced = true;
        data.agent_role_doc = Some("Do the thing".into());
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"agent-role:skipped".to_string()));
        assert!(!assembled.contains("Do the thing"));
    }

    #[test]
    fn conventions_synced_skips_user_profile() {
        let mut data = empty_context_data();
        data.conventions_synced = true;
        data.user_profile = Some(r#"{"name":"Alice","title":"Engineer"}"#.into());
        let (assembled, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"user-profile:skipped".to_string()));
        assert!(!assembled.contains("Name: Alice"));
    }

    #[test]
    fn conventions_not_synced_keeps_static_sections() {
        let mut data = empty_context_data();
        data.conventions_synced = false;
        data.agent_role_doc = Some("Do the thing".into());
        data.user_profile = Some(r#"{"name":"Alice"}"#.into());
        let (assembled, _, meta) = assemble_prompt(&data, None);
        // Default path — all layers present
        assert!(meta.sections.contains(&"platform".to_string()));
        assert!(meta.sections.contains(&"agent-role".to_string()));
        assert!(meta.sections.contains(&"user-profile".to_string()));
        assert!(assembled.contains("Do the thing"));
        assert!(assembled.contains("Name: Alice"));
    }

    #[test]
    fn conventions_synced_preserves_dynamic_layers() {
        let mut data = empty_context_data();
        data.conventions_synced = true;
        data.plan_section = Some("## Active Plan\n\n### Migration".into());
        data.context_mode_override = Some("standard".into());
        let (_, _, meta) = assemble_prompt(&data, None);
        // Dynamic layer stays even when static layers are skipped
        assert!(meta.sections.contains(&"plan".to_string()), "plan should stay: {:?}", meta.sections);
    }
}
