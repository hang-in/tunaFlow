# tunaFlow Agent / Skill / Persona IA 재구성 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

현재 `tunaFlow`의 skill UI는 개발 단계에서는 유용했지만,
최종 제품 UX로는 `skill toggle 중심`이라는 점에서 한계가 있다.

이 문서의 목적은 아래 세 가지를 재정의하는 것이다.

1. 사용자가 실제로 선택해야 하는 1급 개념은 무엇인가
2. skill / persona / model을 어디에서 관리해야 하는가
3. workspace UI와 settings UI를 어떻게 분리해야 하는가

## 핵심 판단

최종적으로 사용자가 채팅 입력 시 선택해야 하는 것은 `Skill`이 아니라 `Agent Profile`이다.

즉:

- `Skill` = 관리 대상
- `Persona` = 역할/스타일 정의
- `Agent Profile` = 실행 단위

가 되어야 한다.

## 현재 구조의 문제

### 1. Skills가 workspace 안에 있다

현재 `ContextPanel` 내 `Skills` 탭 (`src/components/tunaflow/context-panel/SkillsPanel.tsx`) 구조는
작업 중에 사용자가 skill을 직접 토글하는 흐름을 전제한다.

이 구조는 아래 문제를 만든다.

- skill이 작업 목표보다 앞에 드러난다
- 매 요청마다 skill을 조작해야 하는 것처럼 느껴진다
- 사용자가 실제로 고르고 싶은 "에이전트 조합"이 드러나지 않는다

### 2. skill, persona, model이 분리된 채 노출된다

사용자는 실제로 아래 조합을 하나의 작업자로 인식한다.

- engine
- model
- persona
- 기본 skill 세트

그런데 현재 UI는 이걸 하나의 개념으로 보여주지 않는다.

### 3. applied skills visibility가 부족하다

스킬이 실제로 언제 어떻게 적용됐는지 추적이 약하다.
그래서 skill을 자동 선택하도록 바꾸더라도 결과가 블랙박스처럼 느껴질 수 있다.

## 목표 정보 구조

### 1. 메인 워크스페이스

- 좌측: Workspace Tree
  - Projects
  - Chats (RT/Branch는 하위 트리)
  - Artifacts
  - Files
- 중앙 상단 탭:
  - Chat
  - Plan
  - Review
  - Test
- 우측:
  - RT / Branch overlay
- 하단:
  - Runtime Status Bar

### 2. Settings

- Agents
- Personas
- Skills
- Runtime / Tools

즉 skill은 workspace에서 제거하고 settings 쪽으로 이동한다.

## 개념 정의

### Skill

정의:

- 파일 기반 source of truth를 가진 실행 보조 리소스
- 사용자가 직접 "관리"하는 대상

포함 메타:

- name
- vendor
- source path
- scope (global / project)
- collections
- tags
- toolTargets

### Persona

정의:

- 에이전트의 역할/톤/행동 스타일 정의

예:

- architect
- reviewer
- tester
- concise
- docs

### Agent Profile

정의:

- 사용자가 채팅에서 실제로 선택하는 실행 단위

구성:

- engine
- model
- persona
- default skill collections
- optional runtime policy

예:

- `Architect Claude`
- `Reviewer Codex`
- `Tester Gemini`

## chops 도입 방향

`chops`는 "skill 관리 앱"으로서 좋은 참고 구조를 제공한다.
다만 `tunaFlow`는 chops 자체가 아니라, chops의 관리 철학을 일부 도입하는 것이 맞다.

### 가져올 것

1. 파일 기반 registry
2. collection 계층
3. source/path/vendor/toolTargets 메타
4. search / filter / refresh 구조

### 그대로 가져오지 않을 것

1. skill 관리 앱 전체 UX
2. 내장 editor 중심 흐름
3. chops 전용 플랫폼 가정

### tunaFlow식 해석

- chops = `Settings > Skills`
- tunaFlow main workflow = `Agent Profile 선택 + skill auto-selection`

## 권장 UX 흐름

### 사용자 흐름

1. 사용자는 settings에서 skill과 persona와 agent profile을 관리한다
2. 채팅 입력에서는 `어떤 agent profile로 실행할지`를 고른다
3. 필요하면 고급 옵션에서 일회성 override를 사용한다
4. 실행 후 applied skills를 결과/trace에서 확인한다

### 시스템 흐름

1. agent profile이 기본 model/persona/default skills를 제공한다
2. 요청 분석 후 agent가 필요한 skills를 추가 선택할 수 있다
3. 실제 적용된 skills는 trace/message meta에 남긴다

## 단계별 구현 제안

### Phase 1. UI 역할 정리

- SkillsPanel을 workspace에서 제거
- Settings IA에 `Agents / Personas / Skills` 추가
- 현재 Skills UI 작업은 settings 하위 registry로 재배치

### Phase 2. Agent Profile 1차 도입

- 입력창에서 skill이 아니라 agent profile 선택
- 기존 activeSkills는 내부 구현 detail로 후퇴
- profile에 model/persona/default skills 연결
- 제약: RT는 독립 conversation이 아니라 branch-only로 생성됨 — Agent Profile은 branch/RT 양쪽에서 동작해야 함

### Phase 3. chops-style Skill Registry

- vendor / collection / scope / search / snapshot 메타 정리
- 수동 refresh / snapshot 상태 표시

### Phase 4. Skill auto-selection

- 요청 전 agent가 skill 후보를 선택
- applied skills를 trace/message meta에 남김

### Phase 5. Override / explainability

- 사용자가 필요하면 일회성 override 가능
- "왜 이 skill이 적용됐는지"를 확인 가능

## 비목표

- 즉시 chops 앱 전체 복제
- settings 내 내장 skill editor 우선 구현
- 자동 추천을 충분한 visibility 없이 먼저 도입

## 완료 기준

1. workspace에서 skill toggle이 Settings로 이동한다
2. chat input은 agent profile 중심이 된다
3. settings에서 skill/persona/agent를 관리할 수 있다
4. applied skills가 실행 결과에 연결된다

## 최종 판단

이 구조가 맞다.

최종 제품에서 사용자는:

- `skill을 직접 실행`하지 않고
- `agent를 선택`하며
- `agent가 skill을 선택해 사용`하고
- 사용자는 `skill registry를 관리`하는

방식으로 상호작용해야 한다.

