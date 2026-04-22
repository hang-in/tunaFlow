# Subtask 04 — Post-pass Architect review (verdict 수신 + 4 액션)

> 상위 plan: [designReviewGatePlan.md](./designReviewGatePlan.md)
>
> **배경** (2026-04-23): 기존 workflow 는 review pass 시 결과가 메타에이전트로 전달되어 "다음 액션 제안" 단계로 이어졌다. 그러나 개별 plan 의 verdict 해석 + 다음 제안은 plan 작성자 (Architect) 가 맥락을 가장 많이 보유. 사용자 지시로 **review pass 이벤트 수신자를 메타에이전트 → Architect** 로 변경하고, 명시적 4 액션 경로를 제공한다.

## Scope

본 subtask 가 제공:
- `design_review_verdict_ready` 이벤트 수신 경로 (RT 드로어 → Architect session)
- Architect post-review 패널 UI (verdict 요약 + 4 액션 버튼)
- 각 액션의 backend 배선:
  - **액션 1** "그대로 구현 진입" → subtask-03 의 `adopt_design_review` 호출
  - **액션 2** "수정 후 재 review" → plan 문서 편집 대기 + 라운드 카운터 +1
  - **액션 3** "다음 plan 제안" → 신규 Architect session spawn + 현 plan 을 참조 컨텍스트로 주입
  - **액션 4** "사용자에게 질문" → open-question modal + 답변 수신 후 plan 문서 말미 append
- 메타에이전트 routing 에서 review verdict 제거 (raw 연결 해제)

## Changed files

- `src-tauri/src/commands/design_review.rs` — verdict 수신 이벤트 emit + 4 액션별 Tauri commands.
- `src-tauri/src/commands/roundtable_helpers/deliberative.rs` — verdict=pass 시점에 `design_review_verdict_ready` emit.
- `src-tauri/src/commands/meta_agent.rs` (또는 동등) — review verdict routing 제거.
- `src/components/tunaflow/ArchitectPostReviewPanel.tsx` (신규) — verdict 카드 + 4 버튼 + 요약 UI.
- `src/components/tunaflow/chat/PlanProposalCard.tsx` — verdict=pass 시 이 컴포넌트로 상태 전환.
- `src/lib/api/design_review.ts` — 4 액션 wrapper.

## Change description

### 1. 이벤트 spec

```rust
app.emit("design_review_verdict_ready", serde_json::json!({
    "branchId": branch_id,
    "planDocumentPath": plan_document_path,
    "round": current_round,
    "verdict": "pass" | "fail" | "escalate_to_human",
    "blockerCount": n, "majorCount": n, "minorCount": n,
    "failedSubtaskIds": [...],
    "transcriptMarkdown": "...",
}))?;
```

FE 는 이 이벤트를 listen 해 `ArchitectPostReviewPanel` 에 전달. PlanProposalCard 는 verdict=pass 에 한해 이 panel 로 교체 (fail 은 기존 escalate modal 경로 유지).

### 2. Architect 4 액션 UI

```tsx
export function ArchitectPostReviewPanel({ payload }: Props) {
    const [busy, setBusy] = useState<string | null>(null);
    const summary = `Round ${payload.round} — verdict=${payload.verdict}`
        + ` · BLOCKER ${payload.blockerCount}`
        + ` · MAJOR ${payload.majorCount}`
        + ` · MINOR ${payload.minorCount}`;

    const canApprove = payload.verdict === 'pass' && payload.blockerCount === 0;

    return (
        <Panel title="Architect post-review">
            <SummaryRow>{summary}</SummaryRow>
            <TranscriptCollapsible markdown={payload.transcriptMarkdown} />
            <ActionRow>
                <Button disabled={!canApprove || !!busy} onClick={() => approve(payload)}>
                    그대로 구현 진입
                </Button>
                <Button variant="ghost" onClick={() => requestRevision(payload)}>
                    수정 후 재 review
                </Button>
                <Button variant="ghost" onClick={() => proposeNextPlan(payload)}>
                    다음 plan 제안
                </Button>
                <Button variant="ghost" onClick={() => openOpenQuestionModal(payload)}>
                    사용자에게 질문
                </Button>
            </ActionRow>
        </Panel>
    );
}
```

### 3. 각 액션의 backend

```rust
#[tauri::command]
pub async fn post_review_approve_and_implement(
    branch_id: String,
    state: State<'_, DbState>,
    app: AppHandle,
) -> Result<(), AppError> {
    let meta = load_branch_review_meta(&branch_id, &state)?;
    adopt_design_review(
        branch_id, meta.plan_document_path, meta.main_conversation_id,
        state, app.clone(),
    ).await?;
    app.emit("design_review_approved", serde_json::json!({ "branchId": branch_id }))?;
    Ok(())
}

#[tauri::command]
pub fn post_review_request_revision(
    branch_id: String, plan_document_path: String, app: AppHandle,
) -> Result<(), AppError> {
    app.emit("architect_revision_requested", serde_json::json!({
        "branchId": branch_id, "planDocumentPath": plan_document_path,
    }))?;
    Ok(())
}

#[tauri::command]
pub async fn post_review_propose_next_plan(
    branch_id: String, plan_document_path: String,
    state: State<'_, DbState>, app: AppHandle,
) -> Result<(), AppError> {
    let plan_content = std::fs::read_to_string(&plan_document_path)?;
    app.emit("architect_next_plan_requested", serde_json::json!({
        "previousBranchId": branch_id,
        "previousPlanPath": plan_document_path,
        "previousPlanContent": plan_content,
    }))?;
    Ok(())
}
```

### 4. Open question modal

```tsx
function OpenQuestionModal({ payload, onClose }: Props) {
    const [question, setQuestion] = useState("");
    const submit = async () => {
        await invoke('append_user_question_to_plan', {
            planDocumentPath: payload.planDocumentPath,
            round: payload.round, question,
        });
        onClose();
    };
}
```

plan 문서 말미에 `## Architect → User question (Round N)` 섹션 append. 사용자 답변은 plan 문서 직접 편집 또는 후속 chat turn 으로.

### 5. 메타에이전트 routing 해제

기존 `src-tauri/src/commands/meta_agent.rs` (또는 동등) 에서 review verdict listener 제거. 메타에이전트의 다른 input (사용자 요청, 이슈 tracker 등) 은 유지.

**주의**: 메타에이전트가 review verdict 를 다른 목적으로 사용하는 경로가 있을 수 있음 (프로젝트 통계 등). 그런 경우는 route 유지하되, "다음 액션 제안" 역할만 제거. Developer 가 실 코드 경로 확인 후 판단.

## Dependencies

depends_on: [02, 03]

## Verification

- `cargo test --lib commands::design_review::post_review_*`:
  - `post_review_approve_and_implement` — `adopt_design_review` 호출 + `design_review_approved` 이벤트
  - `post_review_request_revision` — `architect_revision_requested` emit, branch archive 되지 않음
  - `post_review_propose_next_plan` — `architect_next_plan_requested` emit + plan content 포함
- `npx vitest run src/components/tunaflow/ArchitectPostReviewPanel.test.tsx`:
  - verdict=pass + BLOCKER=0 시 "그대로 구현 진입" 활성
  - verdict=fail 시 패널 자체가 렌더되지 않음
  - 각 버튼 클릭 시 해당 invoke 호출 1회
- 메타에이전트 routing 해제: `rg "review_verdict_ready|on_review_complete" src-tauri/src` 에서 `meta_agent` 관련 부재 확인
- 수동 E2E: PlanProposalCard → "RT 검토" → 1 라운드 pass → ArchitectPostReviewPanel 자동 표시 → "그대로 구현 진입" → transcript append + Dev 진입

## Risks

- **Architect session lifecycle**: "수정 후 재 review" / "다음 plan 제안" 액션이 기존 session 을 재사용할지, 신규 spawn 할지. 본 subtask 는 이벤트만 emit — Developer 가 실 코드에서 자연스러운 경로 선택.
- **메타에이전트와의 경계**: metaAgent (P0) 미구현 상태라 "routing 해제" 는 선제적. 구현 시점에 review verdict 를 다른 용도로 받게 될 수 있음 — 그 시점에 재판단. 본 plan 은 "'다음 액션 제안' 역할 제외" 만 강제.
- **4 액션 혼란**: "그대로 구현" 과 "수정 후 재 review" 를 섞어 쓰면 plan 상태 꼬임. UI 에 "현재 Round N · verdict" 명시 + 액션 후 panel 비활성화로 동시 호출 방지.
- **Open question 응답 경로**: 사용자 답변이 plan 문서에 append 되어도 Architect 가 자동 읽지 않음. 사용자가 "다시 RT review" 트리거해야 반영. UX 설명 필수.
- **Round 카운터**: "수정 후 재 review" 는 round +1. "다음 plan 제안" 은 신규 plan 이라 round=1 시작. Backend 가 명확히 구분.
