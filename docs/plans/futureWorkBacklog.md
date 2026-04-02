# Future Work Backlog

> 대화 중 발견된 개선 사항 중 현재 우선순위가 아닌 것들을 기록.
> 컨텍스트 유실 방지용. 워크플로우 안정화 후 착수.
> 최종 갱신: 2026-04-03 (세션 7 반영)

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
- integration test 부재 (Rust 83 + Frontend 96 unit test만)
- 최소 1개 E2E: Chat → 승격 → Subtask → Approved → Dev → Review → Verdict
- 우선순위: 워크플로우 안정화 후

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
