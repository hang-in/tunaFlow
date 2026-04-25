---
title: startReviewRT 진입 실패 매트릭스 — await 단계별 phase rollback 검증
canonical: false
status: audit-snapshot
created_at: 2026-04-25
related:
  - docs/reference/asyncCancelPipelineAudit_2026-04-25.md  # 항목 2
  - docs/plans/reviewRTEntryFailureRollbackPlan_2026-04-25.md
  - src/lib/workflow/reviewWorkflow.ts
---

# 동기

`asyncCancelPipelineAudit_2026-04-25` 의 항목 2 후속 — `startReviewRT` 가 진입 도중
임의 단계에서 throw 했을 때 plan.phase 가 어디서 멈추는지, UI 가 사용자에게 실패를
표면화하는지 검증. line 244 의 코멘트 "Review RT 진입 자체가 throw 되던 버그.
s37 재현 로그로 특정" 가 시사한 동일 카테고리 잠재 버그를 모든 단계에 대해 매트릭스로
정리한다.

# 단계 식별 (현행 코드, `reviewWorkflow.ts` 기준)

`startReviewRT(plan, implMessages, testOutput?, reviewers?, runManualGate?)` 의
정상 흐름 await 단계:

| # | Stage 키 | 위치 | 동작 |
|---|---|---|---|
| 1 | `manual_gate` | line 84-137 | ManualVerificationGate (선택) — fail 시 phase=rework + ManualVerificationFailed throw, cancel 시 phase 유지 + Error throw |
| 2 | `update_phase_review` | line 139 | `updatePlanPhase(plan.id, "review")` — 핵심 phase 전환 |
| 3 | `event_impl_completed` | line 140 | `createPlanEvent("impl_completed", "developer")` |
| 4 | `sync_result_report` | line 142-143 | `syncResultReport(...)` — fire-and-forget (await 없음, 본 매트릭스 제외) |
| 5 | `test_artifact` | line 145-147 | `createTestReportArtifact()` — testOutput 있을 때만, 동기 함수 |
| 6 | `get_or_create_review_branch` | line 161-163 | `getOrCreateReviewBranch()` — DB write + branch 생성/재사용 |
| 7 | `build_plan_context` | line 168 | `buildPlanContext(plan)` — context 조립 |
| 8 | `save_rt_config` | line 245 | `invoke("save_rt_config", ...)` |
| 9 | `save_conversation_engine` | line 254-262 | shadow conv 의 engine/model persist (try/catch 명시) |

> NOTE: 실제 RT spawn (`sendThreadRoundtable`) 은 호출자 (DevProgressView) 책임.
> 본 매트릭스는 `startReviewRT` 함수 내부에 한정.

# 단계별 실패 매트릭스 (수정 전 — Layer A 적용 직전 기준)

각 행 = 해당 stage 가 throw 했을 때:

| Stage | phase 결과 | UI 결과 | 잠재 stuck? |
|---|---|---|---|
| `manual_gate` (fail) | `rework` (line 114 에서 명시 전환) | DevProgressView catch 가 phase 갱신 → rework notice | 안전 |
| `manual_gate` (cancel) | (변동 없음) | DevProgressView catch — toast.info("수동 확인이 취소되었습니다") | 안전 |
| `update_phase_review` | (변동 없음 — DB 업데이트 자체가 실패) | DevProgressView catch (warn) — 사용자에 표면화 약함 | **위험 1** — UI 는 review 진입 시도 흔적 없음 |
| `event_impl_completed` | `review` | DevProgressView catch (warn) — 그러나 reviewBranch 없음 | **위험 2** — phase=review + RT 없음 stuck |
| `test_artifact` | `review` | catch (warn) | **위험 3** — 동일 |
| `get_or_create_review_branch` | `review` | catch (warn) — branch 미생성 | **위험 4** — phase=review + RT 없음 stuck (s37 재현 케이스 후보) |
| `build_plan_context` | `review` | catch (warn) | **위험 5** — 동일 |
| `save_rt_config` | `review` | catch (warn) — branch 는 있으나 RT config 없음 | **위험 6** — phase=review + branch 만 있고 RT 미시작 |
| `save_conversation_engine` | `review` | 무시 (이미 try/catch 로 흡수됨) | 영향 미미 (engine/model 만 미저장) |

## 수정 전 결론

- 위험 1~6 가 invariant **[INV-1]** ("plan.phase=review ⇒ RT 존재 또는
  review_entry_failed event") 를 깨뜨릴 수 있음.
- DevProgressView 의 outer catch 는 단순 `console.warn("[tunaflow]", e)` 로
  사용자에게 명시적 피드백을 주지 않음. setBusy(false) 만 하고 끝나서 사용자는
  "버튼이 응답 없는" UX 만 경험.
- s37 의 fix 코멘트는 stage 8 (`save_rt_config`) 의 `config` vs `configJson`
  argument 키 오타 한 건만 다뤘음. **다른 stage 는 검증되지 않은 상태**.

# 의심 시나리오 (재현 가능성 — grep + 정적 분석)

| 시나리오 | 가장 영향 받는 stage | 재현 난이도 |
|---|---|---|
| DB write lock (busy timeout) | 2, 3, 6, 8 | 중 — 다중 panel 동시 write 부담 시 |
| 네트워크/IPC 단절 (Tauri invoke 실패) | 2, 3, 6, 8 | 낮음 |
| Branch 생성 중 plan FK 위반 (race) | 6 | 낮음 |
| `buildPlanContext` 의 메모리/IO 실패 (impl_messages 매우 큰 경우) | 7 | 낮음 — 정적 함수 |
| LLM/엔진 자체 실패 | (해당 없음 — 본 함수 내부에선 LLM 호출 없음) | — |

본 함수는 LLM 직접 호출이 없으므로, "LLM/엔진 타임아웃" 시나리오는 호출자 측
(`sendThreadRoundtable`) 의 책임. startReviewRT 단독 매트릭스는 **DB / IPC / 정적
조립** 3 영역으로 한정.

# Layer A 적용 후 매트릭스 (예측)

`fix(workflow): step-wise catch + phase rollback in startReviewRT` 적용 후:

| Stage | phase 결과 | UI 결과 | invariant |
|---|---|---|---|
| 2 (`update_phase_review`) | (변동 없음 — rollback 의미 없음, 그대로 throw) | DevProgressView 가 review_entry_failed event 감지 → 재시도 버튼 노출 | INV-1, INV-3 |
| 3 (`event_impl_completed`) | `implementation` 으로 rollback | 재시도 버튼 | INV-1, INV-2, INV-3 |
| 5 (`test_artifact`) | `implementation` 으로 rollback | 재시도 버튼 | INV-1, INV-2, INV-3 |
| 6 (`get_or_create_review_branch`) | `implementation` 으로 rollback | 재시도 버튼 | INV-1, INV-2, INV-3 |
| 7 (`build_plan_context`) | `implementation` 으로 rollback | 재시도 버튼 | INV-1, INV-2, INV-3 |
| 8 (`save_rt_config`) | `implementation` 으로 rollback | 재시도 버튼 | INV-1, INV-2, INV-3 |
| 9 (`save_conversation_engine`) | (변동 없음 — 기존 try/catch 유지) | 영향 미미 | — |

> NOTE: rollback target 은 plan 의 직전 phase 인 `implementation` 사용.
> Plan 본문의 "ready" 표현은 informal naming — 실제 PlanPhase enum 에는 `ready`
> 가 없고, 사용자는 review 시작 직전 `implementation` 상태였으므로 그것이
> 자연스러운 복귀점.

# Layer B (UI) 결정 사항

- **재시도 버튼 노출 조건**: 가장 최근 plan_event 가 `review_entry_failed`
  (그 이후 `review_started` / `review_passed` / `review_failed` 등 Review 진행
  이벤트가 추가로 없음) 이고 plan.phase 가 `implementation` 일 때.
- **클릭 동작**: `handleStartReviewRT` (Deep RT) 또는 `handleStartReview` (Quick)
  를 마지막 사용 트랙대로 재호출. 별도 idempotency key 는 두지 않고, startReviewRT
  자체가 `getOrCreateReviewBranch` 의 `reused` 분기로 idempotent. 단, 사용자가
  Quick → Deep 트랙을 변경한 후 재시도하면 새 트랙으로 진행 (의도된 동작).

# Layer C (로그) 결정 사항

- 각 stage 진입 직전 `console.debug("[startReviewRT.stage]", { stage })`.
- Rust trace_log 까지는 도입하지 않음 — TS 단 함수의 단계 로깅에 무거움.
- 디버깅 시 마지막 도달 stage 를 콘솔에서 추적 가능. plan_event 의 `review_entry_failed`
  detail JSON 에도 `stage` 필드를 포함해 사후 분석에 사용.

# 한계

- 실제 재현 (DB lock 강제, 네트워크 차단) 은 본 audit 에서 수행하지 않았음.
  Layer A 적용 후 정상 경로 unit test 만 추가하고, 실측 재현은 별도 QA 또는
  버그 리포트 시 수행.
- `getOrCreateReviewBranch` 내부의 race (이미 다른 panel 에서 review branch
  생성 중) 는 본 매트릭스 범위 밖. 별 plan 후보.
