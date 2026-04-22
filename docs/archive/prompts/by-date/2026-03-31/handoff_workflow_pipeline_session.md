# Handoff: Workflow Pipeline 구현 상태 (2026-03-31)

> 검토/설계용 Opus 세션에서 개발용 Opus 세션으로 작업 전달

---

## 이번 세션에서 완료된 작업

### 1. ContextPack DB/Assembly 분리 리팩토링
- `send_common.rs`에서 `build_normalized_prompt_with_budget()` (540줄)을 Phase A/B로 분리
- `ContextData` 구조체 + `load_context_data()` (DB 의존) + `assemble_prompt()` (순수 함수)
- `prepare_engine_run()`의 DB lock 범위를 `load_context_data()` 호출까지로 축소
- 단위 테스트 4개 추가 (auto mode, section 포함/제외, budget)
- **파일**: `src-tauri/src/commands/agents_helpers/send_common.rs`

### 2. Workflow Pipeline 설계 문서
- `docs/plans/orchestratedWorkflowPipelinePlan.md` — Chat→Plan→Implement→Review 전체 설계
- Stavros 워크플로우 기반, 4개 마커 규약 (plan-proposal, impl-plan, impl-complete, review-verdict)
- 5-Phase 구현 계획 (A: DB, B: Chat→Plan, C: Approval, D: Implementation, E: Review)

### 3. Workflow Agent Templates (신규)
- `scaffold_project_dir()`에 `ensure_workflow_templates()` 추가
- 새 프로젝트/기존 프로젝트 모두에 `docs/agents/{architect,developer,reviewer}.md` 자동 생성
- 각 템플릿에 마커 형식, 역할 가이드, 행동 규칙 포함
- **파일**: `src-tauri/src/commands/projects.rs`

### 4. 기존 프로젝트 마이그레이션
- `ensure_project_workflow_templates` Tauri command 추가
- `projectSlice.selectProject()` 에서 fire-and-forget으로 호출
- **파일**: `src-tauri/src/commands/projects.rs`, `src-tauri/src/lib.rs`, `src/stores/slices/projectSlice.ts`

### 5. ContextPack Tier 0 플랫폼 프롬프트
- `assemble_prompt()`에 `PLATFORM_TIER0` 상수 항상 주입 (~200자)
- "tunaFlow 플랫폼 + plan-proposal 마커 사용 가능 + docs/agents/ 참조" 안내
- **파일**: `src-tauri/src/commands/agents_helpers/send_common.rs`

### 6. Implementation Branch "계획 수정 요청" 기능
- `requestPlanRevision()` — Branch 대화를 압축하여 Architect에게 전달
- `PlanRevisionButton` 컴포넌트 — 에이전트 선택 UI 포함
- 메인 채팅에서 Architect가 수정된 plan-proposal 생성 → PlanProposalCard로 병합
- **파일**: `src/lib/workflowOrchestration.ts`, `src/components/tunaflow/context-panel/PlansPanel.tsx`

### 7. User 메시지 마크다운 렌더링 수정
- `MessageItem.tsx`에서 user 메시지도 마크다운 신호 감지 시 `MarkdownBody`로 렌더링
- `hasMarkdownSignal()` 헬퍼 — 100자 이상 + 마크다운 패턴(#, ```, -, <!-- tunaflow: 등) 감지
- **파일**: `src/components/tunaflow/MessageItem.tsx`

---

## 검증 상태

- Rust: 60 tests passed
- Frontend: 66 tests passed (11 files)
- TypeScript: no errors
- `tauri dev` 로그에서 `platform` 섹션 주입 확인됨

---

## 미완료 / 이어서 해야 할 작업

### 즉시 확인 필요
1. **마크다운 렌더링 실제 확인**: `tauri dev`에서 Implementation Branch 첫 메시지가 마크다운으로 렌더링되는지 시각적 확인
2. **계획 수정 요청 테스트**: Implementation Branch에서 "계획 수정 요청" 버튼 → Architect 에이전트 선택 → 메인 채팅에 수정 요청 전달 → plan-proposal 응답 확인

### 헤드 에이전트 원칙 (구현 필요)
- **채팅(메인)의 기본 에이전트 = Architect** (사용자가 별도 지정하지 않는 한)
- **Plan 탭의 기본 에이전트 = Architect**
- 현재는 사용자가 수동 선택해야 함 → Settings 또는 프로젝트 설정에서 기본 Architect 에이전트 지정하는 UX 필요

### 설계 확정 but 미구현
- Workflow Skill (Tier 1/2) — plan 활성 시 상세 규약 주입, phase별 추가 주입
- Agent Template 자동 로딩 — `docs/agents/*.md`를 ContextPack에서 role에 맞게 자동 주입하는 로직 (현재는 `loader.rs`에서 agent_name으로 수동 로드)

---

## 수정된 파일 목록 (커밋 대상)

```
src-tauri/src/commands/agents_helpers/send_common.rs  — ContextData, load_context_data, assemble_prompt, PLATFORM_TIER0
src-tauri/src/commands/projects.rs                    — ensure_workflow_templates, agent templates, Tauri command
src-tauri/src/lib.rs                                  — command 등록
src/stores/slices/projectSlice.ts                     — selectProject에서 workflow templates 확보
src/lib/workflowOrchestration.ts                      — requestPlanRevision()
src/components/tunaflow/context-panel/PlansPanel.tsx   — PlanRevisionButton
src/components/tunaflow/MessageItem.tsx                — user 메시지 마크다운 렌더링
docs/plans/orchestratedWorkflowPipelinePlan.md         — 설계 문서
docs/plans/index.md                                    — 인덱스 등록
```

---

## 핵심 설계 결정 (개발 시 참고)

1. **마커 기반 워크플로우**: 에이전트 응답에 HTML 코멘트 마커 삽입 → 프론트엔드 파싱 → UI 자동 표시
2. **Tier 0 = 항상 주입 (~200자)**: 새 설치에서도 에이전트가 워크플로우를 인식
3. **계획 수정 = Architect 경유**: Developer Branch 대화 → 압축 → Architect(메인 채팅) → plan-proposal → Plan 병합
4. **에이전트 선택 = 사용자 수동 (현재)**: 나중에 설정 기반 자동화 예정
