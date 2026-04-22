# tunaFlow Persona Baseline 검토 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 초안

## 목적

`/Users/d9ng/privateProject/tunaFlow/docs/how-to/tunaflow_persona_baseline_6.md`
문서를 현재 tunaFlow 구조 기준으로 검토하고,
바로 구현해도 되는 부분과 먼저 보정해야 하는 부분을 구분한다.

이번 단계의 목표는 persona 기능을 바로 구현하는 것이 아니라,
현재 제품 방향과 충돌하지 않도록 기준을 정리하는 것이다.

## 현재 전제

이미 정리된 방향:

- 사용자는 raw skill보다 `Agent Profile`을 선택한다
- `Settings > Agents`는 이미 구현됐고 실제 chat input에 연결됐다
- `Skill`은 관리 대상이고, 장기적으로 agent가 자동 선택하는 방향이다
- `Persona`는 말투 프리셋이 아니라 역할/행동 계약이어야 한다

현재 검토 대상 문서는 위 방향과 대체로 맞지만,
몇 가지는 지금 구조 기준으로 다시 판단해야 한다.

## 핵심 검토 포인트

### 1. 기본 6종 구성이 지금 워크플로에 맞는가

현재 문서의 6종:

1. Architect
2. Implementer
3. Reviewer
4. Debugger
5. UX Critic
6. Prompt Writer

검토할 것:

- `Prompt Writer`를 제품 기본군으로 둘지
- 대신 `Tester`가 더 적절한지
- tunaFlow의 실제 메인 탭/워크플로(`Plan / Artifacts / Review / Test`)와 더 잘 맞는 세트가 무엇인지

### 2. persona 책임 범위를 어디까지 둘 것인가

지금 문서는 `systemPromptTemplate` 비중이 크다.

검토할 것:

- persona가 최종 system prompt 전체를 책임지는 구조가 맞는지
- 아니면 persona는 `prompt fragment / policy block` 정도로 두고
  최종 조립은 runtime 계층이 맡는 것이 맞는지

### 3. `recommendedSkills`의 의미를 유지할 것인가

지금 제품 방향은:

- 사용자가 직접 skill을 토글하지 않는다
- agent profile이 기본 skill set을 가진다
- 장기적으로 agent가 task에 따라 skill을 자동 선택한다

따라서 검토할 것:

- `recommendedSkills`를 유지하되 의미를 축소할지
- `defaultSkillIds / defaultSkillCollections / autoSkillPolicy` 같은 방향으로 나눌지

### 4. scope와 적용 위치가 현재 제품 구조와 맞는가

지금 문서의 scope:

- `global`
- `sub_agent`
- `roundtable_participant`

검토할 것:

- 현재 `Agent Profile` 중심 제품 구조에서 더 필요한 적용 위치가 있는지
- 예: `agent_profile_default`, `chat_input`, `rt_participant`

## 권장 판단 기준

### 유지해도 되는 것

- Model / Persona / Skill 분리 원칙
- persona를 role contract로 보는 관점
- priorities / behaviors / constraints 구조
- built-in 6종 baseline을 먼저 두는 전략

### 바로 구현하면 위험한 것

- persona가 최종 system prompt 전체를 독점하는 구조
- skill 자동 추천과 자동 선택을 같은 필드로 뭉개는 것
- 제품 메인 워크플로와 맞지 않는 기본 persona 세트 확정

## 기대 산출물

이번 단계에서 필요한 것은 구현 코드가 아니라 다음 중 하나다.

1. `tunaflow_persona_baseline_6.md` 보정안
2. 유지 / 수정 / 후순위 항목 분류
3. persona 구현을 바로 시작해도 되는지에 대한 판단

## 비목표

- persona editor 구현
- runtime prompt assembler 대규모 변경
- auto skill selection 구현
- agent profile 데이터 모델 대규모 재설계

## 완료 기준

1. baseline 문서의 유지/수정 포인트가 명확히 정리된다
2. 지금 바로 persona 구현을 시작할지, 선행 보정이 필요한지 판단할 수 있다
3. `Agent Profile`과 persona의 책임 경계가 이전보다 분명해진다

