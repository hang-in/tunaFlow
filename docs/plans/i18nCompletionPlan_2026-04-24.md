---
title: i18n 완결 plan — A2 잔여 60+ 파일 5 슬라이스 분할 + 병렬 Developer 가이드
status: ready-to-implement
priority: P1 (베타 직후 영어 UX 품질 완결)
created_at: 2026-04-24
related:
  - docs/plans/i18nPlan.md
  - docs/prompts/nextSessionHandoff_2026-04-24.md
---

# i18n 완결 plan

## TL;DR

- 세션 40 까지 **A1 + A2 + A2-B + A2-C** 머지 완료 (Sidebar 까지). 현재 `feat/i18n-pr-a3` (Rust persona/tool 영어) 진행 중.
- 잔여 **60+ 파일** (src/ 기준 한국어 포함) 을 **5 슬라이스** 로 분할: A2-D / A2-E / A2-F / A2-G / A3-ext
- 각 슬라이스는 **파일 경로 conflict 없음** → Developer 2~3 세션 병렬 가능
- 공유 리소스: `src/locales/{ko,en}/*.json` — key 단위 merge 이므로 충돌 낮음 + 발생 시 rebase 쉬움
- INV-6 명시: lib/ 의 워크플로우 agent message body 도 **locale 기반 i18n** (한국어 고정 금지). INV-1 의 "에이전트 시스템 프롬프트 영어 고정" 과 구분.

## 슬라이스 요약

| # | 이름 | 대상 | 예상 파일 | 예상 키 | Namespace |
|---|---|---|---|---|---|
| 1 | **A2-D** | Settings 하위 섹션 전부 | 10 | ~120 | `settings` 확장 |
| 2 | **A2-E** | Context panel 탭 | 10 | ~110 | `insight`, `trace`, `quality`, `skills`, `harness`, `identity` |
| 3 | **A2-F** | Plan / Workflow 카드 + proposal | 10 | ~180 (INV-6 포함) | `workflow` |
| 4 | **A2-G** | Branch / RT / Chat / Input / Common UI | 20 | ~150 | `branch`, `chat`, `dialog`, `common` |
| 5 | **A3-ext** | lib/ + stores/ 서비스·에러·토스트 (Rust A3 의 FE 연장) | 25 | ~100 (INV-6 신중) | `error`, `workflow.msg`, `toast` |
| 합 | | **75 파일, ~660 키** | | | |

---

## 슬라이스 Spec

### Slice A2-D — Settings subpanels

**파일**:
```
src/components/tunaflow/SettingsPanel.tsx                              # outer shell
src/components/tunaflow/settings/HelpSection.tsx                        # 28 KR lines
src/components/tunaflow/settings/MobileSection.tsx
src/components/tunaflow/settings/IdentityAnalysisSettings.tsx
src/components/tunaflow/settings/ProfileSection.tsx
src/components/tunaflow/settings/RuntimeSection.tsx
src/components/tunaflow/settings/ConventionsSection.tsx
src/components/tunaflow/settings/PersonasSection.tsx
src/components/tunaflow/settings/AgentsSection.tsx                       # 잔여 hint 만
src/components/tunaflow/settings/WorldviewSettings.tsx
```

**Namespace**: `settings` 확장. 섹션별 서브키:
```
settings.help.*
settings.mobile.*
settings.identity.*
settings.profile.*
settings.runtime.*
settings.conventions.*
settings.personas.*
settings.agents.*   (A2-C 에서 부분 완료, hint 추가)
settings.worldview.*
```

**검증**:
- Settings 전체 탭 ko ↔ en 전환
- 단축키 테이블 컬럼 폭 깨짐 없음 (en 20~30% 김)
- interpolation (`{{count}}`, `{{path}}`, `{{email}}`) 정상 렌더

**PR 제목**: `feat(i18n): PR A2-D — Settings subpanels (~120 keys)`

---

### Slice A2-E — Context panel tabs

**파일**:
```
src/components/tunaflow/context-panel/InsightPanel.tsx
src/components/tunaflow/context-panel/IdentityView.tsx
src/components/tunaflow/context-panel/PlansPanel.tsx
src/components/tunaflow/context-panel/TracePanel.tsx
src/components/tunaflow/context-panel/QualityDashboard.tsx
src/components/tunaflow/context-panel/SkillsPanel.tsx
src/components/tunaflow/context-panel/HarnessSummary.tsx
src/components/tunaflow/context-panel/insight/insightConstants.tsx
src/components/tunaflow/context-panel/PlanDocumentModal.tsx           # A2-F 와 중복 가능 — 이쪽에서 처리
src/components/tunaflow/context-panel/SubtaskReviewView.tsx
```

**Namespace** (신규 추가):
```
insight.*      (이미 존재)
trace.*        (신규)
quality.*      (신규)
skills.*       (신규)
harness.*      (신규)
identity.*     (신규, identity_view)
```

**주의**: `insightConstants.tsx` 의 카테고리/severity 라벨은 **UI 노출 label** (i18n) 과 **에이전트 프롬프트 카테고리 키** (영어 고정 INV-1) 를 구분해야 함. 영어 identifier 는 불변 소스, 한국어 label 은 locale.

**PR 제목**: `feat(i18n): PR A2-E — Context panel tabs (~110 keys)`

---

### Slice A2-F — Plan / Workflow cards (INV-6 핵심 슬라이스)

**파일**:
```
src/components/tunaflow/context-panel/plans/ApprovalGate.tsx
src/components/tunaflow/context-panel/plans/DraftingActions.tsx
src/components/tunaflow/context-panel/plans/PlanCard.tsx
src/components/tunaflow/context-panel/plans/ReviewVerdictCard.tsx
src/components/tunaflow/context-panel/plans/SubtaskRow.tsx
src/components/tunaflow/context-panel/plans/constants.ts
src/components/tunaflow/context-panel/DevProgressView.tsx
src/components/tunaflow/chat/PlanProposalCard.tsx
src/components/tunaflow/MergeBranchButton.tsx                          # 있으면 포함
```

**Namespace**: `workflow.*` 중앙화. 서브그룹:
```
workflow.plan.*            (PlanCard / ApprovalGate)
workflow.dev.*             (DraftingActions / DevProgressView / SubtaskRow)
workflow.review.*          (ReviewVerdictCard)
workflow.proposal.*        (PlanProposalCard)
workflow.msg.*             (agent 에게 보내는 고정 템플릿 문자열 — INV-6)
```

**INV-6 적용 지점** (중요):
- `sendMessage()` / `appendText()` 로 **agent 에게 전달되는 고정 템플릿** 문자열 (예: `"[Conditional Review] 리뷰어가 일부 수정을 요청했습니다..."`, `"다음 subtask 를 구현하세요..."`) 는 **locale 기반 i18n 대상**. 한국어 고정 금지.
- 이유: chat 히스토리에 user message 로 노출되는 사용자 대면 콘텐츠 + agent 응답 언어는 `preferredLanguages` 로 통제되므로 입출력 언어 일관성.
- 예외: marker (`## ✅ Subtask N 완료`) / 고정 토큰 규약에 엮인 문자열은 i18n 하되 **marker 토큰 자체는 영어 불변**. 번역 품질 주의.

**검증**:
- Workflow 전체 사이클 (Plan → Dev → Review → Merge) 을 en locale 에서 1회 실행 → agent 가 영어로 응답 + UI 전체 영어
- marker parsing 이 depends on 토큰은 여전히 인식됨 확인

**PR 제목**: `feat(i18n): PR A2-F — Workflow cards + INV-6 agent message body (~180 keys)`

---

### Slice A2-G — Branch / RT / Chat / Input / Common

**파일**:
```
src/components/tunaflow/AppShell.tsx
src/components/tunaflow/CenterPanel.tsx
src/components/tunaflow/RuntimeStatusBar.tsx
src/components/tunaflow/TerminalPanel.tsx
src/components/tunaflow/TerminalFloatingPanel.tsx
src/components/tunaflow/BranchThreadPanel.tsx
src/components/tunaflow/CreateRoundtableDialog.tsx
src/components/tunaflow/RoundtableView.tsx
src/components/tunaflow/ChatPanel.tsx
src/components/tunaflow/NewMessageInput.tsx
src/components/tunaflow/MessageItem.tsx
src/components/tunaflow/message/MessageActions.tsx
src/components/tunaflow/input/ContextBadges.tsx
src/components/tunaflow/input/useSendActions.ts
src/components/tunaflow/ContextMenu.tsx
src/components/tunaflow/ProjectOnboardingModal.tsx
src/components/tunaflow/MetaAgentSelector.tsx
src/components/tunaflow/MetaFloatingChat.tsx
src/components/tunaflow/ErrorBoundary.tsx
src/components/tunaflow/NotificationBell.tsx
src/components/tunaflow/sidebar/AddProjectForm.tsx
```

**Namespace**: 기존 확장
```
branch.*  (branch + RT)
chat.*    (chat + message + input)
dialog.*  (dialog + modal)
common.*  (AppShell / StatusBar / Terminal / ContextMenu / ErrorBoundary)
```

**주의**:
- `useSendActions.ts` 에 `sendMessage()` 템플릿 있으면 INV-6 적용 (locale 기반)
- `ErrorBoundary.tsx` 는 최후 fallback — i18n 실패 시에도 영어로는 렌더돼야 함. hard-coded 영어 fallback 유지 OK

**PR 제목**: `feat(i18n): PR A2-G — Branch / RT / Chat / Input / Common (~150 keys)`

---

### Slice A3-ext — lib/ + stores/ 서비스 계층

**파일**:
```
# lib/
src/lib/api/identityAnalysis.ts
src/lib/api/plans.ts
src/lib/attachments.ts
src/lib/errors/extractErrorCode.ts
src/lib/errors/userFriendlyMessage.ts
src/lib/initialSetupApply.ts
src/lib/insightOrchestration.ts                      # Phase 4B 범위 — 본 슬라이스 제외 가능
src/lib/metaAnalysis.ts
src/lib/metaAnalysisTrigger.ts
src/lib/metaNotifications.ts
src/lib/parseIdentitySummary.ts
src/lib/planProposalParser.ts
src/lib/roleAssignments.ts
src/lib/schemas/planProposal.ts
src/lib/skillMappings.ts
src/lib/skillSets.ts
src/lib/toolRequestHandler.ts
src/lib/workflow/branchSync.ts
src/lib/workflow/helpers.ts
src/lib/workflow/implementWorkflow.ts
src/lib/workflow/reviewWorkflow.ts
src/lib/workflow/services/identityArtifactClassifier.ts
src/lib/workflow/services/reviewVerdict.ts
src/lib/workflow/services/subtaskCompletion.ts
src/main.tsx

# stores/
src/stores/ptyStore.ts
src/stores/slices/branchSlice.ts
src/stores/slices/insightSlice.ts
src/stores/slices/ptyMessageSender.ts
src/stores/slices/runtimeSlice.ts
src/stores/slices/threadRtRunner.ts
src/stores/slices/threadSlice.ts
```

**Namespace**:
```
error.*          (AppError code → 사용자 메시지 매핑 — Phase 3-5 완료 후 확장)
workflow.msg.*   (A2-F 에서 선언한 agent message body, lib 에서 재사용)
toast.*          (스토어 액션 실패 시 토스트)
log.*            (console 에러 메시지 — 선택적 i18n, 최소한만)
```

**INV-6 신중 적용**:
- `workflow/helpers.ts`, `implementWorkflow.ts`, `reviewWorkflow.ts` 에는 agent 에게 보내는 템플릿 문자열 대량 존재 가능. 각각 "UI 노출 된다 → i18n" vs "순수 agent 지시문 → 영어 고정" 분류 필수.
- **분류 기준**: 해당 문자열이 chat 히스토리에 user 메시지로 들어가는가? → Yes: i18n locale. No: 영어 고정.
- 실제 코드 확인 후 분류표 작성 → PR 설명에 첨부.

**insightOrchestration.ts 제외**: Phase 4B (별도 PR B) 범위. 본 슬라이스에서는 건드리지 않음.

**검증**:
- `cargo test --lib` + `npx vitest run` 전량 pass
- marker 의존 parsing 회귀 없음 (planProposalParser / subtaskCompletion / reviewVerdict)
- Error code 매핑 (Phase 3-5) 유지

**PR 제목**: `feat(i18n): PR A3-ext — lib/stores services + INV-6 agent body (~100 keys)`

---

## Invariants

**기존 i18nPlan INV 1~5 유지**. 추가:

- **[INV-6]** `sendMessage()` / `appendText()` 등으로 agent 에게 전달되는 **고정 템플릿 문자열** (워크플로우 UI 버튼이 생성하는 메시지 body) 은 **user locale 기반 i18n 대상**. agent 응답 언어는 ContextPack `user_profile.preferredLanguages` 로 별도 통제되므로 입력과 출력 언어가 자연스럽게 일치. INV-1 의 "시스템 프롬프트 / 페르소나 / 도구 스키마 영어 고정" 과 구분.
  - **예외**: marker / 고정 토큰 규약 (`## ✅ Subtask N 완료`, `<!-- tunaflow:... -->`) 에 엮인 문자열은 i18n 하되 **토큰 자체는 영어 불변**. 번역 시 이 토큰을 replaceable placeholder 로 취급하지 않음.
  - **검증**: PR A2-F / A3-ext 머지 시 en locale 로 Plan → Dev → Review → Merge 사이클 1회 실행 → agent 응답 영어 + marker parsing 정상.

- **[INV-7]** 5 슬라이스 간 파일 경로 겹침 **없음**. 공유 리소스는 `src/locales/{ko,en}/*.json` 만이며 key 추가 단위 이므로 git merge 충돌 확률 낮음. 충돌 시 key-by-key 수동 resolve.
  - **검증**: 각 PR 머지 시 `git log --name-only` 로 파일 경로 list 추출 → 겹침 0 확인.

---

## 병렬 실행 전략

### 2 세션 병렬 (권장)

| 세션 | 슬라이스 순서 | 이유 |
|---|---|---|
| Track 1 | **A2-D → A2-E** | UI 라벨 중심, 단순. 빠르게 머지 누적 |
| Track 2 | **A2-F → A3-ext** | INV-6 판정 필요. 신중 진행 |
| (순차) | **A2-G** (Track 1 세션 이어서) | 가장 광범위 — 위 두 Track 끝나고 나중에 |

### 3 세션 병렬 (빠른 완결)

| 세션 | 슬라이스 |
|---|---|
| Track 1 | A2-D |
| Track 2 | A2-E |
| Track 3 | A2-F |
| 이후 순차 | A2-G → A3-ext |

locale JSON 충돌 시: 각 Developer 가 자기 namespace 만 추가하면 key-level merge 자동 해결.

### 단일 세션 (최저 리스크)

순서: **A2-D → A2-E → A2-F → A2-G → A3-ext**. 각 슬라이스 PR 분리 머지 후 다음 진행.

---

## Developer 핸드오프 프롬프트 (슬라이스별)

> 각 Developer 세션에 **해당 슬라이스 blob 하나만** 붙여넣는다. 다른 슬라이스는 별도 세션.

### Blob A2-D

```
[작업] i18n PR A2-D — Settings subpanels

[SSOT] docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-D" + docs/plans/i18nPlan.md 먼저 읽기

[범위]
10 파일 (SettingsPanel shell + 9 하위 섹션). 예상 ~120 키.
대상 파일 목록: plan 의 §"Slice A2-D" 참고.

[Namespace]
settings.{help,mobile,identity,profile,runtime,conventions,personas,agents,worldview}.* 확장

[패턴 준수]
기존 A2-C (Sidebar) PR #161 작업 패턴 그대로 — ko.json 먼저 채우고 en.json 은 Claude/Gemini 로 번역 후 수동 검수.

[INV]
- INV-1 유지 (Agent system prompt 영어 고정). 본 슬라이스는 UI 라벨만.
- INV-5 (namespace.section.action 3계층).
- INV-7 (A2-E 등 다른 슬라이스와 파일 경로 겹치지 않음).

[검증]
- npx tsc --noEmit / npx vitest run / npx vite build
- 수동: Settings 전체 탭 ko ↔ en 전환, 컬럼 폭 깨짐 없음, interpolation 정상

[브랜치/PR]
feat/i18n-pr-a2d
PR 제목: feat(i18n): PR A2-D — Settings subpanels (~120 keys)
각 커밋 본문에 Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
```

### Blob A2-E

```
[작업] i18n PR A2-E — Context panel tabs

[SSOT] docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-E"

[범위]
10 파일 (Insight/Identity/Plans/Trace/Quality/Skills/Harness/insightConstants/PlanDocumentModal/SubtaskReviewView). 예상 ~110 키.

[Namespace 신규 추가]
trace, quality, skills, harness, identity (insight 는 기존 확장)

[주의]
insightConstants.tsx 의 카테고리/severity 는 **영어 identifier (불변) + 한국어 label (i18n)** 두 축 구분. identifier 는 에이전트 프롬프트 카테고리 키로 사용되므로 영어 고정.

[검증/브랜치/PR]
feat/i18n-pr-a2e
feat(i18n): PR A2-E — Context panel tabs (~110 keys)
```

### Blob A2-F

```
[작업] i18n PR A2-F — Workflow cards + INV-6 agent message body

[SSOT] docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-F" + §"Invariants INV-6"

[범위]
9 파일 (ApprovalGate/DraftingActions/PlanCard/ReviewVerdictCard/SubtaskRow/constants.ts/DevProgressView/PlanProposalCard/MergeBranchButton). 예상 ~180 키 (UI 145 + agent body 35).

[INV-6 핵심 작업]
sendMessage() / appendText() 로 agent 에게 전달되는 고정 템플릿 문자열 (예: "[Conditional Review] 리뷰어가..." / "다음 subtask 를 구현하세요...") 도 locale 기반 i18n.
이유: chat 히스토리에 user message 로 노출되는 사용자 대면 콘텐츠 + agent 응답 언어는 preferredLanguages 로 통제.

[예외]
marker 토큰 ("## ✅ Subtask N 완료", "<!-- tunaflow:... -->") 은 i18n 하되 토큰 자체는 영어 불변. 번역 시 placeholder 로 취급하지 말 것.

[검증]
- en locale 로 Plan → Dev → Review → Merge 1 cycle: agent 영어 응답 + marker parsing 정상
- subtaskCompletion / reviewVerdict parser 회귀 없음

[브랜치/PR]
feat/i18n-pr-a2f
feat(i18n): PR A2-F — Workflow cards + INV-6 agent body (~180 keys)
```

### Blob A2-G

```
[작업] i18n PR A2-G — Branch / RT / Chat / Input / Common

[SSOT] docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A2-G"

[범위]
20+ 파일. 예상 ~150 키. Namespace: branch, chat, dialog, common 확장.

[주의]
- useSendActions.ts 에 sendMessage 템플릿 있으면 INV-6 적용
- ErrorBoundary.tsx fallback 은 i18n 실패해도 렌더돼야 → 영어 hardcoded fallback 유지

[검증/브랜치/PR]
feat/i18n-pr-a2g
feat(i18n): PR A2-G — Branch/RT/Chat/Common (~150 keys)
```

### Blob A3-ext

```
[작업] i18n PR A3-ext — lib/ + stores/ services

[SSOT] docs/plans/i18nCompletionPlan_2026-04-24.md §"Slice A3-ext" + §"Invariants INV-6"

[범위]
25 파일 (lib/api, errors, workflow, metaAnalysis, planProposalParser, schemas, skill*, toolRequestHandler + stores/slices/*, main.tsx).
insightOrchestration.ts 는 Phase 4B 범위 — 본 PR 제외.

[INV-6 신중 작업]
lib/workflow/{helpers, implementWorkflow, reviewWorkflow}.ts 에 agent 메시지 템플릿 대량. 각 문자열 분류:
  - chat 히스토리에 user 메시지로 노출 → i18n locale
  - 순수 agent 지시문 (UI 미노출) → 영어 고정

PR 설명에 분류표 첨부.

[검증]
- cargo test --lib / npx vitest run 전량 pass
- marker 의존 parsing (planProposalParser, subtaskCompletion, reviewVerdict) 회귀 없음
- Error code → i18n key 매핑 정상

[브랜치/PR]
feat/i18n-pr-a3-ext
feat(i18n): PR A3-ext — lib/stores + INV-6 (~100 keys)
```

---

## Rationale

### 왜 5 슬라이스로 분할

- 단일 PR 으로 60+ 파일 처리는 리뷰 부담 극단, rebase 실패 시 작업 소실 리스크
- 기존 A2-C (Sidebar) 가 3 파일 +51/-15 로 깔끔하게 머지됨 — 같은 리듬 유지
- 파일 경로 독립 = 병렬 가능, Developer 세션 bottleneck 해소

### 왜 A3-ext 가 별도

- lib/stores 계층은 UI 와 달리 **분류 작업** (INV-6 판정) 이 필요
- Rust A3 (persona / tool 영어 고정) 와 symmetry — FE lib 도 "서비스/에러 계층" 이라는 성격
- 향후 Rust A3 확장 (roundtable_helpers 등) 과 paired review 용이

### INV-6 가 왜 INV-1 과 다른가

| 분류 | 예시 | INV-1 | INV-6 |
|---|---|---|---|
| **시스템 프롬프트** | defaultPersonas.ts description, insightOrchestration.ts 지시문, tool schema description | **영어 고정** | — |
| **Agent 에게 전달되는 사용자 메시지 body** | "다음 subtask 를 구현하세요", "리뷰어가 일부 수정을 요청했습니다" | — | **locale i18n** |
| **순수 UI 라벨** | 버튼, 상태, 타이틀 | — | locale i18n |

INV-1 은 "매 요청 동일 고정 시스템 지시어", INV-6 은 "동적 사용자 메시지". 둘이 혼재하지 않음.

### 왜 Rust A3 가 먼저이고 FE A3-ext 가 나중

- Rust A3 (persona / tool / roundtable_helpers) 는 **시스템 프롬프트** — INV-1 직접 대상
- FE A3-ext 는 **사용자 메시지 body** — INV-6 대상
- 두 INV 의 경계를 Rust A3 가 먼저 확정 (영어 고정 영역 명확화) → FE A3-ext 는 "나머지는 locale i18n" 으로 안전하게 분류 가능

---

## 완결 후 상태

- i18n PR A (Phase 1-4A) 완전 종결
- 영어 사용자가 tunaFlow 전 UI + 워크플로우 cycle 을 영어로 완주 가능
- 잔여는 **Phase 4B (insightOrchestration 영어 전환 + A/B 검증)** 뿐 — 이건 베타 후 별도 PR B

## 관련 문서

- `docs/plans/i18nPlan.md` — 원 plan + §12 PR A 핸드오프 + §13 PR B 핸드오프
- `docs/prompts/nextSessionHandoff_2026-04-24.md` — 세션 41 SSOT
- `docs/plans/preBetaAuditPlan_2026-04-23.md` — 베타 감사 (i18n 관련 P1 항목 포함)
