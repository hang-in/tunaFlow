---
title: 프로젝트 분석 실패 회귀 + "건너뛰기" 버튼 미노출 회복
status: ready
phase: planning
priority: P1 (외부 사용자 보고)
created_at: 2026-04-29
canonical: true
related:
  - src/components/tunaflow/ProjectOnboardingModal.tsx
  - src/locales/ko/dialog.json
  - docs/plans/codexGeminiOnboardingAnalysisFailureInvestigationPlan_2026-04-25.md
  - docs/plans/onboardingCancelLeakFixPlan_2026-04-25.md
issue_source: batmania52 보고 (#5, 2026-04-29)
---

# Onboarding 분석 실패 + Skip UI 회복

## Context

batmania52 보고:
> "프로젝트 열기 해서 프로젝트 분석 중.. 도중 알 수 없는 이유로 실패했어요. 화면 상에서 아래처럼 나타났습니다. **분석 실패 / codex 분석 실패 (exit: Some(1)) / 건너뛰기를 눌러 빈 템플릿으로 시작할 수 있습니다.** (건너뛰기 라는 버튼은 찾을 수 없었습니다. 그래서 그냥 닫기 눌렀습니다.)"

두 axis:
- (a) **codex 분석 자체 실패 (exit 1)** — 메타에이전트 onboarding analysis 의 codex 엔진 path 문제. 기존 plan `codexGeminiOnboardingAnalysisFailureInvestigationPlan_2026-04-25.md` (P2) 가 같은 영역 다룸. 본 plan 은 그 plan 의 활성화/조사 진행 위주.
- (b) **"건너뛰기" 버튼 미노출** — `ProjectOnboardingModal.tsx:265` 주석에 "Error 상태에서 '건너뛰기' 와 '닫기' 의 사용자 의도가 동일" 명시. 회귀로 의심되며 사용자 가시 라벨이 "닫기" 로 표시되는 듯. dialog.json:38 `skip_button: "건너뛰기"` 라벨은 살아있음 → 렌더링 분기 회귀.

## Goals

- (G1) Error 상태에서 "건너뛰기" 버튼이 명시적으로 보이도록 노출 회복. "닫기" 와 분리 또는 라벨을 "건너뛰기" 로 변경.
- (G2) Codex 분석 exit 1 root cause 좁히기 — 진단만이라도 chat 보고 (즉 fix 시도 전 가설 확정).
- (G3) Codex 실패 시 사용자가 즉시 "Skip with empty template" path 로 빠질 수 있도록 (Goal 1 의 결과).

## Non-goals

- ❌ Codex 분석 자체의 fix (회귀 root cause 까지 가지 않으면 별 plan, 본 plan 은 진단까지).
- ❌ 영문 라벨 변경 (한국어 원래 그대로).
- ❌ Gemini / Claude 분석 path 변경 (Codex 한정).

## Subtasks

### Task 01 — Error 상태 "건너뛰기" 버튼 노출 회복 [P1]

**Changed files**: `src/components/tunaflow/ProjectOnboardingModal.tsx`

**Change description**:
- `ProjectOnboardingModal.tsx:265` 부근 Error 상태 렌더링 분기 확인:
  - 현재 "닫기" 버튼만 보이는지 / "건너뛰기" + "닫기" 둘 다 노출인지 / 라벨이 통합되어 한 버튼에 "닫기" 만 박혀있는지
- 사용자 의도가 동일하더라도 라벨은 분리 — 사용자 보고 메시지가 "건너뛰기" 명시이므로 "건너뛰기" 라벨 우선 노출. "닫기" 별도 노출은 선택.
- 클릭 시 동작은 기존 `handleSkip` (line 153) 또는 `handleSkipFromSelector` (line 140) 와 동일 — 빈 템플릿으로 시작.
- i18n key: `dialog.json` 의 `skip_button` 활용. "닫기" 별도 표시 시 `close_button` 등 별 key.

**Verification**:
- dev 모드에서 Codex 분석 실패 강제 시뮬 (예: `~/.claude/CLAUDE.md` 가 없는 새 프로젝트 또는 rawq 미가용 환경) 후 모달 확인 → "건너뛰기" 라벨 가시
- 클릭 시 빈 템플릿으로 진입
- `npx tsc --noEmit` 통과
- `npx vitest run src/components/tunaflow/` 통과

**회귀 위험 가드**:
- `handleSkip`, `handleSkipFromSelector` 함수 본체 변경 금지 (다른 path 영향).
- ProjectOnboardingModal 의 정상 path (분석 성공 → 분석 결과 표시 → 진행) 영향 없는지 확인.
- `onboardingCancelLeakFixPlan_2026-04-25.md` 의 cancel 흐름 통합과 충돌 없는지 cross-check (그 plan 의 변경분이 main 에 들어가 있다면 그 흐름 보존).

### Task 02 — Codex 분석 exit 1 root cause 진단 [P1, 진단만]

**Changed files**: 없음 (진단)

**Change description**:
- `agents/codex.rs` 또는 `agents/codex_app_server.rs` 의 onboarding analysis 호출 path 추적
- 사용자 환경 재현 시도 (외부 사용자 환경에 가깝게: rawq 미가용 / docs 적은 프로젝트)
- exit 1 시점의 stderr 캡처. claude transport flip (PR 4396aa6) 이후 codex 측에 동일 영향 있는지 확인
- 가설 좁히기 (예: codex CLI 신버전 호환 / API key 미설정 / project context loading 실패)
- chat 으로 가설 + 다음 step 보고

**Verification**:
- 사용자에게 진단 가설 + 재현 명령 chat 보고
- 별 fix 들어가기 전 사용자 결정 받기

**회귀 위험 가드**:
- 진단 단계라 코드 변경 0. 만약 fix 가 명백하면 별 PR 로 분리.

## Cross-cutting risks

| 위험 | 대응 |
|---|---|
| Task 01 fix 가 다른 onboarding state (loading/success) 의 모달 렌더링 회귀 | manual smoke 4 state (loading / success / error / cancel) 모두 확인. unit test 1개 추가 권장. |
| Codex 분석 실패가 사실 다른 root cause (rawq 미가용 등) | Task 02 진단 결과로 좁힘. 본 plan scope 가 "skip path 회복" 이라 코덱스 fix 전이라도 사용자 진입 가능해짐. |

## Rollback

각 task 단독 revert 가능. Task 02 는 코드 변경 0 이라 rollback 불필요.
