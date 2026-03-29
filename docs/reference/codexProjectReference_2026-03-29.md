# tunaFlow 프로젝트 참고 문서 (Codex 기준)

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 목적: 새 Claude Code 세션이 `CLAUDE.md` 외에 함께 참고할 수 있는, 코드베이스 관찰 기준 요약

## 한 줄 정의

`tunaFlow`는 단순 채팅 앱이 아니라,
프로젝트 단위로 여러 CLI 에이전트를 오케스트레이션하는 Tauri 데스크톱 IDE다.

## 현재 큰 구조

### 프론트엔드

- React
- Zustand
- 3패널 기반 UI에서 점차 linear.app식 구조로 재편 중

핵심 진입:

- `src/App.tsx`
- `src/components/tunaflow/AppShell.tsx`
- `src/components/tunaflow/ChatPanel.tsx`
- `src/components/tunaflow/ContextPanel.tsx`

### 백엔드

- Tauri v2
- Rust command layer
- SQLite 저장

핵심 진입:

- `src-tauri/src/lib.rs`
- `src-tauri/src/commands/*.rs`
- `src-tauri/src/agents/*.rs`

## 현재 제품 개념

### 1. Conversation / Branch / Roundtable

- main conversation
- branch shadow conversation
- roundtable mode

즉 하나의 프로젝트 안에서 chat, branch, RT가 함께 돌아간다.

### 2. ContextPack 기반 agent 실행

에이전트 실행은 단순 prompt 전달이 아니라
project / recent context / plan / findings / artifacts / skills / rawq / cross-session / thread inheritance
같은 section을 조립해 전달하는 구조다.

### 3. Artifact / Plan / Review / Test workflow

채팅 결과를 단순 message로 끝내지 않고,
artifact / review / test / plan 흐름으로 재사용한다.

## 최근 구조 변화

### 1. 4-engine parity 방향 채택

현재 프로젝트는 `Claude 중심`에서
`Claude / Codex / Gemini / OpenCode 동등성` 방향으로 이동 중이다.

이미 정리된 영역:

- skills injection parity
- normalized context pack parity
- collaboration context parity
- rawq context parity

부분 완료:

- streaming parity
- token/cost parity
- continuation parity 문서 정리

### 2. rawq는 필수 sidecar 방향

rawq는 optional fallback tool이 아니라,
제품이 함께 관리해야 하는 runtime dependency로 보는 방향이 확정됐다.

### 3. skills는 runtime snapshot 운영

- source of truth: `_research/_skills`
- runtime snapshot: `~/.tunaflow/skills`

현재는 snapshot publish 방식으로 운영한다.

### 4. Skills UI는 임시 성격이 강해짐

기존 SkillsPanel은 visibility 실험 단계에서는 유용했지만,
최종 구조에서는 workspace에서 빠지고 settings / agent profile 중심 구조로 이동할 가능성이 높다.

## 현재 UI/UX 재구성 방향

### 확정에 가까운 방향

- 오른쪽 ContextPanel 제거 또는 대폭 축소
- `Plan / Review / Test`를 메인 탭으로 승격
- `Artifacts`를 좌측 트리 영역으로 이동
- `Trace`는 상태바 + 상세 팝업/모달로 축소

### 새 핵심 구조

- 좌측: workspace tree
- 중앙 상단: Chat / Plan / Review / Test
- 중앙 본문: 작업 화면
- 우측: RT / Branch overlay
- 하단: runtime status

## Skill / Persona / Agent 방향

Codex 기준 판단:

- Skill은 사용자가 직접 매번 토글하는 메인 UI가 아니다
- 사용자는 agent profile을 선택해야 한다
- skill은 settings에서 관리해야 한다
- 최종적으로는 agent가 요청에 따라 skill을 자동 선택하는 방향이 맞다

즉:

- Skills = 관리 대상
- Personas = 역할 정의
- Agent Profiles = 실제 실행 단위

## TracePanel 관련 판단

현재 TracePanel은 runtime monitor와 trace history가 섞여 있어 역할이 불명확하다.

권장 방향:

- 상단: 현재 conversation 기준 runtime 상태
- 하단: trace history
- 전역 active jobs와 conversation-local history를 섞지 말 것

## 채팅 UI 관련 판단

`tunaFlow`는 기능은 강하지만 채팅 표면 UI는 `tunaChat`보다 덜 다듬어져 있다.

우선순위:

1. codeblock UX
2. file viewer / path click
3. message density
4. virtualization

## 지금 새 세션에서 Claude가 특히 조심할 것

1. 엔진 동등성 원칙
- Claude만 특별 취급하는 구조를 다시 만들지 말 것

2. skill toggle 중심 UX 유지 금지
- skill은 장기적으로 settings/agent profile로 이동할 방향이다

3. Trace와 runtime 데이터를 섞지 말 것
- global active jobs와 current conversation history는 구분해야 한다

4. 메인/보조 작업 영역 구분
- Plan / Review / Test는 보조 패널이 아니라 메인 workflow 영역이다

## 새 세션 추천 읽기 순서

1. `CLAUDE.md`
2. `docs/reference/implementationStatus.md`
3. `docs/reference/codexProjectReference_2026-03-29.md`
4. 관련 task의 `docs/plans/*.md`
5. 관련 task의 `docs/prompts/*.md`

## 최종 판단

이 프로젝트는 이제 단순 기능 추가보다,

- 정보 구조 재정리
- 엔진 동등성
- agent 중심 UX
- runtime / artifact / review 흐름의 분리

가 더 중요한 단계에 들어와 있다.

