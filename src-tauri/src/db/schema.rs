/// Migration v0: schema_version table (always applied first)
pub const CREATE_SCHEMA_VERSION: &str = "
CREATE TABLE IF NOT EXISTS schema_version (
    version    INTEGER PRIMARY KEY,
    applied_at INTEGER NOT NULL
);
";

/// Migration v11: ContextPack traceability columns on trace_log.
/// Stores metadata (not full prompt body) for each agent execution.
#[allow(dead_code)]
pub const V11_SCHEMA: &str = "
ALTER TABLE trace_log ADD COLUMN context_mode     TEXT;
ALTER TABLE trace_log ADD COLUMN context_sections TEXT;
ALTER TABLE trace_log ADD COLUMN context_length   INTEGER;
ALTER TABLE trace_log ADD COLUMN context_hash     TEXT;
ALTER TABLE trace_log ADD COLUMN context_truncated INTEGER DEFAULT 0;
";

/// Migration v10: agent_jobs table for durable job tracking.
pub const V10_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS agent_jobs (
    id               TEXT    PRIMARY KEY,
    conversation_id  TEXT    NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    message_id       TEXT    REFERENCES messages(id) ON DELETE SET NULL,
    engine           TEXT    NOT NULL,
    kind             TEXT    NOT NULL DEFAULT 'agent',
    status           TEXT    NOT NULL DEFAULT 'running',
    error            TEXT,
    started_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_agent_jobs_conversation_id ON agent_jobs(conversation_id);
CREATE INDEX IF NOT EXISTS idx_agent_jobs_status ON agent_jobs(status);
";

/// Migration v9: add subtask_id to branches for developer lane linkage.
#[allow(dead_code)]
pub const V9_SCHEMA: &str = "
ALTER TABLE branches ADD COLUMN subtask_id TEXT REFERENCES plan_subtasks(id) ON DELETE SET NULL;
";

/// Migration v8: add mode column to branches (chat/roundtable).
#[allow(dead_code)]
pub const V8_SCHEMA: &str = "
ALTER TABLE branches ADD COLUMN mode TEXT DEFAULT 'chat';
";

/// Migration v7: add agent ownership columns to plan_subtasks.
#[allow(dead_code)]
pub const V7_SCHEMA: &str = "
ALTER TABLE plan_subtasks ADD COLUMN owner_agent     TEXT;
ALTER TABLE plan_subtasks ADD COLUMN last_updated_by TEXT;
";

/// Migration v6: extend trace_log with OTel-ready span columns.
/// Existing rows get NULL for new columns — INSERT paths updated separately.
/// NOTE: apply_v6() now uses idempotent add_column_if_missing; this const is kept for reference.
#[allow(dead_code)]
pub const V6_SCHEMA: &str = "
ALTER TABLE trace_log ADD COLUMN trace_id       TEXT;
ALTER TABLE trace_log ADD COLUMN span_id        TEXT;
ALTER TABLE trace_log ADD COLUMN parent_span_id TEXT;
ALTER TABLE trace_log ADD COLUMN operation      TEXT;
ALTER TABLE trace_log ADD COLUMN engine         TEXT;
ALTER TABLE trace_log ADD COLUMN duration_ms    INTEGER;
ALTER TABLE trace_log ADD COLUMN status         TEXT DEFAULT 'ok';
CREATE INDEX IF NOT EXISTS idx_trace_log_trace_id ON trace_log(trace_id);
";

/// Migration v5: evaluation harness tables
pub const V5_SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS eval_runs (
    id               TEXT    PRIMARY KEY,
    conversation_id  TEXT    NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    title            TEXT    NOT NULL,
    prompt           TEXT    NOT NULL,
    mode             TEXT,
    participants     TEXT,
    rounds           INTEGER NOT NULL DEFAULT 1,
    status           TEXT    NOT NULL DEFAULT 'pending',
    created_at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_eval_runs_conversation_id ON eval_runs(conversation_id);

CREATE TABLE IF NOT EXISTS eval_results (
    id           TEXT    PRIMARY KEY,
    eval_run_id  TEXT    NOT NULL REFERENCES eval_runs(id) ON DELETE CASCADE,
    agent_name   TEXT    NOT NULL,
    engine       TEXT    NOT NULL,
    round        INTEGER NOT NULL,
    content      TEXT    NOT NULL,
    input_tokens  INTEGER NOT NULL DEFAULT 0,
    output_tokens INTEGER NOT NULL DEFAULT 0,
    cost_usd     REAL    NOT NULL DEFAULT 0.0,
    duration_ms  INTEGER NOT NULL DEFAULT 0,
    created_at   INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_eval_results_run_id ON eval_results(eval_run_id);
";

/// Migration v4: add subtask_id column to artifacts for plan-artifact linking
/// NOTE: apply_v4() now uses idempotent add_column_if_missing; this const is kept for reference.
#[allow(dead_code)]
pub const V4_SCHEMA: &str = "
ALTER TABLE artifacts ADD COLUMN subtask_id TEXT REFERENCES plan_subtasks(id) ON DELETE SET NULL;
CREATE INDEX IF NOT EXISTS idx_artifacts_subtask_id ON artifacts(subtask_id);
";

/// Migration v2: add ResumeToken columns to conversations
/// Stores per-conversation, per-engine session token for --resume support.
/// No new table — stored inline in conversations (DATA_MODEL §1.8).
/// NOTE: apply_v2() now uses idempotent add_column_if_missing; this const is kept for reference.
#[allow(dead_code)]
pub const V2_SCHEMA: &str = "
ALTER TABLE conversations ADD COLUMN resume_token        TEXT;
ALTER TABLE conversations ADD COLUMN resume_token_engine TEXT;
";

/// Migration v3: plan state tables
/// Adds `plans` and `plan_subtasks` for per-conversation/branch planning.
pub const V3_SCHEMA: &str = "
-- plans (DATA_MODEL §plan)
CREATE TABLE IF NOT EXISTS plans (
    id               TEXT    PRIMARY KEY,
    conversation_id  TEXT    NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    branch_id        TEXT    REFERENCES branches(id),
    title            TEXT    NOT NULL,
    description      TEXT,
    expected_outcome TEXT,
    status           TEXT    NOT NULL DEFAULT 'draft',
    created_at       INTEGER NOT NULL,
    updated_at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_plans_conversation_id ON plans(conversation_id);
CREATE INDEX IF NOT EXISTS idx_plans_branch_id       ON plans(branch_id);

-- plan_subtasks (DATA_MODEL §plan)
CREATE TABLE IF NOT EXISTS plan_subtasks (
    id         TEXT    PRIMARY KEY,
    plan_id    TEXT    NOT NULL REFERENCES plans(id) ON DELETE CASCADE,
    idx        INTEGER NOT NULL,
    title      TEXT    NOT NULL,
    details    TEXT,
    status     TEXT    NOT NULL DEFAULT 'todo',
    outcome    TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_plan_subtasks_plan_id ON plan_subtasks(plan_id);
";

/// Migration v1: core tables
pub const V1_SCHEMA: &str = "
-- projects (DATA_MODEL §1.2)
CREATE TABLE IF NOT EXISTS projects (
    key            TEXT    PRIMARY KEY,
    name           TEXT    NOT NULL,
    path           TEXT,
    type           TEXT    NOT NULL DEFAULT 'project',
    default_engine TEXT,
    workspace_root TEXT,
    source         TEXT    NOT NULL DEFAULT 'configured',
    updated_at     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_projects_updated_at
    ON projects(updated_at DESC);

-- conversations (DATA_MODEL §1.3)
CREATE TABLE IF NOT EXISTS conversations (
    id                   TEXT    PRIMARY KEY,
    project_key          TEXT    NOT NULL REFERENCES projects(key) ON DELETE CASCADE,
    label                TEXT    NOT NULL,
    custom_label         TEXT,
    type                 TEXT    NOT NULL DEFAULT 'main',
    mode                 TEXT    NOT NULL DEFAULT 'chat',
    parent_id            TEXT    REFERENCES conversations(id),
    source               TEXT    NOT NULL DEFAULT 'tunadish',
    created_at           INTEGER NOT NULL,
    updated_at           INTEGER NOT NULL,
    -- ConvSettings (inline)
    engine               TEXT,
    model                TEXT,
    persona              TEXT,
    trigger_mode         TEXT,
    -- Usage tracking
    total_input_tokens   INTEGER NOT NULL DEFAULT 0,
    total_output_tokens  INTEGER NOT NULL DEFAULT 0,
    total_cost_usd       REAL    NOT NULL DEFAULT 0.0
);
CREATE INDEX IF NOT EXISTS idx_conversations_project_key
    ON conversations(project_key);
CREATE INDEX IF NOT EXISTS idx_conversations_updated_at
    ON conversations(updated_at DESC);

-- messages (DATA_MODEL §1.5)
CREATE TABLE IF NOT EXISTS messages (
    id                TEXT    PRIMARY KEY,
    conversation_id   TEXT    NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    role              TEXT    NOT NULL,
    content           TEXT    NOT NULL,
    timestamp         INTEGER NOT NULL,
    status            TEXT    NOT NULL DEFAULT 'done',
    progress_content  TEXT,
    engine            TEXT,
    model             TEXT,
    persona           TEXT,
    -- v45: Lindera 등 형태소 분석기가 채우는 FTS5 인덱싱 소스. NULL 이면 `messages_fts`
    -- trigger 가 `COALESCE(content_tokenized, content)` 로 원문을 fallback 인덱싱.
    content_tokenized TEXT
);
CREATE INDEX IF NOT EXISTS idx_messages_conv_timestamp
    ON messages(conversation_id, timestamp);

-- branches (DATA_MODEL §1.4)
CREATE TABLE IF NOT EXISTS branches (
    id               TEXT    PRIMARY KEY,
    conversation_id  TEXT    NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    label            TEXT    NOT NULL,
    custom_label     TEXT,
    status           TEXT    NOT NULL DEFAULT 'active',
    checkpoint_id    TEXT    REFERENCES messages(id),
    parent_branch_id TEXT    REFERENCES branches(id),
    session_id       TEXT,
    git_branch       TEXT,
    created_at       INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_branches_conversation_id
    ON branches(conversation_id);
CREATE INDEX IF NOT EXISTS idx_branches_session_id
    ON branches(session_id);

-- memos (DATA_MODEL §1.10) — schema only, CRUD in later milestone
CREATE TABLE IF NOT EXISTS memos (
    id              TEXT    PRIMARY KEY,
    message_id      TEXT    NOT NULL,
    conversation_id TEXT    NOT NULL,
    project_key     TEXT    NOT NULL,
    content         TEXT    NOT NULL,
    type            TEXT    NOT NULL DEFAULT 'context',
    tags            TEXT    NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_memos_project_key
    ON memos(project_key);
CREATE INDEX IF NOT EXISTS idx_memos_message_id
    ON memos(message_id);

-- artifacts (DATA_MODEL §1.9) — schema only, CRUD in later milestone
CREATE TABLE IF NOT EXISTS artifacts (
    id              TEXT    PRIMARY KEY,
    conversation_id TEXT,
    branch_id       TEXT,
    type            TEXT    NOT NULL,
    title           TEXT    NOT NULL,
    content         TEXT    NOT NULL,
    status          TEXT    NOT NULL DEFAULT 'draft',
    created_at      INTEGER NOT NULL,
    updated_at      INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_artifacts_conversation_id
    ON artifacts(conversation_id);

-- trace_log — schema only, used for token/cost tracking in later milestone
CREATE TABLE IF NOT EXISTS trace_log (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id TEXT    NOT NULL,
    input_tokens    INTEGER NOT NULL DEFAULT 0,
    output_tokens   INTEGER NOT NULL DEFAULT 0,
    cost_usd        REAL    NOT NULL DEFAULT 0.0,
    recorded_at     INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_trace_log_conversation_id
    ON trace_log(conversation_id);

-- messages_fts (FTS5) — v45: standalone FTS5, external content 제거.
-- `content=messages` 대신 `message_id UNINDEXED` 를 인덱스 컬럼으로 넣어 메시지 재구축 없이도
-- rowid 기반 JOIN + message_id 검색 모두 가능. tokenize='unicode61' 는 Lindera 결과 와 원문
-- fallback 을 공통으로 처리하기 위한 trivial tokenizer (Rust 측에서 이미 형태소 분리 완료).
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
    content,
    message_id UNINDEXED,
    tokenize='unicode61'
);
";
