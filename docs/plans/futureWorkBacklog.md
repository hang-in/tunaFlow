# Future Work Backlog

> 대화 중 발견된 개선 사항 중 현재 우선순위가 아닌 것들을 기록.
> 컨텍스트 유실 방지용. 워크플로우 안정화 후 착수.
> 최종 갱신: 2026-04-04 (세션 11 반영)

---

## UI/UX

### 에이전트 CLI 권한 승인 UI
- 현재: Claude CLI `--permission-mode acceptEdits`로 편집 자동 승인
- 필요: 에이전트가 터미널 명령 실행 시 앱 내에서 승인/거부 UI 표시
- 참고: `~/.claude/settings.json` permissions으로 명령별 화이트리스트 가능
- 우선순위: 워크플로우 안정화 후

### ~~DevProgressView 실시간 업데이트~~ — ✅ 완료 (5초 폴링)

### Review RT (Roundtable) 다중 리뷰어
- 현재: 2-agent Review RT 구현 (세션 5)
- 필요: 3+ Reviewer 병렬 토론 후 verdict
- RT 자체 안정화 필요 (progress 가시성, 동기 실행 문제)
- 우선순위: 단일 Reviewer 안정화 후

### 스킬 자동 주입
- ~~키워드 매칭 선택적 주입~~ — ✅ 완료 (세션 4b, 8k→3k 절감)
- ~~워크플로우 phase별 자동 스킬 주입~~ — ✅ 완료 (세션 7, appStore workflowSkills 매핑 + effectiveSkills)
- ~~스킬 세트 그룹화 (Toolset Composition)~~ — ✅ 완료 (세션 7, SKILL_SETS + set: 접두사)
- 에이전트 role 기반 자동 스킬 (Persona recommendedSkills) — 미구현
- 온보딩 메타에이전트 자동 추천 — 미구현
- 우선순위: 스킬 선택 UX 개선 시

### ~~ChatPanel 가상 스크롤~~ — ✅ 완료 (세션 7)
- react-virtuoso Virtuoso 컴포넌트로 ChatPanel 전환
- followOutput + scrollToIndex + initialTopMostItemIndex

### ~~커맨드 팔레트~~ — ✅ 완료 (세션 7)
- cmdk Cmd+K 커맨드 팔레트 (탭/대화/프로젝트 전환, 새 대화, 설정)

---

## 안정성

### 에러 경로 처리
- ~~silent error 표면화~~ — ✅ 완료 (세션 6, 12개 파일 toast/console.error)
- 에이전트 무응답 시 타임아웃 + 사용자 알림
- 마커 미감지 시 fallback 경로
- 크래시 복구 메커니즘
- 참고: `docs/ideas/clawTeamAnalysis.md`
- 우선순위: 정상 경로 안정화 후

### Dynamic Budget Allocation (ContextPack)
- guardrail.rs 섹션별 상수 하드코딩 → 동적 배분
- 빈 섹션 예산 반납, 내용 있는 섹션 확장
- 참고: `docs/plans/contextPackAlgorithmImprovementsPlan.md`
- 우선순위: context 부족 체감 시

### E2E Smoke Test
- integration test 부재 (Rust 84 + Frontend 96 unit test만)
- 최소 1개 E2E: Chat → 승격 → Subtask → Approved → Dev → Review → Verdict
- 우선순위: 워크플로우 안정화 후 (상세: 아래 "테스트 보강" 섹션)

### ~~RT 동기 실행 → 비동기 전환~~ — ✅ 완료 (세션 7)
- tokio async 전환 완료 (execute_round, run_participant, spawn_blocking)
- 다음 단계: RT progress 가시성 강화 (deliberative mode 실시간 진행 표시)

### Tool Steps Gemini 호환
- Gemini CLI 버전에 따라 `tool_use` 이벤트 미지원 가능 (tool_result만)
- 우선순위: Gemini SDK 직접 통합 시 해결 예정

---

## 구조

### 헤드 에이전트 기본값 설정
- 채팅/Plan의 기본 에이전트를 Architect로 설정하는 UX
- Settings 또는 프로젝트 설정에서 기본 Architect 에이전트 지정
- 우선순위: 프로필 시스템 안정화 후

### Workflow Skill Tier 1/2
- plan 활성 시 상세 마커 규약 ContextPack 추가 주입
- phase별 추가 규칙 주입
- 우선순위: 스킬 자동 주입과 함께

### Agent Template 자동 로딩 고도화
- 현재: role 기반 자동 감지 (architect/developer/reviewer)
- 필요: 사용자 커스텀 role 지원, 프로젝트별 role 매핑
- 우선순위: 기본 role 감지 안정화 후

### ContextPack DB/Assembly 완전 분리
- `load_context_data()` + `assemble_prompt()` 2-phase 분리 완료 (세션 4)
- DB/assembly 완전 분리 프롬프트 준비됨
- 우선순위: P1

### Gemini SDK 직접 통합
- CLI 대체 → native SSE streaming, token tracking, function calling
- 참고: `docs/plans/geminiSdkIntegrationPlan.md`
- 우선순위: P1

### Function Calling 마커 대체
- HTML comment 마커 → SDK function calling
- 참고: `docs/plans/toolCallHandlerPlan.md`
- 우선순위: P1 (SDK 통합 후)

---

## 테스트 보강 (세션 11 전수조사 기반, 세션 12 대폭 확장)

> 현재 Rust 174 + Frontend 131 = 305 unit test. integration/E2E 0.
> P0/P1 완료. P2 UI 회귀 테스트 잔여.

### ~~P0: 스트리밍/이벤트 흐름 테스트~~ — ✅ 완료 (세션 12, +22 tests)
- **대상**: `src/stores/slices/runtimeSlice.ts`, `src/stores/slices/threadSlice.ts`
- **선행 작업**: Tauri `invoke`/`listen` mock 인프라 구축 (vitest)
- **케이스**:
  - progress → chunk → completed 순서에서 placeholder 정상 교체
  - agent:error 시 cleanup + 상태 복구 (messages, runningThreadIds)
  - conversationId 불일치 이벤트 무시
  - queue drain이 thread별로 정확히 동작
  - pendingChunk null 처리 (flushChunk race condition 방어 검증)
- **이유**: 가장 회귀 위험이 큰 실행 경로. 세션 8-9에서 race condition 수정 이력 있음.

### ~~P0: ContextPack 조립 테스트 (Rust)~~ — ✅ 완료 (세션 12, +26 tests)
- **대상**: `src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs`
- **기존**: 4개 unit test 존재 → 확장
- **케이스**:
  - mode별 섹션 포함/스킵 (Lite: rawq skip, Full: all include)
  - retrieval/compressed threshold 분기
  - auto mode 선택 결과 (메시지 수 기반)
  - section budget breakdown (guardrail 상한 내)
  - author attribution (`[assistant:ProfileName (engine)]`) 보존
  - identity block (profile/engine/persona 3층) 정확성
  - participants meta 섹션 (multi-agent context)
- **이유**: tunaFlow 핵심 가치. mock 불필요 (pure function).

### ~~P0: RT 프롬프트 조립 테스트 (Rust)~~ — ✅ 완료 (세션 12, +27 tests)
- **대상**: `src-tauri/src/commands/roundtable_helpers/` (executor.rs, prompt.rs)
- **케이스**:
  - blind participant가 이전 transcript 없이 prompt를 받는지
  - role-based token directive 삽입 확인
  - sequential에서 prior/current round semantics 유지
  - completion-order 수집 (deliberative mode)
- **이유**: 세션 8-9에서 RT 전면 수정, 회귀 가능성 높음. pure function 부분만 unit test.

### ~~P1: 워크플로우 오케스트레이션 테스트 (Frontend)~~ — ✅ 완료 (세션 12, +11 tests)
- **대상**: `src/lib/workflowOrchestration.ts`
- **기존**: 170줄 테스트 존재 → 확장
- **케이스**:
  - plan proposal → approval → implementation branch 전이
  - impl-complete → review RT → verdict 전이
  - rework loop: verdict fail → rework phase → re-impl
  - doom loop escalation: review_failed 3회 → subtask_review
  - marker strip (tunaflow 마커 제거 검증)
  - result report 생성 (rework 후 마지막 메시지만 사용)
- **이유**: 기능은 크지만 현재 보호막이 약함.

### ~~P1: 장기기억/검색 테스트 (Rust)~~ — ✅ 완료 (세션 12, +32 tests)
- **대상**: `conversation_memory.rs`, `session_discovery.rs`, `vector_search.rs`
- **케이스**:
  - compressed memory 생성 조건 (12+ 메시지 threshold)
  - topic JSON 파싱 + graceful fallback
  - FTS5 query builder (stopwords 필터, 한국어 처리)
  - vector cosine 검색 (이미 3개 테스트, 추가: 빈 결과/다차원)
  - session_links auto/pinned 정합성
- **이유**: 세션 7에서 대규모 변경, 순수 로직 부분은 DB 없이 테스트 가능.

### P2: UI 회귀 테스트 (통합 테스트 안정화 후)
- **대상**: `RoundtableView.tsx`, `RuntimeStatusBar.tsx`, `TracePanel.tsx`
- **케이스**:
  - auto mode 표시 포맷
  - active/skipped section pills 렌더링
  - memory 상태 badge
  - RT role/blind badge 렌더링
- **선행 조건**: React Testing Library 셋업 + 컴포넌트 렌더 mock
- **우선순위**: P0/P1 테스트 안정화 후

---

## 리팩토링

> 세션 12에서 테스트 커버리지 확보 (305 tests) 후 진행 시작.

### ~~CLI 바이너리 해석 6중 복제 통합~~ — ✅ 완료 (세션 12)
- `agents/resolve.rs` 공용 모듈: `NpmCliConfig` + `resolve_npm_cli()` + `first_existing()` + `build_command()`
- codex 70→7줄, gemini 73→7줄, opencode 48→25줄 (총 ~190줄 삭제)
- rawq/context_hub는 특수 로직 유지 (sidecar, dual binary name)

### ~~TracePanel 분할~~ — ✅ 완료 (세션 12)
- `TraceSpanCard.tsx` 추출 (스팬 카드 + 포매팅 유틸리티 + ContextUsageBar)
- TracePanel 656→400줄

### 대형 컴포넌트 추가 분할 (세션 12 후반 진행 중)
- **DevProgressView.tsx** (519줄): `useSubtaskProgress` hook 추출 + ReworkNoticePanel 분리
- **SkillsPanel.tsx** (516줄): `useSkillFiltering` hook 추출 + SkillVendorGroup 분리
- **CenterPanel.tsx** (408줄): MemoPopover 추출 + useTabNavigation hook 분리
- 참고: 세션 11에서 BranchThreadPanel은 PlanRevisionActions 추출 + useMemo 완료
