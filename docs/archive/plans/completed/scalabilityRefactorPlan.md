# tunaFlow 확장 대비 리팩토링 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 21:00 KST

## 목적

`tunaFlow`는 최근 세션들에서 Sidebar, workspace panel, thread/RT inheritance, harness summary, custom label,
message/input UI, drawer overlay, model discovery 등 기능이 빠르게 누적되었다.

지금 시점의 리팩토링 목표는:

1. 코드 스타일 정리보다 **기능 추가 여지 확보**
2. 앞으로 기능이 더 붙을 허브 파일을 먼저 분해
3. 테스트를 붙이기 쉬운 책임 경계 만들기
4. 프로젝트 중심 설계 원칙을 유지한 채 구조를 안정화

이다.

## 현재 관찰

### 프론트에서 커진 허브 파일

- `src/stores/chatStore.ts`
- `src/components/tunaflow/Sidebar.tsx`
- `src/components/tunaflow/NewMessageInput.tsx`
- `src/components/tunaflow/BranchThreadPanel.tsx`
- `src/components/tunaflow/RoundtableView.tsx`
- `src/components/tunaflow/MessageItem.tsx`

### 백엔드에서 커진 허브 파일

- `src-tauri/src/commands/agents.rs`
- `src-tauri/src/commands/branches.rs`
- `src-tauri/src/commands/roundtable.rs`
- `src-tauri/src/commands/projects.rs`
- `src-tauri/src/commands/model_discovery.rs`

### 테스트 현황

실행 기준:

- `npm test` → Frontend Vitest 3 files / 13 tests 통과
- `cargo test` → Rust 40 tests 통과
- `npx tsc --noEmit` → 통과
- `npm run build` → 통과

현재 테스트 층:

- 프론트:
  - `src/tests/api-artifacts.test.ts`
  - `src/tests/api-memos.test.ts`
  - `src/tests/api-plans.test.ts`
- 백엔드:
  - `src-tauri/tests/db_integration.rs`
  - `commands::agents_helpers::*`
  - `roundtable_helpers::prompt::*`

부족한 층:

- Sidebar / drawer / workspace panel UI 상호작용 테스트
- branch/RT inheritance 흐름 테스트
- model discovery / `!models --refresh` 프론트 연결 테스트
- E2E smoke test

## 리팩토링 원칙

1. 기존 기능을 유지하면서 **책임 경계만 먼저 분리**
2. store와 orchestrator 파일을 우선 분해
3. 중복 UI를 공용 컴포넌트/섹션으로 끌어올림
4. 새 기능이 붙을 지점을 예측해 seam을 만든다
5. 분해 후 바로 작은 테스트를 추가할 수 있는 구조를 목표로 한다

## 우선순위

### Phase 1. 상태 저장소 분해

대상:

- `src/stores/chatStore.ts`

목표:

- 단일 mega store를 기능별 slice 성격으로 분리

권장 분해:

- `projectSlice`
- `conversationSlice`
- `branchSlice`
- `runtimeSlice`
- `artifactMemoSlice`
- `engineModelSlice`

효과:

- 프로젝트 선택 / conversation 전환 / thread drawer / queue / artifact / model 목록이 서로 덜 얽힘
- 이후 테스트에서 slice 단위 검증이 가능해짐

### Phase 2. Sidebar 분해

대상:

- `src/components/tunaflow/Sidebar.tsx`

목표:

- 프로젝트 선택기와 4섹션 탐색기를 파일 단위로 분리

권장 분해:

- `sidebar/TreeRow.tsx`
- `sidebar/SectionHeader.tsx`
- `sidebar/ProjectsSection.tsx`
- `sidebar/ChatsSection.tsx`
- `sidebar/RoundtablesSection.tsx`
- `sidebar/BranchesSection.tsx`
- `sidebar/FilesSection.tsx`
- 필요 시 `sidebar/useDirectoryListing.ts`

효과:

- 현재 선택 프로젝트 중심 구조를 유지하면서도 후속 기능 추가가 쉬워짐
- 프로젝트 row / branch tree / file tree 수정 범위를 좁힐 수 있음

### Phase 3. 입력 영역 분해

대상:

- `src/components/tunaflow/NewMessageInput.tsx`

목표:

- 입력 surface, selector, RT controls, send logic를 분리

권장 분해:

- `input/EngineSelector.tsx`
- `input/ModelSelector.tsx`
- `input/RoundtableControls.tsx`
- `input/ContextBadges.tsx`
- `input/InputToolbar.tsx`
- `input/useSendActions.ts`

효과:

- 이후 agent settings, persona, model discovery, project-specific send 옵션 추가가 쉬워짐

### Phase 4. 메시지/드로어/RT view 공통 패턴 분해

대상:

- `src/components/tunaflow/MessageItem.tsx`
- `src/components/tunaflow/BranchThreadPanel.tsx`
- `src/components/tunaflow/RoundtableView.tsx`

목표:

- 공통 헤더, 메타 row, action group, avatar, markdown surface 패턴을 공용화

권장 분해:

- `message/MessageMeta.tsx`
- `message/MessageActions.tsx`
- `message/ProgressSurface.tsx`
- `message/BranchLinkRow.tsx`
- `message/AgentAvatar.tsx` 정리

효과:

- UI 일관성 강화
- 후속 메시지 기능 추가 시 3곳을 동시에 고치지 않아도 됨

### Phase 5. 백엔드 agents.rs 분해

대상:

- `src-tauri/src/commands/agents.rs`

목표:

- 엔진별 dispatch, context assembly, queue/cancel glue, trace 기록을 helper 수준으로 분리

권장 분해:

- `agents_helpers/send_dispatch.rs`
- `agents_helpers/run_context.rs`
- `agents_helpers/thread_runtime.rs`
- `agents_helpers/model_resolution.rs`

중요:

- 이미 존재하는 `context_pack.rs`, `trace_log.rs`, `compression.rs`와 자연스럽게 맞물리게 할 것

효과:

- 일반 agent cancel 강화
- model validation
- plan-based followup 자동 dispatch
같은 후속 작업을 넣기 쉬워짐

### Phase 6. 백엔드 command 레이어 정리

대상:

- `branches.rs`
- `roundtable.rs`
- `projects.rs`
- `plans.rs`
- `evaluation.rs`

목표:

- DB 조회/정규화/응답 DTO 생성 중복을 줄임

효과:

- commands 파일 길이 증가를 제어
- UI 추가 개발 시 command 확장이 덜 위험해짐

### Phase 7. 테스트 층 확장

우선순위:

1. store/slice 단위 프론트 테스트
2. Sidebar/Workspace/Drawer interaction test
3. model discovery command test
4. branch/RT inheritance integration test
5. E2E smoke test

중요:

- E2E를 먼저 늘리기보다, 분해 후 unit/integration seam이 생긴 뒤 붙이는 것이 더 효율적이다

## 지금 당장 권장 시작점

가장 먼저 손댈 가치가 높은 순서:

1. `chatStore.ts`
2. `Sidebar.tsx`
3. `NewMessageInput.tsx`
4. `agents.rs`

이 순서가 좋은 이유:

- 기능이 계속 붙는 허브
- 변경 충돌이 자주 날 가능성이 큼
- 작은 파일로 갈라야 이후 개발 속도가 유지됨

## 보류할 것

이번 리팩토링 계획에서는 아래를 직접 목표로 삼지 않는다.

- sidecar 전환
- 전역 검색
- 다중 프로젝트 동시 가시화
- git/worktree 실연동
- 대규모 E2E 우선 확대

이들은 현재 제품 원칙상 후순위다.

## 기대 결과

이 리팩토링이 끝나면:

1. 기능 추가가 큰 파일 하나에 집중되지 않음
2. 프로젝트 중심 설계가 더 유지되기 쉬움
3. UI/상태/백엔드 확장 seam이 분명해짐
4. 테스트를 붙일 위치가 명확해짐

