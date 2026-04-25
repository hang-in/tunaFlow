---
title: Long-running async + cancel 경로 파이프라인 감사 (5 항목)
canonical: false
status: audit-snapshot
created_at: 2026-04-25
related:
  - docs/plans/onboardingCancelLeakFixPlan_2026-04-25.md
---

# 동기

`onboardingCancelLeakFix` 의 root cause (handleSkip 이 rust task cancel 을 호출 안 해 orphaned task → UI freeze) 가 다른 long-running async 경로에서도 재발 가능한 패턴인지 확인. plan §(5) 약속 산출물.

# 점검 기준 (각 항목 공통)

각 long-running async 경로마다 세 질문:

1. **Cancel flag/token 진입 전 셋업** — async 작업이 시작될 때 cancel 가능 여부 결정됐는가
2. **UI dismiss → rust cancel 호출** — UI 가 모달/드로어를 닫을 때 rust task 도 취소 트리거하는가
3. **Rust task 가 cancel 을 주기적으로 poll** — flag store(true) 만으로 즉시 응답하는가, 아니면 long await 중 무시되는가

# 5 항목 결과

## 1. ManualVerificationGate (B-19, 2026-04-24 머지)

- **위치**: `src/components/workflow/ManualVerificationGate.tsx`, `src/lib/workflow/reviewWorkflow.ts:startReviewRT`
- **상태**: ✅ **OK** — cancel 책임 위임 패턴
- **근거**: ManualVerificationGate 자체엔 cancel 호출 없음. 모달 닫기는 외부에서 받은 `onCancel` callback 으로 처리. `startReviewRT` 가 dialog 결과를 await 한 뒤 `null` (cancel) → `throw new Error("Manual verification cancelled by user")` (reviewWorkflow.ts:97). 호출자 (DevProgressView) 가 catch 하면 phase 유지 + 모달 dismiss
- **관찰**: 이 패턴은 **dialog 가 자체적으로 long-running async 를 갖지 않을 때** 유효. ManualVerificationGate 는 사용자 입력만 기다리고 백그라운드 task 가 없어 OK. onboarding 모달과 다름

## 2. startReviewRT 진입 실패 시 UI 복귀

- **위치**: `src/lib/workflow/reviewWorkflow.ts:244-262`
- **상태**: ⚠️ **확인 필요 — 별 plan 후보**
- **근거**: line 244 의 코멘트 "Review RT 진입 자체가 throw 되던 버그. s37 재현 로그로 특정" 가 과거 동일 카테고리 버그가 있었음을 시사. 현재 catch 는 `saveConversationEngine failed:` (line 262) 한 곳만 명시적. **Plan 생성/RT 브랜치 생성/persist 단계 중 실패 시 phase rollback 보장 여부 불명**
- **후속**: 별 plan 으로 분리 — startReviewRT 의 모든 await 단계 실패 매트릭스 + UI 복귀 경로 검증

## 3. Branch adopt 실패 경로

- **위치**: `src/stores/slices/branchSlice.ts:124 adoptBranch`
- **상태**: ⚠️ **확인 필요 — 별 plan 후보**
- **근거**: adoptBranch 본문 미확인 (grep 만 수행). adopt 중 LLM 호출 / DB write / 메시지 직렬화 실패 시 부분 적용 / orphaned 상태 위험. 이전 세션 메모리 (`project_session_2026-04-12_s25.md`) 에 "adopt 중 스트리밍 메시지 소멸" 이슈가 s25 에 수정됐다고 기록 → 비슷한 잠재 위험 있음
- **후속**: branchSlice.ts:124 본문 + branchSync.ts adopt 경로 깊은 검토. 별 plan

## 4. Plan 생성 중 LLM 응답 실패 → rollback

- **위치**: `src/lib/api/plans.ts:107 generatePlanDocument` (Rust 쪽으로 위임)
- **상태**: ⚠️ **확인 필요 — 별 plan 후보**
- **근거**: generatePlanDocument 는 Tauri command 호출이라 Rust 쪽에서 actual 실패 처리. UI 단 catch 가 phase 를 어떻게 다루는지 미확인
- **후속**: planSlice / planApi / Rust plan generation 경로 audit. 별 plan

## 5. rawq index build 중 취소

- **위치**: `src-tauri/src/agents/rawq.rs ensure_index`, `start_rawq_index`
- **상태**: ❌ **명시적 cancel 채널 부재**
- **근거**: grep 결과 `cancel|abort` 키워드 0 건. ensure_index 는 `Command::wait_with_output` 으로 동기 대기 (`rawq.rs:348`). subprocess kill 외 cancel 경로 없음
- **현실적 영향**: rawq index build 는 보통 수 초~분. 진행 중 사용자가 프로젝트 닫아도 background thread 가 끝까지 진행. 영향: 잠시 CPU 점유. UI freeze 까진 안 갈 가능성 (별 thread 라 main loop 안 막음). 다만 사용자 경험상 "왜 indexing 이 끊기지 않고 끝까지 가나" 의 의문 발생 가능
- **후속**: 별 plan — rawq sidecar 에 cancel 채널 추가 (이미 daemon 모드 운영 중이라 소켓 신호 가능). 우선순위는 높지 않음

# 종합

| # | 영역 | 상태 | 후속 plan |
|---|---|---|---|
| 1 | ManualVerificationGate | ✅ OK (위임 패턴) | 없음 |
| 2 | startReviewRT 진입 실패 | ⚠️ 추가 검토 | `reviewRTEntryFailureRollbackPlan` (가칭) |
| 3 | Branch adopt 실패 | ⚠️ 추가 검토 | `branchAdoptRollbackPlan` (가칭) |
| 4 | Plan 생성 실패 | ⚠️ 추가 검토 | `planGenerationRollbackPlan` (가칭) |
| 5 | rawq index build 취소 | ❌ 부재 | `rawqIndexCancelChannelPlan` (가칭) — 우선순위 낮음 |

# 일반 규칙 (제안)

이번 onboarding fix 와 위 audit 에서 도출되는 invariant:

- **[규칙 1]** Long-running async (LLM call, subprocess, 외부 API) 는 cancel flag 또는 token 을 받아야 한다
- **[규칙 2]** UI 가 dismiss 되는 모든 경로에서 해당 async 의 cancel command 를 호출해야 한다 (idempotent 라면 무조건 호출 권장 — defensive)
- **[규칙 3]** Rust 쪽 task 는 await 사이마다 cancel flag 를 poll 하거나 `tokio::select!` 로 cancel future 와 경쟁시켜야 한다
- **[규칙 4]** Error/실패 시 UI 가 어떤 버튼을 누르든 cancel command 를 호출하는 단일 경로로 수렴해야 한다 (오늘 수정한 onboarding error state 가 이 케이스)

# 본 audit 의 한계

각 ⚠️ 항목은 **grep 수준 confirm** 만 수행. 코드 깊이 들어가 catch 매트릭스 / await 분기 / DB rollback 시점 검증 미수행. 별 plan 으로 승격 시 그 단계에서 깊은 audit. 본 문서는 **이 5 영역이 동일 카테고리 위험을 공유한다는 인식 + 후속 plan 후보 등재** 가 목표.
