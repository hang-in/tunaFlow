# Subtask 01 — Artifact 자동 생성 지점 보강 (6 타입)

> 상위 plan: [projectIdentityAnalysisPlan.md](./projectIdentityAnalysisPlan.md)

## Changed files

- `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` — `finalize_engine_run` 류에 `decision` / `finding_success` / `finding_failure` 자동 생성 경로.
- `src-tauri/src/commands/roundtable_helpers/persist.rs` — Review verdict 확정 시 `review_outcome` 자동 생성.
- `src-tauri/src/commands/plans.rs` (또는 plan lifecycle 담당 모듈) — Plan 승인 / Rework 진입 / Plan 완료 지점에 각각 `decision` / `rework_reason` / `workflow_milestone` 자동 생성.
- `src-tauri/src/commands/artifacts.rs` — `create_identity_input_artifact(ArtifactKind::*)` 헬퍼 + 자동 생성용 validator.
- `src-tauri/src/db/models.rs` — `ArtifactKind` enum (상수 6개 + `IdentitySummary` 7번째).

## Change description

### 1. `ArtifactKind` enum 신설

```rust
// src-tauri/src/db/models.rs
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub enum ArtifactKind {
    Decision,
    ReviewOutcome,
    ReworkReason,
    FindingSuccess,
    FindingFailure,
    WorkflowMilestone,
    IdentitySummary,   // 분석 output. subtask-03 에서 사용.
}

impl ArtifactKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::ReviewOutcome => "review_outcome",
            Self::ReworkReason => "rework_reason",
            Self::FindingSuccess => "finding_success",
            Self::FindingFailure => "finding_failure",
            Self::WorkflowMilestone => "workflow_milestone",
            Self::IdentitySummary => "identity_summary",
        }
    }
    pub fn is_identity_input(&self) -> bool {
        !matches!(self, Self::IdentitySummary)
    }
}
```

### 2. `create_identity_input_artifact` 헬퍼

```rust
// src-tauri/src/commands/artifacts.rs
pub fn create_identity_input_artifact(
    conn: &Connection,
    kind: ArtifactKind,
    conversation_id: Option<&str>,
    plan_id: Option<&str>,
    subtask_id: Option<&str>,
    title: &str,
    content_json: serde_json::Value,
) -> Result<String, AppError> {
    // INV-1 enforcement: IdentitySummary 는 이 헬퍼로 만들 수 없음 (subtask-03 의 별도 경로)
    if !kind.is_identity_input() {
        return Err(AppError::BadRequest(
            "create_identity_input_artifact rejects IdentitySummary kind".into()
        ));
    }
    let id = format!("art-{}", Uuid::new_v4());
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO artifacts (id, conversation_id, branch_id, subtask_id, plan_id, type, title, content, status, created_at, updated_at)
         VALUES (?1,?2,NULL,?3,?4,?5,?6,?7,'draft',?8,?8)",
        params![
            id, conversation_id, subtask_id, plan_id,
            kind.as_str(), title,
            content_json.to_string(),
            now,
        ],
    )?;
    Ok(id)
}
```

### 3. 자동 생성 경로 6개

각각 "워크플로우 이벤트 발생 시점" 에 1회 호출. 대화 내용 파싱으로 감정 추론 금지 (INV-1).

#### 3.1 `decision` — Plan 승인 / 경로 선택

위치: `plans.rs::approve_plan` (또는 plan.phase='approved' 전이)

```rust
let content = serde_json::json!({
    "what": "plan_approved",
    "plan_slug": plan.slug,
    "previous_phase": "subtask_review",
    "approved_by": "user",
});
create_identity_input_artifact(
    &conn, ArtifactKind::Decision,
    Some(&plan.conversation_id), Some(&plan.id), None,
    &format!("Plan '{}' approved", plan.title),
    content,
)?;
```

추가 시점:
- 엔진 전환 (`set_conversation_engine` 호출 시 `previous != current` 면)
- 사용자가 designReviewGate 에서 "force_approve" 선택 시 (blocker 잔존 plan 승인)

#### 3.2 `review_outcome` — Review verdict 확정

위치: `roundtable_helpers/persist.rs` 또는 Review verdict parse 완료 지점

```rust
let content = serde_json::json!({
    "verdict": verdict.verdict,                     // "pass" | "fail" | "escalate_to_human"
    "rubric": verdict.rubric,                       // {plan_coverage, code_quality, test_coverage, convention}
    "findings_count": verdict.findings.len(),
    "failed_subtask_ids": verdict.failed_subtask_ids,
    "reviewer_engine": reviewer_profile.engine,
    "round": round_number,
});
```

#### 3.3 `rework_reason` — Rework 진입

위치: `plans.rs::handle_rework` (phase → `rework`)

```rust
let content = serde_json::json!({
    "cycle": plan.rework_cycle,
    "findings": prev_verdict.findings,
    "root_cause_hint": prev_verdict.recommendations.join("; "),
});
```

#### 3.4 `finding_success` — Dev 가 subtask 를 scope 내 완료

위치: `subtaskCompletion.ts` / `syncSubtaskCompletion` (Rust) 의 DB status='done' 전이 지점

```rust
let content = serde_json::json!({
    "subtask_id": subtask.id,
    "duration_ms": calculated_duration,
    "agent_engine": impl_branch.engine,
    "notes": null,
});
```

**생성 조건**: subtask status 'in_progress' → 'done' 전이 **AND** 해당 subtask 에 대한 직전 review 가 pass. 단순 DB status 만 보면 noise 증가.

#### 3.5 `finding_failure` — Dev 응답이 scope 벗어남 / blocker

위치: Review verdict=fail + failed_subtask_ids 있는 경우 각 subtask 에 대해 1건

```rust
let content = serde_json::json!({
    "subtask_id": subtask.id,
    "failure_kind": "scope_violation" | "blocker" | "test_failure" | "convention",
    "agent_engine": impl_branch.engine,
    "evidence_msg_id": verdict_message_id,
});
```

**생성 조건**: review_outcome verdict=fail, 해당 subtask 가 `failed_subtask_ids` 에 포함.

#### 3.6 `workflow_milestone` — Plan 완료 / 머지

위치: `plans.rs::complete_plan` + PR 머지 훅 (있다면)

```rust
let content = serde_json::json!({
    "milestone_kind": "plan_done" | "pr_merged" | "release_tagged",
    "plan_id": plan.id,
    "summary": <1-line plan summary>,
});
```

### 4. Dedup / noise guard

- 같은 (conversation_id, type, hash(content)) 가 1 분 이내 중복 생성 시 skip (fat-finger 보호)
- `status='draft'` 로 저장 후 subtask-03 의 분석기가 `status='analyzed'` 로 mark

## Dependencies

depends_on: 없음 (artifacts 테이블은 이미 존재, 새 migration 불필요).

## Verification

- `cargo test --lib commands::artifacts::create_identity_input_artifact`:
  - 6 종 타입 각각 성공 생성
  - IdentitySummary 타입은 Err 반환 (INV-1)
- 각 자동 생성 경로 integration test:
  - `approve_plan` 호출 시 `decision` artifact 1건 추가
  - Review verdict 파싱 시 `review_outcome` 1건 추가
  - fail + failed_subtask_ids=[1,2] 시 `finding_failure` 2건 추가
  - Rework 진입 시 `rework_reason` 1건 + 이전 `review_outcome` 는 유지
  - Plan 완료 시 `workflow_milestone` 1건
- Negative test:
  - 임의 user message 에 "결정" 같은 한국어 포함 → 자동 artifact 생성 되지 **않음** (INV-1 surveillance 금지)
- `cargo check` — exit 0.

## Risks

- **Dedup 범위**: 1 분 내 same-content 중복 차단은 conservative. 실제로 legitimate 한 중복 (재진입 등) 을 block 할 수 있음. 실측 후 조정.
- **finding_success 판정 기준**: "review pass AND subtask done" 조합이 실제로 어떤 시점에 정확히 판정되는지 — review round 가 전체 plan 에 한 번만 돌 수도, subtask 별로 돌 수도. 현재 tunaFlow 는 plan 단위 review 가 기본 — 따라서 "해당 subtask 가 그 review 의 failed_ids 에 없으면 success" 로 정의. 명시.
- **Enum 도입 side-effect**: `ArtifactKind` enum 이 기존 `String type` 필드와 공존. 기존 코드가 자유 문자열을 쓰고 있으면 enum parse 실패 지점 발생 가능. Parse 실패 시 "unknown" 으로 fallback + 경고 로그, enum 확장 고려.
- **artifacts 테이블 row 증가율**: plan 당 평균 4~8 개 artifact 자동 생성 예상 → 월 수십~수백 row. archive 없이는 몇 년 후 수만 row. 당장 문제 아니지만 `longTermMemoryRoadmapPlan` 의 decay 정책과 연동 후속 고려.
- **역사적 artifact 누적 없음**: 본 subtask 머지 시점 이후부터 artifact 가 쌓이므로 **최초 `identity_summary` 는 data 빈곤 가능성** (subtask-02 의 threshold guard 가 이걸 방어 — 10개 미만이면 분석 skip).
