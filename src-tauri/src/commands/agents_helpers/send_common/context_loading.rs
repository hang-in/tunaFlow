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

/// brand:* shadow conv 의 root main conversation_id 를 `branches` 테이블에서 조회.
///
/// `branches.conversation_id` 컬럼에는 이미 root 가 저장되어 있다
/// (`commands/branches.rs::create_branch` 의 `root_conv_id` 정규화).
/// non-branch conv_id 또는 lookup 실패 시 None.
pub fn lookup_branch_root_conv_id(conn: &Connection, conv_id: &str) -> Option<String> {
    let branch_id = conv_id.strip_prefix("branch:")?;
    conn.query_row(
        "SELECT conversation_id FROM branches WHERE id = ?1",
        [branch_id],
        |row| row.get::<_, String>(0),
    )
    .ok()
}

/// brand 가 main 과 같은 엔진을 사용하는지 검사.
///
/// 같은 엔진이면 brand send 가 main 의 sdk-url WS 세션을 그대로 이어받아
/// prior history 가 자동 포함된다 → ContextPack dynamic 섹션 (recent / parent /
/// compressed / retrieval) 재주입은 토큰 낭비 + 오염원.
///
/// 다른 엔진 (예: Claude → Codex) 이면 새 session 이라 ContextPack 정상 빌드 필요.
///
/// engine name normalization:
/// - `claude` / `claude-code` 는 같은 엔진으로 간주 (resume_token_engine 표기 차이)
/// - 그 외는 문자열 일치
///
/// 검사 기준: root main conv 의 가장 최근 assistant 메시지의 `engine` 컬럼.
/// root 에 메시지가 없으면 (= 처음 brand 진입) `true` 로 본다 — 어차피 첫 send
/// 라 LAST_DELIVERED 도 비어 있어 ContextPack full 경로가 자연스럽게 동작.
fn is_engine_continuity(conn: &Connection, root_conv_id: &str, current_engine: &str) -> bool {
    let last_engine: Option<String> = conn
        .query_row(
            "SELECT engine FROM messages
             WHERE conversation_id = ?1 AND role = 'assistant' AND status = 'done'
             ORDER BY timestamp DESC LIMIT 1",
            [root_conv_id],
            |row| row.get(0),
        )
        .ok()
        .flatten();
    let Some(last) = last_engine else { return true };
    normalize_engine(&last) == normalize_engine(current_engine)
}

fn normalize_engine(engine: &str) -> &str {
    match engine {
        "claude-code" => "claude",
        e => e,
    }
}

/// Layer B (branchInheritsMainSessionPlan): brand send 가 main 의 sdk-url WS 세션
/// 을 그대로 이어받을 수 있을 때 ContextPack 의 dynamic 섹션을 비운다.
///
/// 조건:
/// - `data.is_branch == true` (brand:* shadow conv 진입)
/// - root main 의 마지막 assistant 메시지 engine 이 현재 send 의 engine 과 같다
///   (= claude_sdk_session 의 SESSIONS / RESUME_IDS 가 root key 로 통합되어 있어
///    같은 SdkSession 을 재사용)
///
/// 효과:
/// - `is_session_continuation = true` 강제 (assemble_prompt 가 recent_context /
///   compressed_memory / cross-session 을 skip)
/// - `parent_messages` / `retrieval_chunks` / `document_chunks` 비움 (brand 만의
///   "main 으로부터 분기" 이라는 사실은 이미 claude session history 에 들어 있음)
///
/// 정적 레이어 (identity / persona / project / agent-role 등) 는 유지 — engine
/// 이 매 send 마다 system_prompt 를 새로 받으므로 retain.
///
/// engine 이 달라지면 (Claude → Codex) 별 session 으로 분리되므로 본 helper 는
/// no-op (= 정상 ContextPack 빌드, INV-3).
pub fn apply_branch_session_inheritance(
    conn: &Connection,
    data: &mut ContextData,
    engine: &str,
) -> bool {
    if !data.is_branch {
        return false;
    }
    let Some(root_conv) = lookup_branch_root_conv_id(conn, &data.conversation_id) else {
        return false;
    };
    if !is_engine_continuity(conn, &root_conv, engine) {
        eprintln!(
            "[branch-session] engine differs (root last engine ≠ {}) for conv={} — keep ContextPack full",
            engine,
            &data.conversation_id[..data.conversation_id.len().min(20)]
        );
        return false;
    }

    eprintln!(
        "[branch-session] brand same-engine continuation conv={} engine={} → \
         drop dynamic ContextPack sections (rely on main's sdk-url session)",
        &data.conversation_id[..data.conversation_id.len().min(20)],
        engine
    );

    data.is_session_continuation = true;
    data.parent_messages.clear();
    data.current_messages.clear();
    data.retrieval_chunks.clear();
    data.document_chunks.clear();
    data.cross_session_data.clear();
    data.compressed_memory = None;
    data.compressed_memory_source = None;
    // thread_inheritance 도 brand 전용 prepend 인데, claude session 이 자체 history
    // 를 갖고 있으므로 중복. 제거.
    data.thread_inheritance = None;
    true
}

/// userIntentSsotSurfacingPlan: agent role 판정. context_loading 안에서 두 번
/// 사용된다 — (1) `agent_role_doc` 로딩, (2) intent_lookup 활성 여부.
/// 단일 SSOT 로 분리해 두 경로가 어긋나지 않도록 한다.
pub(crate) fn resolve_agent_role(conn: &Connection, conversation_id: &str) -> &'static str {
    let is_branch = conversation_id.starts_with("branch:");
    if !is_branch {
        return "architect";
    }
    let branch_id = conversation_id.strip_prefix("branch:").unwrap_or("");
    let is_impl: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM plans WHERE implementation_branch_id = ?1",
            [branch_id],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if is_impl {
        return "developer";
    }
    let is_review: bool = conn
        .query_row(
            "SELECT COUNT(*) > 0 FROM plans WHERE review_branch_id = ?1",
            [branch_id],
            |row| row.get(0),
        )
        .unwrap_or(false);
    if is_review {
        "reviewer"
    } else {
        "architect"
    }
}

/// userIntentSsotSurfacingPlan §Layer 2: 작업 주제로부터 검색 키워드 추출 +
/// synonym expansion (한/영 mix).
///
/// 입력은 현재 prompt + active plan title (있으면). 출력은 dedup·소문자 키워드
/// 리스트로, 길이 ≥ 2 이고 stopword 가 아닌 토큰만 남긴다. 매핑 대상이 없으면
/// 빈 Vec — 호출 측에서 매칭 skip.
pub(crate) fn extract_intent_keywords(prompt: &str, plan_title: Option<&str>) -> Vec<String> {
    use std::collections::HashSet;

    // 사용자 의도 신호로 자주 함께 묶이는 한/영 동의어 페어. 한 쪽이 등장하면
    // 다른 쪽도 함께 검색해 cross-language 매칭을 보강한다. 양방향이라 키 순서
    // 무관.
    const SYNONYMS: &[(&str, &[&str])] = &[
        ("session", &["세션", "resume", "continuation", "ws"]),
        ("세션", &["session", "resume"]),
        ("branch", &["브랜치", "brand"]),
        ("브랜치", &["branch", "brand"]),
        ("context", &["컨텍스트", "contextpack", "context-pack"]),
        ("컨텍스트", &["context", "contextpack"]),
        ("contextpack", &["context", "컨텍스트"]),
        ("context-pack", &["contextpack", "컨텍스트"]),
        ("memory", &["메모리", "기억", "compressed"]),
        ("메모리", &["memory", "기억"]),
        ("intent", &["의도", "purpose"]),
        ("의도", &["intent"]),
        ("ssot", &["sst", "source-of-truth", "단일소스"]),
        ("retrieval", &["검색", "lookup", "조회"]),
        ("검색", &["retrieval", "lookup"]),
        ("plan", &["플랜", "계획", "설계"]),
        ("플랜", &["plan", "계획"]),
        ("계획", &["plan", "설계"]),
        ("review", &["리뷰", "verdict"]),
        ("리뷰", &["review"]),
        ("rt", &["roundtable", "라운드테이블"]),
        ("roundtable", &["rt", "라운드테이블"]),
        ("brand", &["branch", "브랜치"]),
        ("storage", &["저장소", "저장"]),
        ("저장소", &["storage", "저장"]),
    ];

    // 한국어 짧은 stopword (두 글자 조사/접속어) — extract_intent_keywords 전용
    // (FTS5 build_fts_query 의 STOPWORDS 와 별개).
    const KO_STOPWORDS: &[&str] = &[
        "그리고", "하지만", "그래서", "지금", "여기", "이거", "그거", "저거",
        "있는", "있어", "없는", "없어", "라는", "이라는", "처럼", "에서", "에게",
        "이미", "다시", "정말", "조금", "많이", "어떤", "그런", "이런", "저런",
    ];
    const EN_STOPWORDS: &[&str] = &[
        "the", "and", "for", "with", "that", "this", "from", "have", "has", "had",
        "you", "your", "are", "was", "were", "what", "when", "where", "which",
        "should", "could", "would", "can", "may", "must", "will", "into", "about",
        "but", "not", "yes", "no", "ok", "okay", "very", "much", "more", "some",
        "any", "all", "each", "every", "let", "lets", "just", "only", "also",
    ];

    let mut bucket: HashSet<String> = HashSet::new();

    let feed = |s: &str, out: &mut HashSet<String>| {
        let cleaned: String = s
            .chars()
            .map(|c| if c.is_alphanumeric() || c == '_' || c == '가' || (c >= '\u{AC00}' && c <= '\u{D7A3}') { c } else { ' ' })
            .collect();
        for raw in cleaned.split_whitespace() {
            let lower = raw.to_lowercase();
            if lower.chars().count() < 2 {
                continue;
            }
            if EN_STOPWORDS.contains(&lower.as_str()) || KO_STOPWORDS.contains(&lower.as_str()) {
                continue;
            }
            // 너무 긴 토큰 (URL 등) 스킵
            if lower.len() > 40 {
                continue;
            }
            out.insert(lower.clone());
            // synonym expansion
            for (key, syns) in SYNONYMS {
                if *key == lower.as_str() || lower.contains(*key) {
                    for syn in *syns {
                        out.insert((*syn).to_string());
                    }
                }
            }
        }
    };

    feed(prompt, &mut bucket);
    if let Some(title) = plan_title {
        feed(title, &mut bucket);
    }

    let mut out: Vec<String> = bucket.into_iter().collect();
    out.sort();
    // 너무 많은 키워드는 매칭을 noisy 하게 만든다 — 상한 24개.
    if out.len() > 24 {
        out.truncate(24);
    }
    out
}

/// userIntentSsotSurfacingPlan §Layer 2: project 내 모든 conversation 의
/// `role='user'` 메시지에서 키워드 매칭 + recency boost. INV-2/3/4/5.
///
/// 가중치:
///   score = matched_unique_keywords / total_keywords * 0.7 + recency_score * 0.3
/// recency_score = 1 / (1 + age_days / 14) — 2주 반감기.
///
/// 반환은 score DESC, 동률이면 timestamp DESC. top_n 으로 truncate.
pub(crate) fn lookup_user_intent_messages(
    conn: &Connection,
    project_key: &str,
    keywords: &[String],
    top_n: usize,
) -> Vec<UserIntentMatch> {
    if keywords.is_empty() || top_n == 0 {
        return Vec::new();
    }

    // INV-2: role='user' 만 매칭. INV-3: messages 테이블 raw content 그대로 (truncate
    // 없이). INV-4: 같은 project 의 모든 conversation 대상 (cross-conversation).
    //
    // SQLite LIKE 는 case-insensitive ASCII 만이라 lower(content) 에 적용한다.
    // 키워드 → OR 절. 너무 많으면 SQL 길이가 커지므로 상한 24개 (extract 단계에서
    // 이미 trim).
    let now_ms = crate::db::migrations::now_epoch_ms();

    let mut sql = String::from(
        "SELECT m.id, m.conversation_id, m.content, m.timestamp \
         FROM messages m \
         JOIN conversations c ON c.id = m.conversation_id \
         WHERE m.role = 'user' \
           AND c.project_key = ?1 \
           AND (",
    );
    let mut params: Vec<rusqlite::types::Value> = vec![project_key.to_string().into()];
    let mut first = true;
    for (i, _kw) in keywords.iter().enumerate() {
        if !first {
            sql.push_str(" OR ");
        }
        first = false;
        sql.push_str(&format!("lower(m.content) LIKE ?{}", i + 2));
    }
    sql.push_str(") ORDER BY m.timestamp DESC LIMIT 200");

    for kw in keywords {
        let pat = format!("%{}%", kw.to_lowercase());
        params.push(pat.into());
    }

    let Ok(mut stmt) = conn.prepare(&sql) else {
        return Vec::new();
    };
    let rows: Vec<(String, String, String, i64)> = match stmt.query_map(
        rusqlite::params_from_iter(params.iter()),
        |row| Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
        )),
    ) {
        Ok(mapped) => mapped.filter_map(|r| r.ok()).collect(),
        Err(_) => return Vec::new(),
    };

    let total_kw = keywords.len() as f64;

    let mut scored: Vec<UserIntentMatch> = rows
        .into_iter()
        .map(|(_id, conv_id, content, ts)| {
            let lower = content.to_lowercase();
            let mut hit_kws: Vec<String> = Vec::new();
            for kw in keywords {
                if lower.contains(&kw.to_lowercase()) {
                    hit_kws.push(kw.clone());
                }
            }
            let coverage = if total_kw > 0.0 {
                hit_kws.len() as f64 / total_kw
            } else {
                0.0
            };
            let age_days = ((now_ms - ts).max(0) as f64 / 86_400_000.0).max(0.0);
            // INV-5: 같은 키워드 매칭이라도 최근 메시지가 우위. 14d 반감기.
            let recency = 1.0 / (1.0 + age_days / 14.0);
            let score = coverage * 0.7 + recency * 0.3;
            UserIntentMatch {
                timestamp_ms: ts,
                conversation_id: conv_id,
                content,
                score,
                matched_keywords: hit_kws,
            }
        })
        .filter(|m| !m.matched_keywords.is_empty())
        .collect();

    scored.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.timestamp_ms.cmp(&a.timestamp_ms))
    });

    // 같은 raw content 가 (서로 다른 conversation 에 paste 된 경우) 중복으로
    // 잡힐 수 있다. 앞 80자 prefix 로 dedup.
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut deduped: Vec<UserIntentMatch> = Vec::new();
    for m in scored {
        let key: String = m.content.chars().take(80).collect();
        if seen.insert(key) {
            deduped.push(m);
            if deduped.len() >= top_n {
                break;
            }
        }
    }
    deduped
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

    /// projectIdentityAnalysisPlan subtask-03: project 별 최신 `identity_summary`
    /// artifact 의 body (frontmatter strip 후). ContextPack 주입 시 worldview 뒤 /
    /// identity 앞 위치. 없으면 None.
    pub identity_summary_fragment: Option<String>,

    /// userIntentSsotSurfacingPlan: ContextPack 의 [USER_INTENT_LOOKUP] 섹션에
    /// 주입할 과거 사용자 메시지 후보. architect persona 진입 시에만 빌드되며
    /// (다른 role 은 None), 매칭이 0건이어도 architect 면 빈 Vec 으로 채워서
    /// INV-1 의 "항상 섹션 출력" 을 보장한다.
    ///
    /// 각 항목: (timestamp_ms, conversation_id, content_excerpt, score, matched_keywords).
    /// `content_excerpt` 는 prompt_assembly 에서 ~200 char cap 하기 전 raw 본문.
    pub intent_lookup: Option<Vec<UserIntentMatch>>,

    /// multiDeveloperActivePlanIsolationPlan §Layer B: 현재 send 의 sender
    /// 정보를 active plan 섹션 헤더에 inline 한다. 같은 conv 에서 multi-Developer
    /// 가 동시에 일할 때 LLM 이 "이 메시지가 어떤 Developer 한테 가는지" 와
    /// "그 Developer 가 진행 중인 plan" 을 inline hint 로 인지 (instruction
    /// following 약점 보강).
    ///
    /// 출처:
    ///   - `sender_engine`: prepare_engine_run 의 engine_key
    ///   - `sender_persona`: SendWithClaudeInput.persona_label
    ///   - `sender_role`: resolve_agent_role(conn, conversation_id)
    ///   - `sender_model`: SendWithClaudeInput.model
    ///
    /// load_context_data 단계에선 None — prepare_engine_run 이 채워서
    /// assemble_prompt 가 plan section 직전에 prepend 한다.
    pub sender_engine: Option<String>,
    pub sender_persona: Option<String>,
    pub sender_role: Option<String>,
    pub sender_model: Option<String>,
}

/// userIntentSsotSurfacingPlan: 과거 사용자 메시지 매칭 결과.
/// `prompt_assembly` 에서 inline 형태로 직렬화된다.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct UserIntentMatch {
    pub timestamp_ms: i64,
    pub conversation_id: String,
    pub content: String,
    pub score: f64,
    pub matched_keywords: Vec<String>,
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

    // multiDeveloperActivePlanIsolationPlan §Layer A′: brand 진입 시 brand_id 매핑
    // plan 을 우선 lookup. scratchpad / main conv 는 fallback 으로 main conv active.
    // 본 lookup 결과를 has_active_plan / plan_document / intent_lookup 가 공유해
    // 각 단계가 어긋나지 않도록 한다.
    let plan_lookup_target = plan_lookup_conv.as_deref().unwrap_or(conversation_id);
    let isolated_plan: Option<(String, String, Option<String>, String, Option<String>)> =
        super::super::context_pack::lookup_plan_for_conversation(conn, plan_lookup_target);
    let has_active_plan: bool = isolated_plan.is_some();

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
    // multiDeveloperActivePlanIsolationPlan §Layer A′: brand 진입 시 build_plan_section
    // 이 brand_id 매핑 plan 을 우선 lookup 하도록 conv id 를 그대로 전달. scratchpad
    // 는 main conv 사용 (기존 동작 유지). main conv 는 자기 자신.
    let effective_conv_id = if is_scratchpad {
        plan_lookup_conv.as_deref().unwrap_or(conversation_id)
    } else { conversation_id };
    let plan_conv_id = resolve_plan_conversation_id(conn, effective_conv_id);
    let mut plan_section = build_plan_section(conn, effective_conv_id);

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
    // §Layer A′: isolated_plan 이 brand-aware 결과 — plan_id/title/desc/phase/slug.
    // 여기서는 (title, phase, slug) 만 필요.
    let plan_document: Option<String> = if let Some(ref plan_row_full) = isolated_plan {
        if let Some(pp) = project_path {
            let plan_row = Some((
                plan_row_full.1.clone(),  // title
                plan_row_full.3.clone(),  // phase
                plan_row_full.4.clone(),  // slug
            ));
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

    // userIntentSsotSurfacingPlan: 본 conversation 의 agent role 을 미리 한번
    // 결정해 두 곳 (agent_role_doc 로딩 + intent_lookup 활성화) 에서 공유한다.
    // resolve_agent_role 은 plans.implementation_branch_id / review_branch_id 를
    // 조회하므로 비-architect 일 때만 짧게 read query.
    let agent_role = resolve_agent_role(conn, conversation_id);

    // Load agent role document from project docs/agents/
    let agent_role_doc: Option<String> = project_path.and_then(|pp| {
        let agents_dir = std::path::Path::new(pp).join("docs").join("agents");
        if !agents_dir.is_dir() { return None; }
        let role_file = agents_dir.join(format!("{}.md", agent_role));
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

    // projectIdentityAnalysisPlan subtask-03: 최신 identity_summary 를 pre-load 해
    // ContextPack 주입에 사용. frontmatter 는 strip 후 저장.
    let identity_summary_fragment: Option<String> = project_key
        .as_deref()
        .and_then(|pk| {
            crate::commands::artifacts::fetch_latest_identity_summary(conn, pk)
                .ok()
                .flatten()
        })
        .map(|a| crate::agents::identity_analyzer::strip_frontmatter(&a.content).to_string());

    // userIntentSsotSurfacingPlan: architect persona 진입 시 사용자 의도 SSOT
    // (project 내 모든 conversation 의 role='user' 메시지) 에서 현재 작업 주제와
    // 관련된 과거 메시지를 자동 surface. 매칭 0건이어도 architect 면 빈 Vec 으로
    // 채워서 INV-1 (항상 섹션 출력) 을 보장한다. developer/reviewer 는 None →
    // prompt_assembly 에서 섹션 생략.
    let intent_lookup: Option<Vec<UserIntentMatch>> = if agent_role == "architect" {
        if let Some(pk) = project_key.as_deref() {
            // §Layer A′: Active plan title 을 isolated_plan 결과에서 가져온다.
            // brand-aware lookup 이므로 다른 Developer 의 plan 키워드가 섞이지 않는다.
            let active_plan_title: Option<String> =
                isolated_plan.as_ref().map(|(_id, title, _, _, _)| title.clone());
            let keywords = extract_intent_keywords(prompt, active_plan_title.as_deref());
            if keywords.is_empty() {
                Some(Vec::new())
            } else {
                Some(lookup_user_intent_messages(conn, pk, &keywords, 5))
            }
        } else {
            // project_key 가 없으면 cross-conv 검색 자체가 불가 — 빈 섹션
            Some(Vec::new())
        }
    } else {
        None
    };

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
        identity_summary_fragment,
        intent_lookup,
        // §Layer B: persistence.rs::prepare_engine_run 에서 채워준다.
        sender_engine: None,
        sender_persona: None,
        sender_role: Some(agent_role.to_string()),
        sender_model: None,
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

// ─── Layer B: branch session inheritance tests ────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn build_db_with_branch(root_conv_id: &str, branch_id: &str) -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE branches (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL
             );
             CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'done',
                engine TEXT
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO branches (id, conversation_id) VALUES (?1, ?2)",
            rusqlite::params![branch_id, root_conv_id],
        )
        .unwrap();
        conn
    }

    fn insert_assistant_message(conn: &Connection, conv: &str, engine: &str, ts: i64) {
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status, engine)
             VALUES (?1, ?2, 'assistant', 'reply', ?3, 'done', ?4)",
            rusqlite::params![format!("m-{}-{}", conv, ts), conv, ts, engine],
        )
        .unwrap();
    }

    fn empty_data(conv_id: &str, is_branch: bool) -> ContextData {
        ContextData {
            conversation_id: conv_id.into(),
            project_path: None,
            prompt: "hi".into(),
            is_branch,
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
            thread_inheritance: None,
            agent_role_doc: None,
            previous_impl_status: None,
            active_skills: vec![],
            cross_session_ids: vec![],
            persona_fragment: None,
            context_mode_override: None,
            context_budget_cap: None,
            user_profile: None,
            conventions_synced: false,
            is_session_continuation: false,
            identity_summary_fragment: None,
            intent_lookup: None,
            sender_engine: None,
            sender_persona: None,
            sender_role: None,
            sender_model: None,
        }
    }

    #[test]
    fn lookup_branch_root_returns_main_conv_id() {
        let conn = build_db_with_branch("conv-main", "b1");
        let got = lookup_branch_root_conv_id(&conn, "branch:b1");
        assert_eq!(got.as_deref(), Some("conv-main"));
    }

    #[test]
    fn lookup_branch_root_returns_none_for_non_branch_conv() {
        let conn = build_db_with_branch("conv-main", "b1");
        assert!(lookup_branch_root_conv_id(&conn, "conv-main").is_none());
        assert!(lookup_branch_root_conv_id(&conn, "branch:unknown").is_none());
    }

    #[test]
    fn engine_continuity_treats_claude_and_claude_code_as_same() {
        let conn = build_db_with_branch("conv-main", "b1");
        insert_assistant_message(&conn, "conv-main", "claude-code", 1000);
        assert!(is_engine_continuity(&conn, "conv-main", "claude"));
        assert!(is_engine_continuity(&conn, "conv-main", "claude-code"));
    }

    #[test]
    fn engine_continuity_returns_false_for_different_engine() {
        let conn = build_db_with_branch("conv-main", "b1");
        insert_assistant_message(&conn, "conv-main", "claude-code", 1000);
        assert!(!is_engine_continuity(&conn, "conv-main", "codex"));
        assert!(!is_engine_continuity(&conn, "conv-main", "gemini"));
    }

    #[test]
    fn engine_continuity_returns_true_when_no_prior_messages() {
        // root 에 메시지가 없으면 첫 send 라 continuation 로 본다 (LAST_DELIVERED 가
        // 비어 있어 어차피 full ContextPack 으로 흐르지만, dynamic 섹션은 비울 수 있음)
        let conn = build_db_with_branch("conv-main", "b1");
        assert!(is_engine_continuity(&conn, "conv-main", "claude"));
    }

    #[test]
    fn apply_branch_session_inheritance_same_engine_clears_dynamic_sections() {
        // INV-2: brand:* same engine 진입 시 dynamic 섹션이 비워지고
        //        is_session_continuation=true 가 셋팅된다.
        let conn = build_db_with_branch("conv-main", "b1");
        insert_assistant_message(&conn, "conv-main", "claude-code", 1000);

        let mut data = empty_data("branch:b1", true);
        data.parent_messages = vec![("user".into(), "hi".into(), None, None)];
        data.current_messages = vec![("assistant".into(), "ok".into(), Some("claude".into()), None)];
        data.compressed_memory = Some("memo".into());
        data.thread_inheritance = Some("inherit".into());

        let applied = apply_branch_session_inheritance(&conn, &mut data, "claude");
        assert!(applied, "same-engine brand 면 inheritance 가 적용되어야 함");
        assert!(data.is_session_continuation);
        assert!(data.parent_messages.is_empty());
        assert!(data.current_messages.is_empty());
        assert!(data.compressed_memory.is_none());
        assert!(data.thread_inheritance.is_none());
        assert!(data.retrieval_chunks.is_empty());
    }

    #[test]
    fn apply_branch_session_inheritance_different_engine_keeps_full_pack() {
        // INV-3: engine 변경 시는 별 session 이라 ContextPack 정상 빌드 유지.
        let conn = build_db_with_branch("conv-main", "b1");
        insert_assistant_message(&conn, "conv-main", "claude-code", 1000);

        let mut data = empty_data("branch:b1", true);
        data.parent_messages = vec![("user".into(), "hi".into(), None, None)];
        data.compressed_memory = Some("memo".into());

        let applied = apply_branch_session_inheritance(&conn, &mut data, "codex");
        assert!(!applied, "다른 engine 으로 brand 진입 시 inheritance 미적용");
        assert!(!data.is_session_continuation);
        assert_eq!(data.parent_messages.len(), 1, "parent 메시지가 보존되어야");
        assert_eq!(data.compressed_memory.as_deref(), Some("memo"));
    }

    #[test]
    fn apply_branch_session_inheritance_skips_non_branch_conv() {
        // brand 가 아니면 무조건 false (정상 ContextPack).
        let conn = build_db_with_branch("conv-main", "b1");
        insert_assistant_message(&conn, "conv-main", "claude-code", 1000);

        let mut data = empty_data("conv-main", false);
        data.parent_messages = vec![("user".into(), "hi".into(), None, None)];

        let applied = apply_branch_session_inheritance(&conn, &mut data, "claude");
        assert!(!applied, "non-branch conv 는 inheritance 미적용");
        assert!(!data.is_session_continuation);
        assert_eq!(data.parent_messages.len(), 1);
    }

    // ─── userIntentSsotSurfacingPlan tests ──────────────────────────────────

    fn build_intent_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                project_key TEXT NOT NULL,
                type TEXT
             );
             CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                timestamp INTEGER NOT NULL,
                status TEXT NOT NULL DEFAULT 'done',
                engine TEXT
             );",
        )
        .unwrap();
        conn
    }

    fn add_conv(conn: &Connection, conv_id: &str, project_key: &str) {
        conn.execute(
            "INSERT INTO conversations (id, project_key, type) VALUES (?1, ?2, 'main')",
            rusqlite::params![conv_id, project_key],
        )
        .unwrap();
    }

    fn add_user_msg(conn: &Connection, conv_id: &str, content: &str, ts: i64) {
        let id = format!("u-{}-{}", conv_id, ts);
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
             VALUES (?1, ?2, 'user', ?3, ?4, 'done')",
            rusqlite::params![id, conv_id, content, ts],
        )
        .unwrap();
    }

    fn add_assistant_msg(conn: &Connection, conv_id: &str, content: &str, ts: i64) {
        let id = format!("a-{}-{}", conv_id, ts);
        conn.execute(
            "INSERT INTO messages (id, conversation_id, role, content, timestamp, status)
             VALUES (?1, ?2, 'assistant', ?3, ?4, 'done')",
            rusqlite::params![id, conv_id, content, ts],
        )
        .unwrap();
    }

    #[test]
    fn extract_keywords_drops_stopwords_and_short_tokens() {
        let kws = extract_intent_keywords("you are the architect for branch session", None);
        // 'you', 'are', 'the', 'for' 는 stopword → 제외
        assert!(!kws.contains(&"you".into()));
        assert!(!kws.contains(&"the".into()));
        assert!(kws.contains(&"branch".into()));
        assert!(kws.contains(&"session".into()));
        // synonym expansion 으로 한국어 동의어 추가
        assert!(kws.contains(&"세션".into()) || kws.contains(&"resume".into()),
            "session 의 한국어 동의어 또는 resume 가 expanded: {:?}", kws);
    }

    #[test]
    fn extract_keywords_handles_korean_input() {
        let kws = extract_intent_keywords("브랜치 세션을 메인에서 이어받자", None);
        assert!(kws.contains(&"브랜치".into()));
        assert!(kws.contains(&"세션을".into()) || kws.contains(&"세션".into()),
            "한국어 토큰 추출: {:?}", kws);
    }

    #[test]
    fn extract_keywords_with_plan_title_combines_sources() {
        let kws = extract_intent_keywords("rev.1 설계", Some("Branch session inheritance"));
        assert!(kws.contains(&"branch".into()));
        assert!(kws.contains(&"session".into()));
        assert!(kws.contains(&"inheritance".into()));
    }

    #[test]
    fn lookup_user_intent_filters_by_role_user_only() {
        // INV-2: role='user' 만 매칭 — assistant 메시지는 무시.
        let conn = build_intent_db();
        add_conv(&conn, "c1", "proj-A");
        add_user_msg(&conn, "c1", "branch session 을 ws 모드로 입장한다", 1_700_000_000_000);
        add_assistant_msg(&conn, "c1", "branch session ws 작업 결과", 1_700_000_001_000);

        let kws = vec!["branch".to_string(), "session".to_string(), "ws".to_string()];
        let hits = lookup_user_intent_messages(&conn, "proj-A", &kws, 5);
        assert_eq!(hits.len(), 1, "user 메시지 1건만 잡혀야: {:?}", hits);
        assert!(hits[0].content.contains("입장한다"));
    }

    #[test]
    fn lookup_user_intent_searches_across_conversations() {
        // INV-4: 같은 project 의 다른 conversation 의 user 메시지도 매칭.
        let conn = build_intent_db();
        add_conv(&conn, "c1", "proj-A");
        add_conv(&conn, "c2", "proj-A");
        add_conv(&conn, "c3", "proj-B"); // 다른 project — 매칭 제외
        add_user_msg(&conn, "c1", "branch session 작업 1", 1_700_000_000_000);
        add_user_msg(&conn, "c2", "branch session 작업 2", 1_700_000_001_000);
        add_user_msg(&conn, "c3", "branch session 다른 프로젝트", 1_700_000_002_000);

        let kws = vec!["branch".to_string()];
        let hits = lookup_user_intent_messages(&conn, "proj-A", &kws, 10);
        let conv_ids: std::collections::HashSet<&str> = hits.iter().map(|m| m.conversation_id.as_str()).collect();
        assert!(conv_ids.contains("c1") && conv_ids.contains("c2"),
            "proj-A 의 두 conv 모두 매칭: {:?}", conv_ids);
        assert!(!conv_ids.contains("c3"), "다른 project 의 메시지는 제외");
    }

    #[test]
    fn lookup_user_intent_recency_boost_orders_recent_first() {
        // INV-5: 같은 키워드 매칭 시 최근 메시지가 우위.
        let conn = build_intent_db();
        add_conv(&conn, "c1", "proj-A");
        let old_ts: i64 = 1_500_000_000_000; // 2017
        let recent_ts = crate::db::migrations::now_epoch_ms() - 3600_000; // 1시간 전
        add_user_msg(&conn, "c1", "branch session old", old_ts);
        add_user_msg(&conn, "c1", "branch session recent", recent_ts);

        let kws = vec!["branch".to_string(), "session".to_string()];
        let hits = lookup_user_intent_messages(&conn, "proj-A", &kws, 5);
        assert_eq!(hits.len(), 2);
        assert!(hits[0].content.contains("recent"),
            "최근 메시지가 첫번째: {:?}", hits[0].content);
        assert!(hits[0].score > hits[1].score, "최근 score 우위");
    }

    #[test]
    fn lookup_user_intent_returns_empty_for_no_keywords() {
        let conn = build_intent_db();
        add_conv(&conn, "c1", "proj-A");
        add_user_msg(&conn, "c1", "branch session", 1_700_000_000_000);
        let hits = lookup_user_intent_messages(&conn, "proj-A", &[], 5);
        assert!(hits.is_empty());
    }

    #[test]
    fn lookup_user_intent_dedupes_by_content_prefix() {
        // 같은 prefix(80자) 의 메시지는 한 번만 surface.
        let conn = build_intent_db();
        add_conv(&conn, "c1", "proj-A");
        add_conv(&conn, "c2", "proj-A");
        let prefix = "동일한 사용자 의도 페이스트 — branch session ws 작업 정리, 메인 세션 통합 필요";
        add_user_msg(&conn, "c1", prefix, 1_700_000_000_000);
        add_user_msg(&conn, "c2", prefix, 1_700_000_001_000);
        let kws = vec!["branch".to_string()];
        let hits = lookup_user_intent_messages(&conn, "proj-A", &kws, 5);
        assert_eq!(hits.len(), 1, "동일 prefix 는 dedup: {:?}", hits);
    }

    #[test]
    fn lookup_user_intent_respects_top_n() {
        let conn = build_intent_db();
        add_conv(&conn, "c1", "proj-A");
        for i in 0..10 {
            add_user_msg(&conn, "c1", &format!("branch session iteration {}", i), 1_700_000_000_000 + i);
        }
        let kws = vec!["branch".to_string()];
        let hits = lookup_user_intent_messages(&conn, "proj-A", &kws, 3);
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn resolve_agent_role_returns_architect_for_main_chat() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE plans (
                implementation_branch_id TEXT,
                review_branch_id TEXT
             );",
        ).unwrap();
        assert_eq!(resolve_agent_role(&conn, "conv-main"), "architect");
    }

    #[test]
    fn resolve_agent_role_returns_developer_for_impl_branch() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE plans (
                implementation_branch_id TEXT,
                review_branch_id TEXT
             );",
        ).unwrap();
        conn.execute(
            "INSERT INTO plans (implementation_branch_id) VALUES ('b-impl')",
            [],
        ).unwrap();
        assert_eq!(resolve_agent_role(&conn, "branch:b-impl"), "developer");
    }

    #[test]
    fn resolve_agent_role_returns_reviewer_for_review_branch() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE plans (
                implementation_branch_id TEXT,
                review_branch_id TEXT
             );",
        ).unwrap();
        conn.execute(
            "INSERT INTO plans (review_branch_id) VALUES ('b-rev')",
            [],
        ).unwrap();
        assert_eq!(resolve_agent_role(&conn, "branch:b-rev"), "reviewer");
    }
}
