---
title: Developer 메인 핸드오프 — community 사용자 follow-up batch (5 plan, 서브에이전트 분배)
plans:
  - docs/plans/rawqVendorFallbackPlan_2026-04-29.md
  - docs/plans/onboardingAnalysisFailureAndSkipUiPlan_2026-04-29.md
  - docs/plans/globalSettingsAndRecentProjectsPlan_2026-04-29.md
  - docs/plans/nativeNotificationPlan_2026-04-29.md
  - docs/plans/docsPanelScopePolicyPlan_2026-04-29.md
created_at: 2026-04-29
issue_source: batmania52 보고 7건 (2026-04-29) — 본 batch 는 6건 처리 (Synerge crash 제외, tunaFlow 무관)
---

# Developer 메인 — community follow-up batch

당신은 메인 Developer 입니다. 오늘은 외부 사용자 batmania52 의 7 보고 중 tunaFlow 측 처리 가능한 6 건을 5 개 plan 으로 묶어 진행합니다. 작업이 axis 별로 분리되어 있으니 **plan 단위로 서브에이전트 (Task tool, subagent_type=general-purpose) 를 spawn** 해서 병렬/순차 처리합니다.

## 0. 가이드라인 (모든 plan 공통, 절대 깨지 마세요)

### 사이드 이펙트 방지
- 각 plan 의 §"회귀 위험 가드" / §"DO NOT" 을 작업 전 한 번 읽고, 작업 후 재확인.
- 변경 영역 외 파일은 절대 수정 금지. `git diff --name-only` 로 매번 확인.
- DB migration / template 텍스트 / 시스템 프롬프트 변경은 명시 plan scope 에만.
- `cfg(target_os = ...)` 격리가 필요한 변경 (notification 등) 은 다른 OS 빌드 회귀 0 임을 빌드 또는 cargo check 로 검증.

### 기능 완료 후 테스트
- 각 task 의 §Verification 명령을 **실제 실행** 하고 결과를 chat 보고.
- baseline 테스트 카운트 (현재 main `cc3e14e`): **FE 381 / Rust 559**. 작업 후 동일 또는 +N (새 unit test 만큼) 이어야 함. 감소 시 회귀.
- UI 변경은 가능한 한 dev 모드 manual smoke 1회 (스크린샷 또는 동작 1줄 보고).
- Rust 변경은 `cargo check + cargo test --lib`, Frontend 는 `npx tsc --noEmit + npx vitest run`.

### 자체 리뷰 (PR 생성 전)
- task 별 commit 후 `git show HEAD --stat` 으로 변경 파일 리스트 self-review.
- DO NOT 리스트 위반 없는지 확인.
- 변경 의도가 plan §"Change description" 과 일치하는지 라인 단위 대조.
- 의심 가는 부분은 PR 보내기 전 chat 으로 Architect 에게 escalate.

### 서브에이전트 spawn 규칙
- 각 plan 은 독립이라 **plan 1개당 서브에이전트 1개** 패턴.
- subagent_type 선택:
  - **Plan A (rawq fallback)** — 단순 shell 스크립트 + docs → general-purpose
  - **Plan B (onboarding skip UI + 진단)** — Frontend + 진단 → general-purpose. Task 02 진단은 Plan tool 도 적합 (코드 변경 0)
  - **Plan C (글로벌 설정 + recent projects)** — Frontend + Rust + DB migration → general-purpose
  - **Plan D (native notification)** — Plan tool 로 진단 (Task 01) 후 별 spawn 으로 fix path → general-purpose
  - **Plan E (docs scope policy)** — **정책 결정 우선**. 서브에이전트 spawn 전 사용자/Architect 에게 option 선택 받기. 결정 후 spawn.

## 1. 작업 순서 — 권장 (병렬 가능 영역 표시)

| 순서 | Plan | 이유 |
|---|---|---|
| **1** | A — rawq vendor fallback | P0 release blocker, 외부 contributor 진입 장벽. 단일 commit, 30분 내. |
| 2 | B — onboarding skip UI + 진단 | P1, 사용자 가시 회귀. Task 01 (UI) + Task 02 (진단). 1~2 PR. |
| 3 | C — 글로벌 설정 + recent projects | P1, UX 영향 큼. DB migration 포함이라 신중. 2 task = 2 PR 분리 권장. |
| 4 (병렬) | D — native notification | P2, 진단 우선 → fix 결정. C 와 영역 다르므로 병렬 가능. |
| 5 (대기) | E — docs scope policy | **정책 결정 대기**. 사용자/Architect 가 P1~P4 중 선택 후 진행. |

A 는 즉시 시작. B/C 는 순차. D 는 C 와 병렬 가능 (DB / settings 영역 충돌 가능성 있어서 C 의 settings 변경분과 동시 작업 시 merge conflict 주의 — 한쪽이 먼저 머지 후 다른 쪽 rebase).

## 2. Plan 별 spawn 가이드

### Plan A — `rawqVendorFallbackPlan_2026-04-29.md`

**서브에이전트 prompt 요점**:
- "Plan SSOT: docs/plans/rawqVendorFallbackPlan_2026-04-29.md. §Subtasks 의 Task 01 (sh + ps1 + .gitignore) 와 Task 02 (INSTALL/README) 를 단일 PR 로 처리. Verification 모두 실행. 베이스라인 외부 사용자 환경 시뮬: `vendor/rawq` 부재 + RAWQ_SRC unset 시 자동 clone 성공해야 함."
- 회귀 가드 강조: 기존 RAWQ_SRC / 3개 로컬 fallback path 의 우선순위 변경 금지.
- 결과 보고: PR URL + clone fallback 동작 명령 출력 1줄.

**브랜치**: `fix/rawq-vendor-fallback`. **CI 정책**: PR + admin merge 가능 (코드 변경 작고 macOS-only 영향 없음).

---

### Plan B — `onboardingAnalysisFailureAndSkipUiPlan_2026-04-29.md`

**서브에이전트 prompt 요점**:
- "Plan SSOT: docs/plans/onboardingAnalysisFailureAndSkipUiPlan_2026-04-29.md. Task 01 (skip 버튼 노출 회복) 우선 fix + PR. Task 02 (codex exit 1 진단) 는 코드 변경 0, chat 보고만."
- ProjectOnboardingModal.tsx:265 부근 Error 분기 정밀 read 후 변경.
- onboardingCancelLeakFixPlan_2026-04-25 의 cancel 흐름과 충돌 없는지 cross-check.
- manual smoke 4 state (loading/success/error/cancel) 모두 확인.

**브랜치**: `fix/onboarding-skip-button` (Task 01) / 진단 (Task 02) 은 brand 없음.

---

### Plan C — `globalSettingsAndRecentProjectsPlan_2026-04-29.md`

**서브에이전트 prompt 요점**:
- "Plan SSOT: docs/plans/globalSettingsAndRecentProjectsPlan_2026-04-29.md. Task 01 (Cmd+, 글로벌 단축키 + macOS 메뉴) 와 Task 02 (recent projects DB + UI) 를 **분리 PR**."
- Task 01: SettingsPanel mount 위치를 RuntimeStatusBar → root 로 이동, store 기반 open state.
- Task 02: DB migration idempotent + path validate fallback 로직 재사용.
- IME 충돌 (textarea focus 시 listener 무시) 확인.

**브랜치**: `feat/global-settings-shortcut` / `feat/recent-projects-list`.

---

### Plan D — `nativeNotificationPlan_2026-04-29.md`

**서브에이전트 prompt 요점 (Task 01 진단 단계)**:
- "Plan SSOT: docs/plans/nativeNotificationPlan_2026-04-29.md. Task 01 진단 — Cargo.toml `tauri-plugin-notification` 버전 확인 + 해당 버전 source 의 macOS path read + Tauri 2.x 최신 plugin native option 제공 여부 확인. 진단 결과 chat 보고. fix 시도 X."
- 결과에 따라 Path A (plugin upgrade) 또는 Path B (직접 bridge) 결정.

**브랜치**: 진단 단계는 brand 없음. fix 단계는 결정 후 별 brand.

---

### Plan E — `docsPanelScopePolicyPlan_2026-04-29.md`

**상태**: status=design (정책 결정 우선).

**서브에이전트 spawn 전 필요한 일**:
- 사용자/Architect 에게 P1~P4 중 선택 chat 질의.
- 결정 후 Plan E 의 Goals/Subtasks 갱신 + status: ready 로 전환.
- 그 후 서브에이전트 spawn.

서브에이전트는 P3 가정 (Recommended) 으로 작업 가이드 작성됨. P1/P2/P4 채택 시 Subtasks 재작성 필요.

## 3. CI 정책

- 각 plan 의 PR 머지: self-trust 기본. 단 Plan C (DB migration) 와 Plan D (native bridge if Path B) 는 **CI watch 필수** — cross-platform 회귀 위험.
- Plan E 는 정책 결정 후 진행이라 별개.
- 머지 후 main 회귀 발생 시 즉시 revert PR.

## 4. 보고 포맷 (Plan 별 완료 시 chat 에)

```
## Plan {A/B/C/D/E} 결과

- 변경 라인 수 + 핵심 파일 (1~3줄)
- Verification 결과: PASS/FAIL + 핵심 출력
- baseline 대비 테스트 카운트 (FE/Rust)
- PR URL + 머지 commit hash
- DO NOT / 회귀 가드 위반 없음 확인 (1줄)
- 다음 plan 진행 여부 또는 escalate 사유
```

## 5. 막히면 (escalate)

- Plan B Task 02 진단이 1시간 이상 답 안 나오면 → 가설 + 재현 명령 chat 보고 후 사용자 판단 대기.
- Plan D Task 01 진단 결과가 Path B (직접 ObjC bridge) 로 가야하면 → 코드 작업량 큼, 사용자/Architect 에게 path 확정 받기.
- Plan E 정책 결정이 대기 중이면 → 다른 plan 4 개 먼저 마무리 후 Plan E 진입.
- 어느 plan 이든 회귀 가드 위반이 의심되면 → 작업 중단 + chat 보고. 임의 fix 금지.

## 6. 본 batch 외 issue (#2 Synerge crash, 답변용)

batmania52 의 #2 보고 (Synerge core crash) 는 Architect 분석 결과 **tunaFlow 무관** (Synerge 의 deprecated `ProcessSerialNumber` API 가 macOS 26.x 에서 회귀). plan 미작성. 사용자(Architect) 가 batmania52 에게 답변할 때 활용:

> "Crash log 분석 결과 tunaFlow 와 무관합니다. Synerge 의 OSXScreenSaver 컴포넌트가 macOS 10.9 이래 deprecated 된 ProcessSerialNumber API 를 쓰고 있는데 macOS 26.x (Tahoe) 에서 회귀해서 다른 앱이 launch/terminate 할 때마다 invalid pointer dereference 로 crash 합니다. tunaFlow 는 trigger 일 뿐 원인이 아닙니다. Symless 의 issue tracker 에 macOS 26.x 회귀로 보고하시면 좋겠습니다."

## 7. 오늘 작업 종료 시 정리 (day-end)

- 머지된 PR 목록 + 머지 commit hash
- 진단만 한 plan (D Task 01, B Task 02) 의 결과 요약
- 정책 결정 대기 plan (E) 의 다음 step
- batmania52 답변 발송 여부 (사용자 영역, Developer 는 정보만 제공)
- 각 plan index.md 의 status 갱신 (ready → in-progress / completed)
