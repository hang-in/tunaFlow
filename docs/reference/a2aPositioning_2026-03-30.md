# A2A Positioning

상태: 초안
작성: 2026-03-30

## 결론

tunaFlow에서 A2A는 장기적으로 유효한 방향이지만, **내부 코어를 대체하는 기준**이 아니라 **외부 호환/브리지 계층**으로 다루는 것이 맞다.

즉:
- `ContextPack`을 A2A로 재구성하지 않는다
- `Artifact / Plan / Handoff / Eval` 같은 내부 작업 객체를 유지한다
- 나중에 필요할 때 이 객체들을 A2A 메시지/태스크/파트로 투영한다

## 현재 판단

### A2A가 잘 맞는 이유

- tunaFlow는 이미 멀티에이전트 제품 구조를 가진다
- `RT`, `handoff`, `evaluation`, `agent profile` 같은 개념이 있어 agent-to-agent 통신 모델과 궁합이 있다
- 장기적으로 외부 에이전트와 상호운용하려면 공통 프로토콜 계층이 필요하다

### 지금 코어로 채택하면 안 되는 이유

- A2A는 agent 간 통신 프로토콜이지, 내부 컨텍스트 최적화 알고리즘이 아니다
- `ContextPack`은 최근 대화, plans, artifacts, rawq, context-hub 등을 고르고 압축하는 내부 조립 계층이다
- 지금 tunaFlow의 내부 오케스트레이션은 여전히 빠르게 진화 중이므로, 외부 프로토콜을 먼저 코어로 고정하면 제약이 커진다

## tunaFlow 내부 계층 구분

### 내부 코어

- `ContextPack`
- `Artifact`
- `Plan`
- `Memo`
- `Handoff`
- `Evaluation`

이 계층은 tunaFlow가 직접 설계하고 최적화한다.

### 외부 호환 계층

- `A2A`

이 계층은 외부 에이전트와 상호운용할 때 쓰는 transport / interoperability layer로 둔다.

## 권장 관계

- `ContextPack` = 내부 memory/context assembly
- `Artifact` = 재사용 가능한 문서/결과물
- `Plan` = 작업 구조
- `A2A` = 이 객체들을 외부 에이전트와 주고받는 표준 통신 계층

즉 A2A는:
- 내부 컨텍스트 코어가 아니라
- 내부 객체를 외부에 드러내는 adapter에 가깝다

## 나중에 가능한 매핑 예시

- `Artifact` → A2A file part 또는 structured JSON part
- `Plan summary` → A2A structured data
- `Current task` → A2A message/task
- `ContextPack summary` → A2A metadata 또는 attachment

## 지금 하지 말 것

- A2A를 기준으로 ContextPack을 재설계
- 내부 handoff 모델을 A2A task 모델로 즉시 치환
- 아직 안정화되지 않은 내부 객체를 A2A 사양에 맞추기 위해 왜곡

## 권장 순서

1. tunaFlow 내부 `ContextPack / Artifact / Handoff` 모델을 계속 안정화
2. 나중에 `A2A compatibility layer` 설계 문서 작성
3. 최소 범위의 adapter 실험:
   - Agent Card
   - message/send
   - message/stream
4. 필요 시 Eval/RT와의 연결 검토

## 한 줄 원칙

**A2A는 tunaFlow의 코어가 아니라, 나중에 붙는 외부 호환 레이어다.**
