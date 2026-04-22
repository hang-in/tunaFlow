# Subtask 02 — Backend: `branch.mode='design_review'` + Reviewer role + 라운드 상한

> 상위 plan: [designReviewGatePlan.md](./designReviewGatePlan.md)

## Changed files

- `src-tauri/src/commands/branches.rs` — `branch_mode` 분기에 `"design_review"` 추가 + 신규 Tauri command `open_design_review_branch`.
- `src-tauri/src/commands/roundtable_helpers/types.rs` — `role_guidance()` 에 `mode` 파라미터 추가 + design_review reviewer guidance 신설.
- `src-tauri/src/commands/roundtable_helpers/deliberative.rs` (또는 RT 실행 루프) — design_review 모드 전용 라운드 카운터 + escalate 분기.
- `src-tauri/src/commands/roundtable_helpers/prompt.rs` — plan 본문 + subtask 파일 payload 주입 + focus areas.
- `src-tauri/src/commands/design_review.rs` (신규) — `advance_design_review_round`, `force_approve_design_review`, `cancel_design_review` Tauri commands.
- `src-tauri/src/lib.rs` — 신규 command 등록.

## Change description

### 1. `branch.mode='design_review'` 수용

`src-tauri/src/commands/branches.rs` 의 기존:
```rust
let branch_mode = input.mode.as_deref().unwrap_or("chat");
let is_rt = branch_mode == "roundtable";
```
→
```rust
let branch_mode = input.mode.as_deref().unwrap_or("chat");
let is_rt = matches!(branch_mode, "roundtable" | "design_review");
let is_design_review = branch_mode == "design_review";
```

이후 `is_design_review` 분기에서:
- 시스템 메시지 자동 삽입: `"Design review started · plan=<slug> · reviewer=<engine> · round=1"`
- RT payload 에 plan 문서 본문 + subtask 파일들 + frontmatter `related` 파일 경로 자동 주입

### 2. Reviewer role guidance 확장

`src-tauri/src/commands/roundtable_helpers/types.rs`:

```rust
pub fn role_guidance(role: &str, mode: Option<&str>) -> &'static str {
    match (role, mode) {
        ("reviewer" | "critic", Some("design_review")) => DESIGN_REVIEW_REVIEWER_GUIDANCE,
        ("reviewer" | "critic", _) => REVIEWER_GUIDANCE,
        ("proposer", Some("design_review")) => DESIGN_REVIEW_PROPOSER_GUIDANCE,
        ("proposer", _) => PROPOSER_GUIDANCE,
        // ... 기존 verifier/synthesizer ...
        _ => "",
    }
}

const DESIGN_REVIEW_REVIEWER_GUIDANCE: &str = r#"
You are a blind verifier of a design plan produced by an Architect agent.

Produce the following sections in order (markdown):

## Invariant checks
JSON array of `{ "id": "INV-N", "status": "pass|fail|cannot_verify", "evidence": "<file:line or reasoning>" }`.
Verdict MUST be `fail` if any status == "fail".

## Scores (1-5)
- plan_coverage: N/5 — one-line reason
- code_quality: N/5 — one-line reason
- test_coverage: N/5 — one-line reason
- convention: N/5 — one-line reason

## Findings
- [BLOCKER] file:line — concrete defect
- [MAJOR] ...
- [MINOR] ...
No subjective "clean/nice/better" language.

## Recommendations
Per finding, minimal actionable fix.

## failed_subtask_ids
JSON array of subtask numbers that should be blocked.

## Verdict
`pass` | `fail` | `escalate_to_human` — one-line reason.

## regression_check
{"prev_findings_resolved": [...], "newly_broken": [...]}
"#;

const DESIGN_REVIEW_PROPOSER_GUIDANCE: &str = r#"
You are the Architect presenting a plan for blind review. The reviewer has full
codebase access via tool-request markers. Your job is NOT to implement — your
job is to respond to reviewer findings by updating the plan document.

- Acknowledge each BLOCKER / MAJOR with either "resolved via X" or "out of scope — <reason>"
- Do not inflate scope to include MINOR unless it blocks later subtasks
- Keep the 4-section structure (TL;DR / Specification / Invariants / Rationale)
- Record review history at the bottom of the plan as `## Codex Review (Round N — YYYY-MM-DD)`
"#;
```

### 3. 라운드 카운터 + escalate

`src-tauri/src/commands/design_review.rs`:

```rust
const MAX_ROUNDS: u8 = 3;

#[derive(serde::Serialize, Clone)]
pub struct ReviewRoundResult {
    pub round: u8,
    pub verdict: String,  // "pass" | "fail" | "escalate_to_human"
    pub blocker_count: u32,
    pub major_count: u32,
    pub minor_count: u32,
    pub failed_subtask_ids: Vec<u32>,
    pub transcript_markdown: String,  // auto-append 용
}

#[tauri::command]
pub async fn advance_design_review_round(
    branch_id: String,
    app: AppHandle,
    state: State<'_, DbState>,
) -> Result<ReviewRoundResult, AppError> {
    let current_round: u8 = { /* SELECT design_review_round FROM branches WHERE id=?1 */ };
    if current_round >= MAX_ROUNDS {
        return Err(AppError::Agent(format!(
            "design_review: max rounds ({}) reached — use force_approve or cancel",
            MAX_ROUNDS
        )));
    }
    // 1) RT deliberative 실행 (reviewer 호출)
    // 2) reviewer 응답 파싱 (JSON + markdown sections)
    // 3) divergence detector — 이전 라운드와 같은 finding category 2회 연속 시 verdict 강제 escalate
    // 4) round += 1, UPDATE branches SET design_review_round = ?
    // 5) emit "design_review_round_complete" event
    // 6) 반환
}

#[tauri::command]
pub fn force_approve_design_review(
    branch_id: String,
    plan_document_path: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    // plan frontmatter 에 force_approved_at_round + blocker_findings 기록
    // branch archived=1
    // main conversation 에 "plan force-approved (N blockers pending)" 시스템 메시지
}

#[tauri::command]
pub fn cancel_design_review(branch_id: String, state: State<DbState>) -> Result<(), AppError> {
    // INV-5: plan 문서 건드리지 않음. branch archived=1.
    // 중간 reviewer 응답은 branch shadow conversation 메시지로만 남음.
}
```

### 4. `open_design_review_branch` Tauri command

Subtask 01 의 "RT 검토 먼저" 버튼이 호출:

```rust
#[tauri::command]
pub async fn open_design_review_branch(
    plan_id: String,
    plan_document_path: String,
    main_conversation_id: String,
    reviewer_engine: Option<String>,  // default = "codex"
    state: State<'_, DbState>,
    app: AppHandle,
) -> Result<String, AppError> {  // returns branch_id
    // 1) 기존 create_branch 호출 with mode="design_review"
    // 2) branch frontmatter / meta 에 plan_id, plan_document_path, reviewer_engine 기록
    // 3) 첫 라운드 자동 시작 (advance_design_review_round 호출)
    // 4) RT 드로어 open 이벤트 emit
}
```

### 5. Divergence detector 재활용

`roundtable_helpers/deliberative.rs` (또는 관련) 에 이미 Phase 2 구현된 divergence detector 를 `mode="design_review"` 에서도 활용:

```rust
if mode == Some("design_review") {
    let same_category_count = count_consecutive_findings_category(&reviewer_outputs);
    if same_category_count >= 2 && round >= 2 {
        verdict = "escalate_to_human";
    }
}
```

## Dependencies

depends_on: [01] — UI 버튼이 없으면 이 command 들을 호출할 경로가 없음.

## Verification

- `cargo test --lib commands::design_review`:
  - `advance_design_review_round` — round < 3 이면 성공, round == 3 에서 호출 시 에러
  - `cancel_design_review` — plan 문서 md5 변경 없음, branch archived=1
  - `force_approve_design_review` — plan frontmatter 에 `force_approved_at_round` 기록 확인
- `cargo test --lib commands::roundtable_helpers::types` — design_review reviewer guidance 가 `invariant_checks`, `Scores`, `Findings`, `Verdict`, `regression_check` 6 키워드를 모두 포함하는지 contains assert.
- Integration: mock Codex 가 fail 3회 연속 → 4번째 호출 에러 확인.
- Divergence: mock 이 라운드 1~2 에서 같은 `"defect_type": "deadlock"` 반환 → round 3 기다리지 않고 escalate_to_human 반환.
- `cargo check` — exit 0.

## Risks

- **Codex CLI 호출 경로 유무**: 현재 tunaFlow 의 Codex 호출이 `agents/codex.rs` (CLI) + `agents/codex_app_server.rs` (app-server) 로 이원화. 어느 경로로 design review reviewer 를 호출할지 Developer 결정 — app-server 가 stateful 해서 thread_key 유지 가능하고 latency 낮음 (권장).
- **Reviewer 응답 파싱**: reviewer 가 guidance 를 따르지 않고 자유 markdown 을 반환하면 `verdict` 추출 실패. 방어책: parsing fallback (verdict 섹션 못 찾으면 "escalate_to_human" 으로 안전 처리). Guidance 에 "반드시 위 섹션 순서" 를 강조.
- **Lock contention**: `advance_design_review_round` 가 DB write lock 을 오래 잡으면 streaming 충돌. RT 실행 부분은 기존 deliberative 모드 경로를 통과하므로 이미 적절히 분리됨. 추가 주의 없음.
- **라운드 카운터 persist**: `branches` 테이블에 `design_review_round INTEGER DEFAULT 0` 컬럼 필요. 본 subtask 에서 migration v46 으로 add_column_if_missing 추가. Schema 변경 주의.
- **Reviewer engine 미설치**: `codex` CLI 가 시스템에 없으면 첫 라운드 spawn 실패. error 경로에서 UI 에 "codex 미설치 — Settings 에서 reviewer 변경 or 설치" 안내. Gemini fallback 자동화 여부는 Q-2 로 flag.
