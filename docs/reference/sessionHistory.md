---
title: tunaFlow 세션 이력
updated_at: 2026-04-12
description: 세션별 전체 작업 이력. 새 세션 시작 시 또는 과거 결정 맥락 필요 시 참조.
---

# tunaFlow 세션 이력

> CLAUDE.md §5·§10 에서 분리. 매 요청 자동 로드 대상 아님.

---

## 세션별 핵심 성과 요약

| 세션 | 날짜 | 핵심 성과 |
|------|------|-----------|
| 1 | 2026-03-28~29 | Linear UI 리팩토링, 4-engine parity Wave 1+2, 드로어/Branch/RT 통합, Skills UI, Agent Profile/Persona, Artifacts 워크플로, Settings, rawq sidecar, 문서 IA 거버넌스 |
| 2 | 2026-03-30 | ContextPack 전체 파이프라인 (visibility/compression/budget/identity/memory), context-hub 연동, runtimeSlice/SettingsPanel 리팩토링, 108 tests |
| 3 | 2026-03-30 | Claude parity fix (unified `build_normalized_prompt_with_budget()`), auto mode +1 bias 수정, compression DB lock 3-phase 분리, agents.rs 1168→260줄 |
| 4 | 2026-03-31 | Multi-agent context 3-layer, retrieval 품질 튜닝, Gemini auto/fnm/nvm, streaming UX 정리, project scaffold, deps Phase 1-4.2, rawq fs watcher |
| 5 | 2026-04-01 | 오케스트레이션 워크플로 파이프라인 Phase A-E 전체 완료 (DB v18, 마커 파서 4종, Approval Gate, Test Runner, Review RT, Verdict, Rework 루프) |
| 6 | 2026-04-02 | zod 스키마 검증 인프라, OpenAI Compatible 엔진 (Ollama), Tool Steps 가시화, silent error 표면화, Developer/Reviewer 프롬프트 수정 |
| 7 | 2026-04-02~03 | 장기기억 4단계, Vector DB, virtuoso/cmdk, tokio async, rawq 고도화, 워크플로우 스킬/doom loop/가독성, 코드 리팩토링 Tier1, 실사용 검증 50+ 버그 수정 |
| 8-9 | 2026-04-03~04 | 이벤트 격리, RT 전면 수정, 스트리밍 race condition 근본 해결, Virtuoso re-render, 메시지 duration/token 표시, trace_log JOIN (v23), SQLite PRAGMA, ollama 엔진 추가 |
| 10 | 2026-04-04 | Trace Phase 1, 스킬 4-layer + 레지스트리, CRG 통합, Architect/Developer/Reviewer 고도화, 전역 profileId 제거, 마커 기반 도구 호출, DB v25 |
| 11 | 2026-04-04 | 전수조사→문서 정합성 복구, expect 패닉 제거, 스트리밍 중복 150줄 제거, useMemo, 경고 0 |
| 12 | 2026-04-05 | 테스트 180→352, 3-role 프롬프트 근본 수정, 에스컬레이션 경로 완성, 스마트 scaffold, DB v26, UI 수정 20+건 |
| 13 | 2026-04-05~06 | Review 자동 감지, doom loop 안정화, 크로스 프로젝트 격리, 코드 품질 감사 7항목, Plan UX |
| 14 | 2026-04-06~07 | Failure Learning (DB v27-28), Artifacts Plan 그룹핑, Insight 탭 설계, 알림 시스템, 채팅 UI 대폭 개선, CI macOS 전환 |
| 15 | 2026-04-07~08 | Insight 탭 구현 (Phase A~G, DB v29), 디자인 시스템 Phase 1, Codex tool steps 수정, 타이틀바 드래그, README |
| 16 | 2026-04-10 | RT 중간 스트리밍 + ContextPack Tiering Tier 0+1 (RT ~70% 절감) + PTY Phase 1-2 + MCP 서버 + JSONL 응답 수집 |
| 17 | 2026-04-11 | PTY Phase 3-5 + 잔여 항목 (delta 주입, Codex/Gemini resume, ToolSteps 고도화, TerminalPanel→StatusBar) |
| 18 | 2026-04-11 | ContextPack Tiering 8항목 완료, sqlite-vec 18x, P0 Structured Memory, WIP Limits, HTTP API Phase 1, DB v30 |
| 19 | 2026-04-11 | HTTP API E2E 테스트 + Phase 2 (16개 엔드포인트), 코덱스 리뷰 대응, 장기기억 품질 테스트, DOOM 이스터에그 (WIP) |
| 20 | 2026-04-11 | 문서 RAG, 장기기억 자동 트리거, write lock 5건, 검색 품질 문제(bge-m3 필요), PTY 안정화 필요 |
| 21 | 2026-04-11 | (메모리 기록 참조) |
| 22 | 2026-04-12 | CPU 수정(bge-m3 증분), PTY 터미널 표시 수정, 사이드바 리사이즈 재설계 (5섹션 분리), ArtifactsPanel/ReviewPanel 마스터-디테일 전환, InsightPanel 재검토+Architect검토+summary strip, -p 모드 resume_token 제거 (PTY 충돌 수정), CLAUDE.md 경량화 |
| 25 | 2026-04-12 | 버그 9건 수정 + UI 개선 4건 (MarkdownComponents h-tags/empty span/relative link, PTY persona+duration+label, PlanProposalCard reload, branch stale closure, Insight 이전분석 우측 패널, adopt 중 스트리밍 보존+결과 복원, 드로어 라운딩, 사이드바 가독성, 알림배지 오버랩) |
| 26 | 2026-04-13 | 코드베이스 리팩토링 v3 Tier 1(부분)+Tier 2 — conversation_memory 3분리, vector_search 4분리, workflowOrchestration 5분리, InsightPanel 4분리, ptyTypes 추출. Rust/TS 테스트 전원 통과. |
| 35 | 2026-04-13 | 구조개선 Sprint 2~3 (planWorkflowService 도메인 규칙, threadRtRunner 분리, silent catch 7건), PTY Enter 3중 수정, bge-m3 CPU 스파이크 수정(ONNX 스레드 제한+세마포어+점진적 인덱싱). 232 Rust + 188 TS tests. |

---

## 세션별 상세 이력

### ✅ 세션 25 (2026-04-12): 버그 수정 + UI 가독성

**버그 수정 (9건)**
- `MarkdownComponents`: h1/h2/h3/h4 커스텀 컴포넌트 추가 (Tailwind reset 대응), 빈 InlineCode 억제 (`!text.trim() → null`), SafeLink 상대 경로 → `FileViewerContext.openFile()`
- PTY 페르소나 미저장: `personaLabel` → `append_assistant_message` Rust command, `persona` 컬럼 저장
- PTY duration 0 표시: `list_messages` SQL → `COALESCE(t.duration_ms, m.duration_ms)`
- `"pty-streaming"` 레이블 제거: `MessageMeta` 스트리밍 표시 단순화
- `PlanProposalCard` 리로드 후 승격 버튼 재표시: 전체 plans 배열에서 title 매칭 (done/abandoned 포함)
- Branch stale closure: `ChatPanel` `handleCreateBranchRef` ref 패턴으로 수정
- Insight "이전 분석" 클릭 시 메인 뷰 교체 → 우측 패널 미리보기로 변경
- `adoptBranch` 중 스트리밍 메시지 소멸: DB reload 시 in-memory streaming 메시지 보존 (merge)
- PTY 완료 후 결과 미표시: `asstMsgId` store에서 제거된 경우 `list_messages` fallback reload

**UI 개선 (4건)**
- 드로어 고정 시 `rounded-l-xl`, 플로팅 시 `rounded-l-xl right-0` (오른쪽 엣지 딱 붙음)
- 사이드바 가독성: 섹션 헤더 `/40→/55`, 비활성 항목 `/60→/68`, 활성 항목 primary accent bar (`border-l-2 border-primary bg-primary/12`)
- 알림 배지: `-top-1 -right-1 border-2 border-background`로 아이콘 오버랩

**수정 파일**
- `src/components/tunaflow/chat/MarkdownComponents.tsx`
- `src/components/tunaflow/message/MessageMeta.tsx`
- `src/components/tunaflow/chat/PlanProposalCard.tsx`
- `src/components/tunaflow/ChatPanel.tsx`
- `src/components/tunaflow/context-panel/InsightPanel.tsx`
- `src/components/tunaflow/AppShell.tsx`
- `src/components/tunaflow/Sidebar.tsx`
- `src/components/tunaflow/sidebar/TreeRow.tsx`
- `src/components/tunaflow/NotificationBell.tsx`
- `src/stores/slices/branchSlice.ts`
- `src/stores/slices/ptyMessageSender.ts`
- `src/stores/slices/runtimeSlice.ts` / `threadSlice.ts`
- `src-tauri/src/commands/messages.rs`

---

### ✅ 세션 26 (2026-04-13): 코드베이스 리팩토링 v3 Tier 1(부분) + Tier 2

**리팩토링 완료 항목 (5개 god-file → 18개 모듈)**

| 원본 | 결과 | 줄 수 |
|------|------|-------|
| `conversation_memory.rs` (984줄) | `memory_topics.rs` + `memory_compression.rs` + 기존 파일 축소 | Tier 1 2.4 ✅ |
| `vector_search.rs` (907줄) | `vector_search/` 모듈: mod.rs + helpers.rs + index.rs + query.rs | Tier 1 2.5 ✅ |
| `ptyMessageSender.ts` | `ptyTypes.ts` 분리 (PtySendOptions, getPtyPollConfig 추출) | Tier 1.5 ✅ |
| `workflowOrchestration.ts` (701줄) | `lib/workflow/`: index.ts + helpers.ts + reportSync.ts + implementWorkflow.ts + reviewWorkflow.ts | Tier 2 2.6 ✅ |
| `InsightPanel.tsx` (726줄) | `context-panel/insight/`: insightConstants.tsx + InsightFindingCards.tsx + InsightQuadrant.tsx + InsightPanel.tsx(thin) | Tier 2 2.8 ✅ |

**부가 수정**
- `planProposalParser.test.ts`: unclosed marker 처리 변경(이전 세션 행동 변경)에 맞게 테스트 기대값 수정 (pre-existing bug)
- Rust `commands/mod.rs`: `pub mod memory_topics; pub mod memory_compression;` 추가
- 분할 전후 전체 검증: `cargo check` + `cargo test --lib` (230 tests) + `npx tsc --noEmit` (0 errors) + `npx vitest run` (176 tests)

**미완료 — Tier 1 잔여 (s27에서 완료)**
- `http_api.rs` → http_api/ 모듈 ✅ s27
- `pty.rs` → commands/pty/ 모듈 ✅ s27
- `executor.rs` → sequential/deliberative 분리 ✅ s27
- `threadSlice.ts` (609줄) → `branchSync.ts` 분리 + `agentStreamHelper.ts` 활용으로 481줄 ✅ s27

---

### ✅ 세션 27 (2026-04-13): 리팩토링 v3 완료 + P1 기능 3종

**리팩토링 v3 Tier 1 잔여 완료**

| 원본 | 결과 |
|------|------|
| `http_api.rs` (1,162줄) | `http_api/` 모듈 분리 ✅ |
| `pty.rs` (1,076줄) | `commands/pty/` 모듈 분리 ✅ |
| `executor.rs` (968줄) | sequential/deliberative 분리 ✅ |
| `threadSlice.ts` (609줄) | `branchSync.ts` + `agentStreamHelper.ts` → 481줄 ✅ |

**P1 기능 완료**
- **RT 전용 페르소나 행동 지침**: `executor.rs`에 `role_guidance()` 추가 — proposer/reviewer/verifier/synthesizer 4종 지침, synthesizer max_tokens 1500→2000
- **Insight Phase H** (auto-export): `insightOrchestration.ts` — 세션 완료 후 `exportInsightToFiles()` 자동 호출
- **Insight Phase J** (plan done → findings resolved): `reviewWorkflow.ts` — review pass 시 `resolveInsightFindingsByPlan()` 자동 호출
- **디자인 시스템 Phase 3**: `ideaImplementationStatus.md` 53개 idea 문서 현황 정리

**수정 파일**
- `src-tauri/src/commands/roundtable_helpers/executor.rs`
- `src/lib/insightOrchestration.ts`
- `src/lib/workflow/reviewWorkflow.ts`
- `src/lib/workflow/branchSync.ts` (신규)
- `src/stores/slices/threadSlice.ts`
- `docs/reference/ideaImplementationStatus.md` (신규)

**테스트**: Rust 230 ✅, Frontend 176 ✅

---

### ✅ 세션 28 (2026-04-13): 사이드바 폰트 크기 수정

**완료**
- 사이드바 모든 섹션(Branches/Roundtables/Scratchpad/Docs/Archive) 아이템 폰트 정상화
  - `text-tf-sm` → `text-[11px]`, `text-tf-xs` → `text-[10px]`, `text-tf-micro` → `text-[9px]`
  - **원인**: Tailwind 4 JIT가 `cn()` 조건부 분기 내 custom `--text-tf-*` 토큰을 dev 모드에서 감지하지 못해 16px(브라우저 기본값)로 렌더링
  - **해결**: arbitrary value(`text-[11px]`)로 교체 → JIT가 항상 즉시 감지

**수정 파일**
- `src/components/tunaflow/Sidebar.tsx` — Branch/RT/Archive 아이템 + 섹션 헤더 + 카운트 배지
- `src/components/tunaflow/sidebar/ScratchpadSection.tsx` — 아이템 + 헤더
- `src/components/tunaflow/sidebar/TreeRow.tsx` — TreeRow label + SectionHeader title
- `src/components/tunaflow/sidebar/DocsSection.tsx` — 파일 아이템

**미완료 (다음 P1)**
- 리팩토링 v3 잔여: `http_api.rs`, `pty.rs`, `executor.rs` 모듈화 (s27 커밋 내 포함여부 확인 필요)
- ContextPack DB/assembly 완전 분리
- 브랜치 label git slug화
- Insight Phase I: `tool-request:insight` 핸들러 — `src/lib/toolRequestHandler.ts`에 insight 케이스 추가 시작점
- 디자인 시스템 Phase 2: prose-* 토큰 확대 적용

**사이드이펙트 경고**
- `TreeRow.tsx` label 폰트 변경 → `ChatsSection.tsx`의 conversation/branch 트리 항목에도 적용됨 (시각적 확인 필요)
- `DocsSection.tsx`는 이전에도 같은 크기였으나 arbitrary value로 통일됨 (동작 변화 없음)

**테스트**: Rust 230 ✅, Frontend 176 ✅. DB v30 변화 없음.

---

### ✅ 세션 1-2 (2026-03-28~30)
- 드로어 RT 기능, 사이드바 계층 구조, Linear UI 리팩토링, rawq 안정화, 프로젝트 soft-delete, Agent Profile/Persona, Branch/RT 고도화, Artifacts 워크플로, Settings, 문서 IA 거버넌스
- 4-engine context metadata parity, ContextPack visibility, rawq 후처리, Compression, Context budget control UI
- context-hub 연동, Agent identity framing, Message author attribution, Compressed conversation memory
- runtimeSlice 팩토리, SettingsPanel 분할, deprecated isRunning 제거, OpenCode discovery

### ✅ 세션 3 (2026-03-30): Claude parity + dead code
- Claude parity fix → unified `build_normalized_prompt_with_budget()` 전환
- Auto mode +1 bias 수정 (persona_fragment → explicit persona check)
- Lite mode retrieval/compressed thresholds 완화
- Compression DB lock 분리 (3-phase)
- Trace surface mode 포맷 호환 (`baseMode()` 헬퍼)
- agents.rs 1168→260줄 (레거시 6개 삭제 + prepare/finalize 공유 추출)
- branchSlice ENGINE_CONFIGS 통합

### ✅ 세션 4 (2026-03-31): multi-agent context + quality + deps
- **Multi-agent context 3-layer**: participants meta + budget-based dynamic window + per-agent last-message guarantee
- **Retrieval 품질 튜닝**: FTS5 stopwords, scoring rebalance, overlap penalty 상향, adaptive limit
- **Compressed memory 참여자 보존**: SUMMARY_PROMPT에 `## Participants` 섹션 필수화
- **Gemini Auto model**: discovery에 `auto` 기본 추가
- **fnm/nvm 바이너리 경로**: Gemini + Codex resolve에 fnm/nvm 탐색 추가
- **프로젝트 scaffolding**: 프로젝트 생성 시 CLAUDE.md + docs/ 자동 생성
- **plan-first 규칙**: ContextPack identity block + CLAUDE.md 양쪽에 "승인 전 구현 금지" 규칙
- **rawq fs watcher**: tauri-plugin-fs watch + 에이전트 완료 시 re-index
- **의존성 도입 Phase 1-4.2**: clipboard-manager, shell, opener, fs (Tauri) + chrono, tokio (Rust) + react-virtuoso, cmdk, sonner (npm)

### ✅ 세션 5 (2026-04-01): 오케스트레이션 워크플로우 파이프라인
- DB v18 migration (plans 확장 + plan_events + plan 6개 컬럼)
- `<!-- tunaflow:plan-proposal -->` 마커 파서 + PlanProposalCard (승격/수정요청/무시 3버튼)
- ApprovalGate (승인→Implementation Branch / 검토→Review Branch / 보류)
- ImplPlanCard (`<!-- tunaflow:impl-plan -->` 마커), Review RT 자동 실행
- ReviewVerdictCard (`<!-- tunaflow:review-verdict -->` 마커), Rework 루프
- Rust 60 unit tests, Frontend 66 tests

### ✅ 세션 6 (2026-04-02): 내부 품질 + 엔진 확장 + tool 가시화
- zod 스키마 검증 인프라 (`src/lib/schemas/` 5개 워크플로우 스키마 SSOT)
- OpenAI Compatible 엔진 (Ollama): `openai_compat.rs` — SSE 스트리밍 + tool call + auto fallback
- Tool Steps 가시화: `__STEP__:{json}` 프로토콜, `toolStepsStore.ts`, `ToolStepsView.tsx`
- Silent error 표면화: 12개 파일 catch → console.error + toast.error
- Developer/Reviewer 프롬프트 수정 (마커 파일 쓰기 금지, 검증 범위 제한)
- Rust 60 tests, Frontend 96 tests

### ✅ 세션 7 (2026-04-02~03): 장기기억 Phase 1-4 + 품질
- **주제별 메모리**: JSON 배열 출력, 토픽별 다중 행 저장
- **자동 세션 발견**: session_links 테이블 (DB v21), FTS5 기반
- **Vector DB**: conversation_chunks (DB v22), rawq embed CLI, brute-force cosine + FTS5 하이브리드
- **rawq 고도화**: SearchOptions + search_with_options, prompt_needs_rawq 완화
- **Doom Loop 감지**: review_failed 3회 → subtask_review 에스컬레이션
- **채팅 가독성**: ASCII 박스→마크다운 (8개 프롬프트)
- **워크플로우 스킬 자동 주입**: phase→스킬 매핑
- 실사용 6+ 풀사이클 검증, 15+ 버그 수정, rawq 94초→500ms

### ✅ 세션 8-9 (2026-04-03~04): 이벤트 격리 + RT + 스트리밍
- ChunkPayload conversationId 추가, isStillActive() 가드
- flushChunk race condition 근본 해결 (pendingChunk=null before cleanup)
- RT: async panic 수정, 라운드 번호 이중 가산 제거, ContextPack 주입+캐싱 (N→1-2회)
- RT participant status store 이동 + conversationId 스코핑
- Virtuoso re-render (messagesRef → context prop)
- 메시지 duration/token 표시, trace_log JOIN (DB v23), SQLite PRAGMA
- ollama 엔진 5곳 하드코딩에 추가

### ✅ 세션 10 (2026-04-04): 스킬 고도화 + 에이전트 고도화
- 스킬 4-layer (A/B/C/D) + 멀티툴 스캔 (chops 포팅) + skills.sh 레지스트리 + 프로젝트 스킬팩
- code-review-graph 통합 (CLI query/impact + Rust sidecar + ContextPack + auto update)
- Architect/Developer/Reviewer 역할 템플릿 전면 갱신 (PLATFORM_TIER0)
- 전역 selectedProfileId 제거 (_convEngineMap만 SSOT)
- 마커 기반 멀티턴 도구 호출 (docs/rawq/graph/plans 4종)
- 후속 플랜 인프라 (DB v25), context-hub chub 수정
- Rust 84 tests, Frontend 96 tests

### ✅ 세션 11 (2026-04-04): 정합성 복구 + 경고 제거
- 전수조사 → 문서 정합성 복구
- expect 패닉 제거, 스트리밍 중복 150줄 제거
- useMemo 적용, 경고 0 달성

### ✅ 세션 12 (2026-04-05): 테스트 + 프롬프트 근본 수정 + UI
- 테스트 180→352 (P0 스트리밍/ContextPack/RT + P1 워크플로우/장기기억 + P2 UI)
- CLI resolve 6중 복제 통합 (`agents/resolve.rs`)
- **3-role 프롬프트 전면 수정**: Architect(검증 명령 필수) + Developer(검증 결과 보고) + Reviewer(코드 읽기만)
- 에스컬레이션 경로 완성 (doom loop → Architect 재설계 → 자동 병합)
- 스마트 scaffold (프로젝트 스택 자동 감지 → CLAUDE.md §1 자동 채움)
- microcompact, 커스텀 타이틀바, 우클릭 컨텍스트 메뉴
- DB v26 slug, UI 수정 20+건

### ✅ 세션 13 (2026-04-05~06): 워크플로우 안정화 + 품질 감사
- Review verdict 자동 감지 (autoDetectReviewVerdict)
- Doom loop 안정화 (카운터 리셋 4곳, conditional→review_conditional 분리, 중복 호출 방지)
- 크로스 프로젝트 격리 (isActiveThread() 가드 5곳)
- 코드 품질 감사 7항목: CSP, 빈 catch 35개, non-null 11개, parking_lot, CJK 토큰, AppError JSON, 커버리지
- Plan UX: 우클릭 컨텍스트 메뉴, status 배지, All 스테이지 탭
- Rust 185 tests, Frontend 175 tests

### ✅ 세션 14 (2026-04-06~07): Failure Learning + Artifacts + Insight 설계 + UI
- **Failure Learning 시스템**: failure_lessons 테이블 (DB v27) + FTS5 + rework 자동 주입 + resolution 자동 채움
- **Artifacts Plan 그룹핑**: artifacts.plan_id 컬럼 (DB v28), PlanGroup 컴포넌트
- **워크플로우 artifact 자동 생성**: architect-decision, test-report, review-findings
- **Insight 탭 설계**: `docs/ideas/insightTabDesign.md` — 카테고리 기반, SQALE+Quadrant
- **알림 시스템**: NotificationBell + Web Audio + on/off (appStore 영속)
- **채팅 UI 대폭 개선**: Pretendard 3-tier 폰트, max-w-4xl 중앙 정렬, 아바타 인라인, 드로워 pin
- impl-complete DB fallback + orphan 자동 복구
- ReviewPanel 구조화 (VerdictCard/DecisionCard + 심각도 정렬 + 모달)
- RuntimeStatusBar: 시간당 비용, 컨텍스트 % 아이콘, Git branch+dirty
- CI macOS 전환 (Node 22, actions v5)
- Rust 188 tests, Frontend 175 tests. DB v28.

### ✅ 세션 15 (2026-04-07~08): Insight 탭 구현 + 디자인 시스템
- **Insight 탭 구현 (Phase A~G)**: DB v29 (insight_sessions/findings/reports), 사전 추출 파이프라인, master-detail UI, Auto Fix 파이프라인, 토큰 추적
- **디자인 시스템 Phase 1**: CSS 토큰 (--text-tf-micro~xl 7단계, --prose-strong~disabled 5단계), reduced motion
- Codex tool steps 수정 (CLI prefix 제거 + SDK __STEP__ 형식)
- 타이틀바 드래그 capability, RT 메시지 헤더 통일, 고아 프로세스 방지
- README: 설계 근거/프로젝트 계보/오케스트레이션 분석
- Rust 188 tests, Frontend 175 tests. DB v29.

### ✅ 세션 16 (2026-04-10): RT 스트리밍 + Tiering + PTY + MCP
- RT 중간 스트리밍
- ContextPack Tiering Tier 0+1 (RT ~70% 절감)
- PTY Phase 1-2
- MCP 서버 연동
- JSONL 응답 수집

### ✅ 세션 17 (2026-04-11): PTY 고도화
- PTY Phase 3-5 (delta 주입, Codex/Gemini resume, ToolSteps 스트리밍)
- TerminalPanel → StatusBar 아이콘 토글

### ✅ 세션 18 (2026-04-11): Tiering 완료 + HTTP API
- ContextPack Tiering 8항목 완료 (chunk 품질, sqlite-vec 18x, RT 벡터 맥락 공유 ~80%)
- Tier 2 9종, 메인 채팅 Tiering, RT resume_token
- P0 Structured Memory (budget weight), WIP Limits 경고, Fresh Session Rework
- Branch PTY 공유
- HTTP API Phase 1 (axum REST+WS+Bearer), DB v30

### ✅ 세션 19 (2026-04-11): HTTP API Phase 2 + 코덱스 리뷰
- HTTP API E2E 테스트 (전 엔드포인트 검증 + Snake Game 풀 워크플로우)
- HTTP API Phase 2 (Branch/RT/Memory/Search 16개 엔드포인트, ContextPack 주입)
- 코덱스 리뷰 대응 (토큰 uuid, 문서 SSOT, async mutex 패턴, agentStreamHelper, 컴포넌트 분할, 테스트 19개)
- 장기기억 품질 테스트 (크로스세션 recall 한계 발견)
- DOOM 이스터에그 (WIP)

### ✅ 세션 20 (2026-04-11): 문서 RAG + 자동 트리거
- 문서 RAG (docs/ md DB 인덱싱 + 그래프 RAG)
- 장기기억 자동 트리거 배선
- write lock 5건 수정
- 검색 품질 문제 발견 (bge-m3 필요)

### ✅ 세션 22 (2026-04-12): PTY 버그 수정 + UI 개선 + CLAUDE.md 경량화
- PTY -p 모드 resume_token 충돌 수정 (Claude "No response requested." 버그)
  - -p 스트리밍 경로에서 resume_token 제거 (ContextPack이 맥락 포함)
  - PTY spawn 대기 로직 추가 (2초 폴링, `isPtySpawning()` export)
- Sidebar 5섹션 분리 (Branches/Roundtables/Scratchpad/Docs/Archive) + `adjustTwoHeights` 패턴
- ArtifactsPanel 마스터-디테일 전환 (좌우 분할, ReactMarkdown 렌더링)
- ReviewPanel 마스터-디테일 전환 (좌우 분할, 인라인 상세 패널)
- InsightPanel: 재검토(revalidateFindings) + Architect 검토(handleSendToArchitect) + summary strip
- Auto Fix → 메타에이전트 도입 후 구현으로 연기, 문서 업데이트
- CLAUDE.md 경량화 (53KB → sessionHistory.md 분리)

### ✅ 세션 39 (2026-04-23): SDK hot fix + Codex review 루프 + designReviewGate 제안

**SDK 30s timeout hot fix (Session Continuity Fix followup)**
- 증상: Branch Dev → Reviewer → Dev 진입 시 `sdk-session: claude did not connect within 30s`
- 가설 4건 중 #2 확정 — claude CLI 2.1.117 의 `--session-id <uuid>` / `--resume <sid>` 인자 상호배타
- PR #135 (stderr capture + `TUNAFLOW_DISABLE_RESUME_BOOTSTRAP` escape hatch, TEMP)
- PR #137 (hot fix): resume 있음 → `--session-id` 생략 / 없음 → `--session-id` 만
- 영향 범위: `claude_sdk_session::spawn_session` 한 곳. RT (`-p` one-shot) 경로 미접촉
- stderr/escape hatch 는 근본 수정 확정 후 제거 PR 대기

**Session Continuity Fix 머지 완료**
- PR #130 (Architect followup prompt), #131 (Plan + 3 subtasks), #134 (task-03 auto-invalidate)
- INV-1~7 전원 해소. `current_session_key` RESUME_IDS 우선 + `promote_pending_to_delivered` live 우선 + bootstrap + auto-invalidate
- Rust 403 tests 통과

**Architect 산출물 수용**
- designReviewGatePlan + 3 subtasks (P1): Plan 승인 시 Architect↔Codex RT 경로
- roleAssignmentCoverageUxPlan + 2 subtasks (P2): Settings 역할 커버리지 UX
- userWorldviewInjectionPlan + 4 subtasks (P1): Identity/Interface/Continuity 3축
- searchPipelineFromSecallPlan-part2 + 5 subtasks: Codex 3-round 리뷰 반영 정제
- PR #136: userWorldviewInjectionPlan scope 를 sdk-session(Branch) 한정으로 명시
- PR #138: userWorldviewInjectionPlan round-1 리뷰 반영 (BLOCKER 3 + MAJOR 3 + MINOR 1 resolved)

**orphan 프로세스 정리**
- 며칠에 걸쳐 누적된 codex app-server/exec 40+건 TERM 정리

**다음**
- SDK hot fix 사용자 재현 검증 → stderr 로그 확인
- userWorldview Codex round-2 결과 → pass 시 01→02→03→04 구현 착수
- TEMP 제거 PR (stderr null 원복 + escape hatch 제거)
