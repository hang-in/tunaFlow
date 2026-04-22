mod agents;
pub mod bootstrap;
#[cfg_attr(test, allow(dead_code))]
pub mod commands;
pub mod db;
mod errors;
mod guardrail;
mod http_api;

/// Thread-aware cooperative cancellation registry.
/// Keys are conversation IDs (including branch shadow IDs like "branch:xxx").
/// A thread checks its own conversation_id; only sees its own cancel flag.
pub struct CancelRegistry(pub std::sync::Arc<parking_lot::Mutex<std::collections::HashSet<String>>>);

impl CancelRegistry {
    /// Mark a conversation/thread as cancelled.
    pub fn cancel(&self, conversation_id: &str) {
        let mut set = self.0.lock();
        set.insert(conversation_id.to_string());
    }

    /// Check and consume the cancel flag for a conversation/thread.
    /// Returns true if cancelled (and clears the flag).
    pub fn check_and_consume(&self, conversation_id: &str) -> bool {
        let mut set = self.0.lock();
        set.remove(conversation_id)
    }

    /// Clear cancel flag for a conversation (e.g., on normal completion).
    pub fn clear(&self, conversation_id: &str) {
        let mut set = self.0.lock();
        set.remove(conversation_id);
    }
}

pub fn run() {
    bootstrap::env::inherit_shell_path();
    bootstrap::crash::install_panic_hook();

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_window_state::Builder::new().build())
        .setup(|app| {
            use tauri::Manager;

            bootstrap::db::init_db(app)?;

            // `CancelRegistry` is cross-cutting state; services (HTTP API)
            // receive a shared Arc before we hand the registry to `app.manage`.
            let cancel_registry = CancelRegistry(std::sync::Arc::new(parking_lot::Mutex::new(
                std::collections::HashSet::new(),
            )));
            let cancel_arc = std::sync::Arc::clone(&cancel_registry.0);
            bootstrap::services::start_background_services(app, cancel_arc)?;
            app.manage(cancel_registry);

            bootstrap::window::restore_window_state(app)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Project
            commands::projects::list_projects,
            commands::projects::create_project,
            commands::projects::get_project,
            commands::projects::hide_project,
            commands::projects::validate_project_path,
            commands::project_tools::ensure_rawq_index,
            commands::project_tools::ensure_project_workflow_templates,
            commands::projects::refresh_project_stack_info,
            commands::agent_detect::detect_available_agents,
            commands::project_onboarding::analyze_project_for_onboarding,
            commands::project_onboarding::cancel_project_onboarding,
            commands::project_onboarding::apply_project_onboarding,
            commands::project_tools::get_project_cli_permissions,
            commands::project_tools::set_project_cli_permissions,
            commands::project_tools::start_rawq_index,
            commands::project_tools::get_rawq_status,
            commands::project_tools::get_git_status,
            // Conversation
            commands::conversations::list_conversations,
            commands::conversations::create_conversation,
            commands::conversations::get_conversation,
            commands::conversations::delete_conversation,
            commands::conversations::rename_conversation,
            commands::conversations::save_rt_config,
            commands::conversations::get_rt_config,
            commands::conversations::update_resume_token,
            // Message
            commands::messages::list_messages,
            commands::messages::create_user_message,
            commands::messages::append_user_message,
            commands::messages::append_assistant_message,
            commands::messages::update_message_status,
            commands::messages::get_progress_content,
            commands::messages::save_progress_content,
            commands::messages::delete_message_pair,
            commands::messages::search_messages,
            commands::search::unified::search_unified,
            // Branch
            commands::branches::list_branches,
            commands::branches::create_branch,
            commands::branches::adopt_branch,
            commands::branches::archive_branch,
            commands::branches::delete_branch,
            commands::branches::rename_branch,
            commands::branches::link_git_branch,
            commands::branches::create_git_branch,
            commands::branches::checkout_git_branch,
            commands::branches::open_branch_stream,
            // Agent (background start_* commands only)
            commands::agents::start_claude_stream,
            commands::agents::start_gemini_stream,
            commands::agents::start_codex_run,
            commands::agents::start_opencode_run,
            commands::agents::start_openai_compat_stream,
            commands::agents::run_eval_agent,
            commands::agents::get_claude_mode,
            commands::agents::restart_sdk_session,
            commands::agents::prewarm_sdk_session,
            commands::agents::has_active_sdk_session,
            commands::agents::persist_system_msg,
            commands::diagnostics::get_rate_limit_info,
            // Crash reports (Phase 4 Finding 4-3)
            commands::crash_reports::list_recent_crash_reports,
            commands::crash_reports::log_js_error,
            // Jobs
            commands::jobs::list_active_jobs,
            commands::jobs::cleanup_stale_jobs,
            commands::jobs::on_run_completed,
            // Roundtable
            commands::roundtable::roundtable_run,
            commands::roundtable::roundtable_followup,
            commands::roundtable::cancel_running,
            commands::roundtable::start_roundtable_run,
            commands::roundtable::start_roundtable_followup,
            // Skill
            commands::skills::list_skills,
            commands::skills::list_skills_with_project,
            commands::skills::get_skill,
            commands::skills::get_skills_snapshot,
            commands::skills::detect_project_stack,
            commands::skills::search_skill_registry,
            commands::skills::install_registry_skill,
            commands::skills::build_skill_pack,
            // Memo
            commands::memos::list_memos,
            commands::memos::list_memos_by_conversation,
            commands::memos::create_memo,
            commands::memos::get_branch_brief,
            commands::memos::delete_memo,
            commands::meta_notifications::create_meta_notification,
            commands::meta_notifications::list_meta_notifications,
            commands::meta_notifications::mark_meta_notification_read,
            commands::meta_notifications::mark_all_meta_notifications_read,
            commands::meta_notifications::dismiss_meta_notification,
            commands::meta_notifications::clear_meta_notifications,
            // Artifact
            commands::artifacts::list_artifacts,
            commands::artifacts::list_artifacts_by_branch,
            commands::artifacts::create_artifact,
            commands::artifacts::update_artifact_status,
            commands::artifacts::link_artifact_to_subtask,
            commands::artifacts::delete_artifact,
            // Models
            commands::model_discovery::list_engine_models,
            commands::model_discovery::refresh_engine_models,
            // Capability
            commands::capabilities::list_capabilities,
            // Evaluation
            commands::evaluation::create_eval_run,
            commands::evaluation::list_eval_runs,
            commands::evaluation::add_eval_result,
            commands::evaluation::list_eval_results,
            commands::evaluation::update_eval_run_status,
            commands::evaluation::delete_eval_run,
            // Conversation Memory
            commands::conversation_memory::get_conversation_memory_status,
            commands::conversation_memory::list_memory_topics,
            commands::conversation_memory::list_recent_turns,
            commands::conversation_memory::probe_message,
            commands::conversation_memory::fetch_message_slice,
            commands::conversation_memory::fetch_full_message,
            commands::conversation_memory::compress_conversation_memory,
            commands::conversation_memory::force_recompress_memory,
            // Session Discovery
            commands::session_discovery::get_session_links,
            commands::session_discovery::refresh_session_links,
            commands::session_discovery::toggle_manual_session_link,
            // Vector Search
            commands::vector_search::index_conversation_chunks,
            commands::vector_search::search_conversation_vectors,
            commands::vector_search::search_memory_semantic,
            commands::vector_search::get_vector_index_status,
            // Context Hub
            commands::context_hub::context_hub_health,
            commands::context_hub::context_hub_search,
            commands::context_hub::context_hub_get,
            // Files
            commands::files::list_directory,
            commands::files::read_file_content,
            commands::files::read_text_file,
            // Tracing
            commands::tracing::list_traces,
            commands::tracing::export_traces_otel,
            // Plan
            commands::plans::create_plan,
            commands::plans::get_plan,
            commands::plans::list_plans_by_conversation,
            commands::plans::list_plans_by_project,
            commands::plans::get_active_plan_phase,
            commands::plans::count_active_plans,
            commands::plans::update_plan_status,
            commands::plans::update_plan_meta,
            commands::plans::list_subtasks,
            commands::plans::set_subtask_owner,
            commands::plans::update_subtask_status,
            commands::plans::replace_plan_subtasks,
            commands::plans::delete_plan,
            commands::plans::update_plan_phase,
            commands::plans::create_plan_event,
            commands::plans::list_plan_events,
            commands::plans::assign_plan_engines,
            commands::plans::link_plan_branch,
            commands::plans::find_plan_by_branch,
            commands::plans::bump_plan_major_version,
            commands::plans::generate_plan_document,
            commands::plans::generate_review_report,
            commands::plans::generate_result_report,
            // Failure Lessons
            commands::failure_lessons::create_failure_lesson,
            commands::failure_lessons::create_failure_lessons_batch,
            commands::failure_lessons::list_failure_lessons,
            commands::failure_lessons::search_similar_failures,
            commands::failure_lessons::resolve_failure_lesson,
            commands::failure_lessons::resolve_failure_lessons_by_plan,
            commands::failure_lessons::delete_failure_lesson,
            // Test Runner
            commands::test_runner::run_project_tests,
            // Insight
            commands::insight::create_insight_session,
            commands::insight::get_insight_session,
            commands::insight::list_insight_sessions,
            commands::insight::update_insight_session_status,
            commands::insight::delete_insight_session,
            commands::insight::create_insight_findings_batch,
            commands::insight::list_insight_findings,
            commands::insight::count_open_insight_findings,
            commands::insight::update_insight_finding_status,
            commands::insight::update_insight_findings_batch_status,
            commands::insight::resolve_insight_findings_by_plan,
            commands::insight::link_insight_findings_to_branch,
            commands::insight::resolve_insight_findings_by_branch,
            commands::insight::create_insight_report,
            commands::insight::list_insight_reports,
            commands::insight::export_insight_to_files,
            commands::insight_extract::run_insight_extraction,
            commands::insight_extract::run_insight_analysis,
            // Conventions sync (Phase 2 — ContextPack 정적 레이어 외부화 토글)
            commands::conventions_sync::list_project_conventions,
            commands::conventions_sync::set_project_convention,
            commands::conventions_sync::delete_project_convention,
            commands::conventions_sync::sync_project_conventions,
            commands::conventions_sync::get_project_conventions_sync,
            commands::conventions_sync::set_project_conventions_sync,
            // Secrets (OS keychain)
            commands::secrets::secret_set,
            commands::secrets::secret_get,
            commands::secrets::secret_has,
            commands::secrets::secret_delete,
            // Attachments — 첨부 파일 저장/삭제/정리
            commands::attachments::save_attachment,
            commands::attachments::delete_attachment,
            commands::attachments::cleanup_attachments,
            // PTY
            commands::pty::pty_spawn,
            commands::pty::pty_write,
            commands::pty::pty_get_screen,
            commands::pty::pty_is_alive,
            commands::pty::pty_resize,
            commands::pty::pty_kill,
            commands::pty::pty_poll_jsonl,
            commands::pty::pty_list_jsonl_files,
            commands::pty::pty_build_context,
            commands::pty::pty_update_claude_md,
            commands::pty::pty_poll_codex,
            commands::pty::pty_poll_gemini,
            commands::pty::pty_list_codex_files,
            commands::pty::pty_list_gemini_files,
            commands::pty::pty_kill_all,

            // Document Index (docs/ RAG)
            commands::document_index::index_project_docs,
            commands::document_index::cleanup_project_stale_docs,
            commands::document_index::search_project_docs,
            commands::document_index::get_project_document_graph,
            commands::document_index::get_orphan_documents,
            commands::document_index::get_document_index_status,
            // Mobile pairing
            commands::mobile::get_api_connection_info,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                use tauri::Manager;
                use tauri_plugin_window_state::{AppHandleExt, StateFlags};
                let _ = window.app_handle().save_window_state(StateFlags::all());
                // Kill all sdk-url/app-server sessions to prevent orphan processes
                crate::agents::claude_sdk_session::shutdown_all_sessions();
                crate::agents::claude_sdk_session::kill_orphan_sdk_processes();
                eprintln!("[shutdown] all agent sessions terminated");
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("tunaFlow failed to start: {e}");
            std::process::exit(1);
        })
}
