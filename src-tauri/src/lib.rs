mod agents;
#[cfg_attr(test, allow(dead_code))]
pub mod commands;
pub mod db;
mod errors;
mod guardrail;
mod http_api;

use db::DbState;

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

/// Inherit the user's login-shell PATH.
///
/// macOS .app bundles launched from Finder/Launchpad get a minimal PATH
/// (`/usr/bin:/bin:/usr/sbin:/sbin`) and miss user-installed CLI agents such as
/// `claude`, `codex`, `gemini` that live under `/opt/homebrew/bin`,
/// `~/.npm-global/bin`, `~/.nvm/...`, or similar.
/// `npm run tauri dev` works because the terminal already has a full PATH;
/// the bundled app does not. Spawn the user's login shell once and adopt
/// its PATH so every subsequent `Command::new("claude")` resolves correctly.
fn inherit_shell_path() {
    #[cfg(target_os = "macos")]
    {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".into());
        let output = std::process::Command::new(&shell)
            .args(["-l", "-c", "echo -n $PATH"])
            .output();
        if let Ok(out) = output {
            if out.status.success() {
                if let Ok(path) = String::from_utf8(out.stdout) {
                    let trimmed = path.trim();
                    if !trimmed.is_empty() && trimmed != std::env::var("PATH").unwrap_or_default() {
                        eprintln!("[startup] PATH inherited from {} login shell", shell);
                        std::env::set_var("PATH", trimmed);
                    }
                }
            }
        }
    }
}

pub fn run() {
    inherit_shell_path();

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
            let data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from(".tunaflow_data"));
            std::fs::create_dir_all(&data_dir)?;
            let db_path = data_dir.join("tunaflow.db");
            let (write_conn, read_conn) = db::init(db_path)?;

            // Cleanup stale streaming messages from previous crash/shutdown
            let cleaned = write_conn.execute(
                "UPDATE messages SET status = 'error', content = CASE WHEN content = '' THEN '(이전 세션에서 중단됨)' ELSE content END WHERE status = 'streaming'",
                [],
            ).unwrap_or_else(|e| { eprintln!("[startup] stale message cleanup failed: {e}"); 0 });
            let jobs = write_conn.execute(
                "UPDATE agent_jobs SET status = 'failed', error = 'app restart' WHERE status = 'running'",
                [],
            ).unwrap_or_else(|e| { eprintln!("[startup] stale job cleanup failed: {e}"); 0 });
            if cleaned > 0 || jobs > 0 {
                eprintln!("[startup] Cleaned {} stale streaming messages, {} stale jobs", cleaned, jobs);
            }

            app.manage(DbState {
                write: std::sync::Arc::new(std::sync::Mutex::new(write_conn)),
                read: std::sync::Arc::new(std::sync::Mutex::new(read_conn)),
            });

            let cancel_registry = CancelRegistry(std::sync::Arc::new(parking_lot::Mutex::new(std::collections::HashSet::new())));

            // Start HTTP API server (E2E testing + mobile access + MCP)
            {
                let db_state = app.state::<DbState>().inner().clone();
                let cancel_arc = std::sync::Arc::clone(&cancel_registry.0);
                let api_token = http_api::start_server(db_state, app.handle().clone(), cancel_arc);
                eprintln!("[startup] HTTP API token: {}", api_token);
            }

            app.manage(cancel_registry);
            app.manage(commands::pty::PtyState::new());
            app.manage(commands::projects::RawqIndexing(std::sync::Arc::new(parking_lot::Mutex::new(std::collections::HashSet::new()))));

            // Center window on primary monitor if no saved window state
            if let Some(window) = app.get_webview_window("main") {
                // window-state plugin restores position before setup runs.
                // If position is (0,0) or negative, assume no saved state → center on primary monitor.
                let needs_center = match window.outer_position() {
                    Ok(pos) => pos.x == 0 && pos.y == 0,
                    Err(_) => true,
                };
                if needs_center {
                    if let Some(monitor) = window.primary_monitor().ok().flatten() {
                        let screen = monitor.size();
                        let scale = monitor.scale_factor();
                        let mon_pos = monitor.position();
                        let win_w = 1200.0;
                        let win_h = 800.0;
                        let x = mon_pos.x as f64 + (screen.width as f64 / scale - win_w) / 2.0;
                        let y = mon_pos.y as f64 + (screen.height as f64 / scale - win_h) / 2.0;
                        let _ = window.set_position(tauri::PhysicalPosition::new(
                            (x * scale) as i32,
                            (y * scale) as i32,
                        ));
                    }
                }
                let _ = window.show();
            }

            // Start rawq daemon in background — pre-loads embedding model for fast indexing/search
            std::thread::spawn(|| {
                crate::agents::rawq::ensure_daemon();
            });

            // Initialize bge-m3 embedder (document/conversation search)
            // Try sync init first (if model already cached), then async download if needed
            if let Err(e) = crate::agents::embedder::init_global_embedder() {
                eprintln!("[startup] bge-m3 sync init error: {}", e);
            }
            if crate::agents::embedder::get_embedder().is_none() {
                tauri::async_runtime::spawn(async {
                    if let Err(e) = crate::agents::embedder::init_global_embedder_async().await {
                        eprintln!("[startup] bge-m3 async download/init error: {}", e);
                    }
                });
            }

            // Backfill NULL-embedding chunks left over from v32 (bge-m3 migration).
            // Sleeps 15s before starting to let embedder/rawq settle, then processes
            // one conversation/project at a time with throttling.
            crate::commands::vector_search::spawn_startup_backfill(
                app.state::<DbState>().inner().clone(),
            );

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
            commands::conversation_memory::compress_conversation_memory,
            commands::conversation_memory::force_recompress_memory,
            // Session Discovery
            commands::session_discovery::get_session_links,
            commands::session_discovery::refresh_session_links,
            commands::session_discovery::toggle_manual_session_link,
            // Vector Search
            commands::vector_search::index_conversation_chunks,
            commands::vector_search::search_conversation_vectors,
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
            commands::plans::get_active_plan_phase,
            commands::plans::count_active_plans,
            commands::plans::update_plan_status,
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
            }
        })
        .run(tauri::generate_context!())
        .unwrap_or_else(|e| {
            eprintln!("tunaFlow failed to start: {e}");
            std::process::exit(1);
        })
}
