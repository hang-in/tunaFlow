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

---

## 세션별 상세 이력

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
