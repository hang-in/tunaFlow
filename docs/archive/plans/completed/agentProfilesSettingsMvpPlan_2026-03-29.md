# tunaFlow Agent Profiles Settings MVP 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

`Settings > Agents`를 placeholder 상태에서 실제 관리 UI로 올린다.

이번 단계의 목표는 사용자가 채팅에서 선택할 `Agent Profile`의 최소 관리 기능을 만드는 것이다.

## 전제

- Persona 편집 UI는 이번 단계에서 하지 않는다
- Skills는 이미 `Settings > Skills`로 이동했다
- 이번 단계는 agent profile의 **구조와 관리 UI**를 먼저 만드는 MVP다

## Agent Profile 정의

MVP 기준 Agent Profile은 아래 필드를 가진다.

- `id`
- `label`
- `engine`
- `model`
- `personaKey` 또는 임시 persona 문자열
- `defaultSkills[]`

장기적으로는 여기에 runtime policy, default context policy 등이 붙을 수 있다.

## 이번 단계 목표

1. `Settings > Agents`에서 profile 목록을 볼 수 있다
2. 기본 profile 2~4개를 생성/표시할 수 있다
3. profile을 선택하면 우측에서 engine/model/default skills를 편집할 수 있다
4. 저장 위치는 우선 settings 기반으로 둔다

## 권장 초기 profile

- `Architect Claude`
- `Reviewer Codex`
- `Tester Gemini`
- `General OpenCode`

이 이름은 고정이 아니라 seed data에 가깝다.

## UI 구조

### 좌측

- profile 목록
- `+ New Agent`

### 우측

- label
- engine selector
- model selector
- persona 표시
- default skills multi-select 또는 최소 list 형태

## 저장 전략

MVP에서는 app settings 또는 store 기반 local persistence로 충분하다.

후속에서 별도 DB/프로젝트 기본값으로 확장 가능하다.

## 비목표

- chat input 연결
- persona 관리 화면 구현
- auto skill selection
- project별 agent override

## 완료 기준

1. `Settings > Agents`가 placeholder가 아니다
2. agent profile 목록/선택/편집이 가능하다
3. 기본 skills를 profile에 연결할 수 있다
4. 재실행 후 profile이 유지된다

