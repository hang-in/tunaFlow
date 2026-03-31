mod agents;
mod commands;
pub mod db;
mod errors;
mod guardrail;

use db::DbState;

/// Thread-aware cooperative cancellation registry.
/// Keys are conversation IDs (including branch shadow IDs like "branch:xxx").
/// A thread checks its own conversation_id; only sees its own cancel flag.
pub struct CancelRegistry(pub std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>>);

impl CancelRegistry {
    /// Mark a conversation/thread as cancelled.
    pub fn cancel(&self, conversation_id: &str) {
        if let Ok(mut set) = self.0.lock() {
            set.insert(conversation_id.to_string());
        }
    }

    /// Check and consume the cancel flag for a conversation/thread.
    /// Returns true if cancelled (and clears the flag).
    pub fn check_and_consume(&self, conversation_id: &str) -> bool {
        if let Ok(mut set) = self.0.lock() {
            set.remove(conversation_id)
        } else {
            false
        }
    }

    /// Clear cancel flag for a conversation (e.g., on normal completion).
    pub fn clear(&self, conversation_id: &str) {
        if let Ok(mut set) = self.0.lock() {
            set.remove(conversation_id);
        }
    }
}

pub fn run() {
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
            app.manage(DbState {
                write: std::sync::Arc::new(std::sync::Mutex::new(write_conn)),
                read: std::sync::Arc::new(std::sync::Mutex::new(read_conn)),
            });
            app.manage(CancelRegistry(std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()))));
            app.manage(commands::projects::RawqIndexing(std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()))));

            // Center window on primary monitor if no saved window state
            {
                let window = app.get_webview_window("main").expect("main window");
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
            // Message
            commands::messages::list_messages,
            commands::messages::create_user_message,
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
            commands::agents::run_eval_agent,
            // Jobs
            commands::jobs::list_active_jobs,
            commands::jobs::cleanup_stale_jobs,
            // Roundtable
            commands::roundtable::roundtable_run,
            commands::roundtable::roundtable_followup,
            commands::roundtable::cancel_running,
            commands::roundtable::start_roundtable_run,
            commands::roundtable::start_roundtable_followup,
            // Skill
            commands::skills::list_skills,
            commands::skills::get_skill,
            commands::skills::get_skills_snapshot,
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
            commands::conversation_memory::compress_conversation_memory,
            // Context Hub
            commands::context_hub::context_hub_health,
            commands::context_hub::context_hub_search,
            commands::context_hub::context_hub_get,
            // Files
            commands::files::list_directory,
            commands::files::read_text_file,
            // Tracing
            commands::tracing::list_traces,
            commands::tracing::export_traces_otel,
            // Plan
            commands::plans::create_plan,
            commands::plans::get_plan,
            commands::plans::list_plans_by_conversation,
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
            commands::plans::generate_plan_document,
            // Test Runner
            commands::test_runner::run_project_tests,
        ])
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { .. } = event {
                use tauri::Manager;
                use tauri_plugin_window_state::{AppHandleExt, StateFlags};
                let _ = window.app_handle().save_window_state(StateFlags::all());
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tunaFlow")
}
