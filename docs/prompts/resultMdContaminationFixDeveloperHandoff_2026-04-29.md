---
title: Developer 핸드오프 — result.md contamination fix
plan: docs/plans/resultMdContaminationFixPlan_2026-04-29.md
created_at: 2026-04-29
---

# Developer 핸드오프 — result.md contamination fix

## 0. 한 줄 요약

리뷰어 ContextPack 에 자동 첨부되는 `*-result.md` 본문이 REVIEWER_TEMPLATE 의 "Never judge result.md" 규칙과 모순되어 정책 위반 verdict 를 유발. 입력 단 차단(P0) + truncation/self-include 가드(P1) + i18n 정리(P2) 를 4 개 task 로 처리.

## 1. 작업 개요 — 4 task, 정확한 scope 만 변경

**Plan SSOT**: `docs/plans/resultMdContaminationFixPlan_2026-04-29.md`. 작업 시작 전 Plan 의 §4 Subtasks 를 그대로 따를 것 — 각 task 의 Changed files / Verification / 회귀 가드가 모두 명시되어 있음.

| Task | 파일 | 핵심 변경 | 우선 |
|---|---|---|---|
| 01 | `src-tauri/src/commands/agents_helpers/send_common/context_loading.rs` | line 670-676 result.md 첨부 블록 **삭제** | P0 |
| 02 | `src/lib/workflow/reportSync.ts` | `truncateSafe` 헬퍼 + summary 8k / subtask 2k 상한 + 잘림 마커 | P1 |
| 03 | `src/lib/workflow/reportSync.ts` | sentinel 기반 self-include guard (두 헤더 동시 매칭) | P1 |
| 04 | `src/locales/ko/workflow.json` (+ `en/` 있으면) | review 메시지에서 result.md 경로 라인 삭제 | P2 |

## 2. DO — 반드시 지킬 것

1. **Plan §4 의 Verification 명령을 task 마다 실제로 실행** 하고 결과를 chat 으로 보고. (예: `cargo check` 실행 후 통과 출력 첨부)
2. **Task 01 → 02 → 03 → 04 순서로 진행**. 01 단독으로 root cause 차단 가능하므로 02-03 에서 막히면 01 만 PR 로 분리할 수 있음.
3. **회귀 위험 가드** (각 task 의 "회귀 위험 가드" 섹션) 를 작업 전후로 확인. 특히:
   - Task 01: `context_loading.rs` 의 다른 phase 분기는 절대 수정 금지. 같은 함수 안 task files 첨부(라인 658-668), latest review report 첨부(라인 678-690) 는 유지.
   - Task 02: `reportSync.ts` 의 `syncPlanDocument`, `syncReviewReport`, `lastReworkIdx` 로직은 건드리지 말 것.
   - Task 03: sentinel 패턴은 두 헤더 동시 매칭만. false positive 테스트 추가.
4. **feature 브랜치 생성** 후 작업: `feat/result-md-contamination-fix` 권장.
5. **commit 단위는 task 별 분리**: `fix(reviewer-input): drop result.md from ContextPack (Task 01)` / `fix(reportSync): boundary-safe truncation (Task 02)` / ...
6. **PR description 에 Plan 링크 + 각 task 의 Verification 결과** 첨부.

## 3. DO NOT — 사이드 이펙트 차단

다음은 plan scope 외이며 변경 시 다른 기능에 영향. **절대 수정 금지**.

- ❌ `src-tauri/src/commands/project_tools.rs` 의 REVIEWER_TEMPLATE / DEVELOPER_TEMPLATE 본문 (Plan §3 Non-goals).
- ❌ `src-tauri/src/commands/plans.rs` 의 `generate_result_report`, `build_plan_markdown` 등 Rust write 측 (다른 caller 영향).
- ❌ `src/lib/workflow/reviewWorkflow.ts:186` 또는 `DevProgressView.tsx:123` 의 `syncResultReport` **호출 자체** (UI/event log 의존).
- ❌ `src/lib/workflow/reportSync.ts` 의 다른 export 함수(`syncPlanDocument`, `syncReviewReport`).
- ❌ `src/lib/manualVerification.ts` (lastReworkIdx 로직 약속 공유 — Plan §4 Task 02 회귀 가드).
- ❌ `context_loading.rs` 의 ContextPack 다른 분기 (planning/dev phase, task files 첨부, latest review report 첨부, failure lessons 등).
- ❌ DB 스키마, migration, settings store 관련 — 범위 외.
- ❌ 새 dependency 추가 (헬퍼는 vanilla TS / Rust std 로 충분).

## 4. 변경 후 검증 (전체)

각 task 의 개별 Verification 외에 **PR 머지 직전에 다음 모두 통과 확인**:

```bash
# Rust
cd src-tauri && cargo check --message-format=short
cd src-tauri && cargo test --lib

# Frontend
npx tsc --noEmit
npx vitest run

# 회귀 grep
rg "result\.md" src/locales/                              # Task 04 잔여 확인
rg "phase == .review|phase == .rework" src-tauri/src/commands/agents_helpers/send_common/context_loading.rs
                                                            # 두 분기(task files, review report) 가 살아있는지
```

테스트 카운트는 작업 전 baseline 기록 후 작업 후 동일 또는 +N(새 unit test 만큼) 가 되어야 함. **감소 시 회귀** — 즉시 원인 파악.

## 5. e2e 수동 검증 1회

PR 직전:
1. dummy plan(`docs/plans/test-...`) 생성
2. dev → impl-complete 마커
3. review RT 진입 → reviewer verdict 확인
4. result.md 가 reviewer ContextPack 에 들어가지 **않는지** backend 로그(`[context_pack]` 또는 trace) 확인
5. reviewer 가 task 파일 + 코드만으로 verdict 도출하는지 확인

verdict 가 task 파일/코드 근거로 잘 나오면 ok. result.md 잘림을 근거로 conditional 이 또 나오면 그 자체가 이번 fix 의 미흡 신호 — 즉시 보고.

## 6. CI 정책

- PR 직후 admin merge 즉시 가능 (CI watch 불필요). 자체 검증 §4 통과한 상태로 self-merge.
- merge 후 main 에서 추가 회귀 발생 시 즉시 revert PR 생성.

## 7. 보고 포맷

작업 완료 시 chat 에:
- task 별 변경 라인 수
- 각 Verification 결과 (PASS/FAIL + 핵심 출력)
- e2e 수동 검증 결과 1줄
- PR URL
- 회귀 위험 가드 위반 없음 확인 (Task 01 의 다른 phase 분기 grep 결과 1줄, Task 02 의 syncPlanDocument/syncReviewReport diff 0 확인 등)

## 8. 막히면

- Plan 의 가정이 틀렸다고 판단되면 코드 수정하지 말고 chat 에서 Architect 에게 escalate. 무리한 우회 금지.
- sentinel 패턴 (Task 03) 이 false positive 를 너무 많이 잡으면 Task 03 만 보류하고 01-02-04 로 PR 분리.
- truncation 상향 (Task 02) 이 다른 ContextPack 토큰 budget 을 압박하면 상한값 (8k/2k) 만 조정해서 보고 — 큰 구조 변경 금지.
