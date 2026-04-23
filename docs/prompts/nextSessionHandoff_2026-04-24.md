---
title: 다음 세션 핸드오프 — 베타 공개 직전 잔여 작업
created_at: 2026-04-24
scope: Developer session
---

# 다음 세션 핸드오프 — i18n 잔여 + 베타 공개 마감

## 0. 현재 상태 요약 (2026-04-23~24 세션 40 종료 시점)

- **main 최신 머지**: PR #161 (Sidebar 전환)
- **열린 PR 0건** (사용자가 #161 머지하면)
- **DB schema**: v46 (agent_jobs priority/dedupe_key/visibility 추가됨)
- **Rust tests**: 485 / **Frontend tests**: 322 / tsc 통과

## 1. 세션 40 에 완결된 plan / 주요 머지

| 영역 | PR | 상태 |
|---|---|---|
| projectIdentityAnalysisPlan subtask-01 Phase A | #149 | ✅ |
| projectIdentityAnalysisPlan subtask-01 Phase B | #150 | ✅ |
| metaAgent Phase 4 bg worker | #151 | ✅ |
| metaAgent Phase 3 identity trigger | #152 | ✅ |
| projectIdentityAnalysisPlan subtask-03 (analyzer + ContextPack inject) | #153 | ✅ |
| projectIdentityAnalysisPlan subtask-04 (Insight UI) | #154 | ✅ |
| Settings UI + StatusBar badge | #155 | ✅ |
| insightStability 4-bug chain | #156 | ✅ |
| public release Phase 0~4 | #157 | ✅ |
| i18n PR A1 (infra + error contract) | #158 | ✅ |
| i18n PR A2 partial (4 namespace + Settings) | #159 | ✅ |
| i18n PR A2-B (NewMessageInput + ChatPanel) | #160 | ✅ |
| i18n PR A2-C (Sidebar.tsx 15 strings) | #161 | 대기 (사용자 머지) |

## 2. 남은 작업 — 우선순위 순서

### P0 — 베타 공개 blocker 에 가까운 것

#### P0-1. i18n PR A 잔여 슬라이스 (4개)

우선순위 순으로:

**A2-D (sidebar 하위 컴포넌트)**
- 파일: `src/components/tunaflow/MessageItem.tsx`, `src/components/tunaflow/sidebar/TreeRow.tsx`, `ChatsSection.tsx`, `FilesSection.tsx`, `DocsSection.tsx`, `ScratchpadSection.tsx`, `SidebarContextMenu.tsx`
- 기존 `sidebar.json` 확장 필요 (각 section 의 고유 라벨들)
- `chat.json` 의 `message.*` 키는 이미 있음

**A2-E (workflow namespace 신규)**
- 파일: `src/components/tunaflow/context-panel/plans/*.tsx` (PlanCard, ApprovalGate, DraftingActions, ReviewVerdictCard, MergeBranchButton), `src/components/tunaflow/context-panel/DevProgressView.tsx`, `src/components/tunaflow/chat/PlanProposalCard.tsx`
- `src/locales/{ko,en}/workflow.json` 신규 생성 필요
- 예상 key 수: ~50 (plan 상태 / 버튼 / toast / rubric 라벨)

**A2-F (branch + roundtable namespace 신규)**
- 파일: `src/components/tunaflow/BranchThreadPanel.tsx`, `CreateRoundtableDialog.tsx`, Branch 드로어 관련
- `src/locales/{ko,en}/branch.json` 신규
- RT 관련 labels + adopt/archive/create 메시지

**A2-G (insight namespace 신규)**
- 파일: `src/components/tunaflow/context-panel/InsightPanel.tsx` 본문, `IdentityView.tsx` 본문 (일부는 이미 전환됨)
- `src/locales/{ko,en}/insight.json` 신규
- finding 카테고리 라벨 / severity / quadrant 라벨

**A3 (Rust 페르소나 + 도구 영어화)**
- `src/lib/defaultPersonas.ts` — description / traits / instructions 영어로 (사용자 대면 label 만 i18n key)
- `src-tauri/src/agents/tool_handler.rs` — 도구 스키마 description 영어로
- `src-tauri/src/commands/roundtable_helpers/*` — RT role_guidance 영어로
- INV-1 (에이전트 프롬프트 영어 고정) 의 마지막 조각

#### P0-2. 베타 공개 최종 작업 (Phase 5)

**선행 조건**: A2-D/E/F/G + A3 완료 후.

1. README 보류 섹션 작성 (**사용자 입력 필요**):
   - "Built with tunaFlow" 섹션 — secall / tunaReader / tunaInsight GitHub URL
   - "References & Acknowledgments" — `_util/` 원 repo URL + 차용한 아이디어
2. `gh repo edit --visibility public`
3. `git tag v0.1.0-beta -m "First public beta release"` + `git push origin v0.1.0-beta`
4. GitHub Release 작성 — Vision/Mission, 지원 엔진, 알려진 제약, 설치 방법, Built with 4 프로젝트

### P1 — 베타 공개 이후

#### P1-1. i18n PR B (insightOrchestration 영어화)

- `src/lib/insightOrchestration.ts` 시스템 프롬프트 영어 재작성
- A/B 검증: 한국어 버전 vs 영어 버전 (finding 수 / severity / JSON 파싱률 / 구체성)
- 품질 하락 시 한국어 유지 (기능 > 일관성)
- Plan 참조: `docs/plans/i18nPlan.md` §13

#### P1-2. 관찰 포인트 (실사용 감시)

- `review_outcome.failed_subtask_ids` 정확도 — reviewer 프롬프트가 해당 필드를 일관되게 작성하는지
- `identity_analysis` job 실측 비용 + 시간
- `background_insight_progress` 이벤트 UX 피드백

## 3. 개발 환경 상태 (참고)

### 알려진 로컬 patch (upstream 없음)

- `/Users/d9ng/privateProject/_research/_util/rawq/src/search/engine.rs` — ctx_before + chunk_start clamp
- `/Users/d9ng/privateProject/tunaDish/vendor/rawq/src/search/engine.rs` — 동일
- sidecar 바이너리: `src-tauri/binaries/rawq-aarch64-apple-darwin` 에 재빌드된 결과 있음. 재빌드 시 `./scripts/build-rawq.sh` (tunaDish 경로 우선)

### 사용자 환경 변수 override

- `TUNAFLOW_IDENTITY_ANALYSIS_THRESHOLD` — Settings atomic 보다 우선. default 10
- `TUNAFLOW_DISABLE_RESUME_BOOTSTRAP` — **이미 제거됨** (PR #142)

### DB 상태

- v46 — `agent_jobs` 에 priority/dedupe_key/visibility 포함
- v45 — `messages_fts` standalone + `content_tokenized` 컬럼 (아직 backfill 안 됨 — `rebuild_messages_fts` 수동 실행 필요, UI 는 searchPipeline part-2 subtask-05 에서 추가 예정)

## 4. 중요한 invariants / 기억할 것

### Token Policy (주요 철학)

- tunaFlow 는 **토큰 절약 도구가 아님**. 품질 우선. AGENTS.md 수준 1.5~3K tokens 허용.
- 회피 대상: 중복 재주입 / stale context / doubling / 불필요한 tool 호출 반복
- SSOT: `docs/reference/tokenPolicyReference.md`

### session continuity (claude sdk-url 세션)

- `current_session_key` RESUME_IDS 우선 (router UUID 가 아닌 claude session_id)
- `--session-id` vs `--resume` 인자 상호배타 (PR #137 hot fix)
- Session Continuity Fix INV-1~7 전부 해소됨

### Identity artifact 생성 규칙

- 워크플로 "이벤트 발생 시점" 에만 — 대화 내용 파싱 금지 (INV-1)
- `IdentitySummary` kind 는 `create_identity_input_artifact` 가 거부 — analyzer 만 `create_identity_summary` 로 저장
- 1분 window dedup
- `BACKGROUND_INSIGHT_ENABLED` 토글로 Phase 3/4 모두 OFF 가능 (INV-3)

### 코드 수정 원칙 (user 지시)

- 에이전트 프롬프트 / 페르소나 / 도구 description 은 **영어 고정**
- Rust 측에 locale 상태/invoke 인자 없음 — `{ code, context, message }` 만 반환
- FE 의 catch 블록은 `extractErrorCode(e)` + `t(\`error.\${code}\`, { context })` 패턴으로 점진 이관 (후속 세션)

## 5. 로컬/세션 environment

- 현재 브랜치: `main` (이 세션 종료 시)
- OS: darwin 25.4.0, macOS
- claude CLI: 2.1.117

---

## 다음 세션 첫 프롬프트

다음 세션에 그대로 붙여넣기:

```
[세션 목표] i18n PR A 의 A2-D 슬라이스 착수 (sidebar 하위 컴포넌트).

[SSOT]
- `docs/reference/sessionHistory.md` 세션 40 엔트리 (2026-04-23~24 완결 사항)
- `docs/prompts/nextSessionHandoff_2026-04-24.md` (본 프롬프트 출처)
- `docs/plans/i18nPlan.md` — i18n 상위 plan

[현재 상태]
- PR #161 머지 완료 가정. 열린 PR 0건. DB v46. tests Rust 485 / FE 322
- 세션 40 에서 identity 전체 + public release prep + i18n A1~A2-C 완결

[이번 세션 scope — A2-D]
대상 파일:
- src/components/tunaflow/MessageItem.tsx
- src/components/tunaflow/sidebar/TreeRow.tsx
- src/components/tunaflow/sidebar/ChatsSection.tsx
- src/components/tunaflow/sidebar/FilesSection.tsx
- src/components/tunaflow/sidebar/DocsSection.tsx
- src/components/tunaflow/sidebar/ScratchpadSection.tsx
- src/components/tunaflow/sidebar/SidebarContextMenu.tsx
- src/components/tunaflow/sidebar/AddProjectForm.tsx

작업 순서:
1. 각 파일에서 Korean 문자열 grep 으로 식별 (`"[가-힣]`, `\`[^\`]*[가-힣]`)
2. `src/locales/ko/sidebar.json` 에 필요한 키 추가 (기존 section / action / confirm 그룹 확장). SSOT 는 ko.
3. en/sidebar.json 동기화 번역
4. 컴포넌트에 `useTranslation("sidebar")` hook 추가 + `t(...)` 로 교체
5. `npx tsc --noEmit` + `npx vitest run` 통과 확인
6. 커밋 + PR (제목: `feat(i18n): PR A2-D — sidebar 하위 컴포넌트 전환`)

[패턴 정착 (세션 40 에서 검증됨)]
- `import { useTranslation } from "react-i18next"`
- `const { t } = useTranslation("<namespace>")`
- Interpolation: `t('key', { name, error, ... })`
- 조건부: `t(cond ? 'a' : 'b')`
- 동적: `t(\`section.\${id}\`)`

[하지 말 것]
- 에이전트 프롬프트 / 페르소나 / 도구 description 영어 고정 — 건드리지 말 것 (A3 scope)
- insightOrchestration.ts 는 PR B 범위 — 이번 세션 무관
- Rust AppError 의 catch 블록 전수 전환은 점진 — 본 세션은 sidebar 하위 UI 에만 집중
- workflow / branch / insight namespace 는 A2-E/F/G 범위 — 이번 세션 무관

[완료 후 다음]
- A2-D 머지 → A2-E (workflow namespace) 착수 또는 A3 (Rust 페르소나) 중 선택
- A2 전체 완료 후 → A3 → Phase 5 (repo public + v0.1.0-beta)
```

---

## 참고 — 세션 40 에서 검증된 작업 리듬

1. 플랜 파일 **먼저 읽기** (특히 Invariants)
2. 현재 코드 상태 grep 으로 정량 파악 (예: Korean 문자열 개수)
3. 최소 타겟 scope 정의 → 과욕 금지
4. JSON SSOT (ko) 먼저 채운 뒤 en 번역 → 타입 체크 자동 검증
5. 컴포넌트 1개씩 마이그레이션 → `tsc --noEmit` 점진 확인
6. 테스트 전체 통과 확인 후 커밋
7. PR 머지 순차 (동시 다발 금지 — 사용자 확인 사이클)
