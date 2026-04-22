# 자연어 기반 Agent Handoff 고도화 방안

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 08:37 KST
- 대상 프로젝트: `D:\privateProject\tunaFlow`

---

## 목적

현재 tunaFlow는 다음 수준까지 구현되어 있다.

- 멀티엔진 호출
- branch / roundtable / plan / artifact / findings 기반 handoff
- message / artifact 기준 follow-up UX

다음 고도화 후보는 사용자가 버튼이나 고정 UI 액션만이 아니라,
**자연어에 가까운 표현으로 다른 agent/engine에게 작업을 넘길 수 있게 하는 것**이다.

예:

- “이건 Codex로 넘겨서 구현해”
- “Claude로 더 다듬어”
- “Gemini한테 반박 검토시켜”
- “이 artifact 기준으로 Codex follow-up”
- “이 subtask는 Claude에게 맡겨”

이 문서는 그 기능을 한 번에 무리하게 넣지 않고,
현실적으로 단계별 도입하는 방안을 정리한다.

---

## 왜 지금 가능한가

현재 tunaFlow에는 이미 다음 기반이 존재한다.

- `sendFollowup(...)` 기반 handoff 실행 경로
- `Shared Brief`
- `Recent Agent Findings`
- `Recent Artifacts`
- `Plan ownership` 메타데이터
- branch / canonical conversation 처리

즉, 문제는 “handoff 기능 자체가 없어서”가 아니라,
**사용자가 handoff를 더 자연스럽게 호출할 수 있도록 해석 계층을 추가하느냐**의 문제다.

---

## 왜 바로 완전 자유 자연어로 가지 않는가

완전 자유 자연어 해석은 다음 리스크가 있다.

- 어떤 engine을 의미하는지 애매함
- 어떤 source를 넘기라는 건지 애매함
- refine / implement / critique / summarize 중 무엇을 뜻하는지 애매함
- branch / message / artifact / subtask 중 무엇을 가리키는지 애매함

예:

- “이건 코덱스로 넘겨”
- “좀 더 날카롭게 검토해”
- “이거 구현 쪽으로 보내”

이런 표현은 사람에겐 자연스럽지만,
시스템 입장에서는 구조화된 action으로 바꾸는 과정이 필요하다.

따라서 가장 좋은 방식은:

1. 제한된 자연어 alias
2. source-aware 문장 패턴
3. 더 자유로운 자연어

순서로 점진 도입하는 것이다.

---

## 목표 상태

최종적으로는 사용자가 다음처럼 말할 수 있게 하는 것이 이상적이다.

- “이 메시지를 Codex로 넘겨서 구현해”
- “이 artifact를 Claude로 정리해”
- “현재 플랜의 이 subtask를 Gemini한테 검토시켜”

그리고 시스템은 이를 아래 구조로 해석한다.

- target engine
- source type
- source object
- goal
- optional constraints

즉, 목표는 단순 채팅이 아니라
**자연어 기반 handoff command layer**를 가지는 것이다.

---

## 권장 구현 단계

## Phase A. 제한된 자연어 alias

### 목표

완전 자유 문장이 아니라, 자연어처럼 보이지만 사실상 제한된 패턴부터 지원한다.

예:

- “Claude로”
- “Codex로 구현”
- “Gemini로 검토”
- “Claude로 다듬기”

### 구현 방식

현재 follow-up UX와 연결되는 command parser를 아주 얇게 추가한다.

입력 예시를 아래처럼 해석:

- `Claude로` → engine = claude, goal = refine
- `Codex로 구현` → engine = codex, goal = implement
- `Gemini로 검토` → engine = gemini, goal = critique

### 장점

- 구현 난이도가 낮음
- 사용감은 자연어에 가까움
- 기존 `sendFollowup(...)`를 그대로 재사용 가능

### 한계

- source 지정이 약함
- 현재 선택 중인 메시지/아티팩트 같은 UI 상태에 의존할 수 있음

---

## Phase B. Source-aware handoff 문장

### 목표

source 대상까지 자연어로 지정하게 한다.

예:

- “이 메시지를 Codex로 넘겨”
- “이 artifact로 Claude follow-up”
- “이 subtask를 Gemini한테 검토시켜”

### 필요한 해석 요소

- source type:
  - message
  - artifact
  - plan
  - subtask
- target engine:
  - claude
  - codex
  - gemini
  - 필요 시 opencode
- goal:
  - refine
  - implement
  - critique
  - summarize

### 구현 방향

현재 UI에서 선택된 객체 또는 hover된 객체를 기준으로,
자연어 command가 들어오면 구조화된 handoff payload로 바꾼다.

즉:

- 자연어는 intent 입력
- 실제 source object는 UI selection/context가 보완

### 장점

- 실제 사용성이 크게 좋아짐
- 버튼 UX와 자연어 UX를 공존시킬 수 있음

### 리스크

- “이거”, “이 메시지”, “이 artifact” 같은 지시 대상 해석이 UI 상태에 의존

---

## Phase C. 자유 자연어 해석

### 목표

보다 자유로운 문장을 handoff action으로 변환한다.

예:

- “이거 Codex한테 넘겨서 바로 구현하게 해”
- “Claude로 다시 정리하고 Gemini로 반박 검토까지 이어가”
- “현재 플랜 기준으로 다음 할 일을 Codex에 맡겨”

### 필요한 것

- 자연어 → 구조화 action parser
- ambiguity fallback
- 확인 프롬프트 또는 preview step

예:

- 해석 결과:
  - engine = codex
  - source = active message
  - goal = implement

실행 전 사용자에게 한 줄 preview를 보여줄 수도 있음

### 권장 여부

이 단계는 가장 나중에 검토하는 것이 좋다.

현재 tunaFlow는 아직

- ownership 완성
- plan-origin follow-up
- 일부 UX polish

가 먼저다.

---

## 현실적인 첫 구현 방향

현재 tunaFlow 상태에서 가장 추천하는 시작점은 아래다.

### 1순위

Phase A:

- 제한된 자연어 alias 지원

예:

- “Claude로”
- “Codex로 구현”
- “Gemini로 검토”

이건 follow-up 버튼 없이도 빠르게 handoff를 걸 수 있게 해 준다.

### 2순위

Phase B 일부:

- “이 메시지를 Codex로”
- “이 artifact를 Claude로”

정도의 명시적 source 지정

### 3순위

Phase C는 보류

완전 자유 자연어는 나중에 해도 된다.

---

## 추천 설계 원칙

### 1. 자연어 해석 결과는 항상 구조화된 action으로 변환

중간 표현 예:

```json
{
  "engine": "codex",
  "sourceType": "message",
  "goal": "implement",
  "sourceRef": "currentMessage"
}
```

즉, 자연어를 직접 실행하지 말고
항상 기존 handoff 실행 계층으로 변환한 뒤 실행한다.

### 2. 애매한 경우는 실행하지 말고 fallback

예:

- 해석 실패
- source 미확정
- engine 애매

이 경우:

- 기존 버튼 UX로 유도
- 또는 짧은 확인 문구 표시

### 3. 기존 UI와 경쟁하지 말고 보완

버튼 기반 follow-up은 유지하고,
자연어 handoff는 power-user 기능처럼 추가하는 것이 좋다.

### 4. 현재 ContextPack 구조를 재사용

새로운 대규모 prompt framework를 만들 필요는 없다.

이미 있는:

- plan
- findings
- artifacts
- context summary

를 그대로 활용하고,
자연어 해석 결과는 handoff prompt 앞부분만 보강하면 충분하다.

---

## 예상 구현 위치

프론트 기준 후보:

- `src/stores/chatStore.ts`
- `src/components/tunaflow/NewMessageInput.tsx`
- `src/components/tunaflow/ChatPanel.tsx`

백엔드 기준 후보:

- 별도 큰 변경 없이 기존 `sendMessage` / `sendWithCodex` / `sendWithGemini` 재사용 가능

즉, 이 기능은 현재 구조상
**프론트 command interpretation layer**에 더 가깝다.

---

## 완료 기준 예시

### Phase A 완료 기준

- 최소 3개 alias 지원
- 현재 선택된/최근 source를 기준으로 handoff 가능
- 기존 send 흐름 재사용

### Phase B 완료 기준

- message / artifact / subtask 중 최소 2종 이상 source 지정 가능
- handoff parser가 구조화 action을 반환

### Phase C 완료 기준

- 자유 자연어의 일부를 안정적으로 해석
- ambiguity fallback 존재

---

## 결론

자연어 기반 agent handoff는 **지금 tunaFlow에서 충분히 현실적인 다음 고도화 후보**다.

다만 바로 완전 자유 자연어로 가지 말고,

1. 제한된 alias
2. source-aware 문장
3. 더 자유로운 해석

순으로 가는 것이 가장 안전하다.

현재 tunaFlow는 이미 handoff 실행 기반이 있으므로,
이 작업의 핵심은 “새 시스템을 만드는 것”이 아니라
**자연어를 기존 handoff 실행 계층으로 잘 연결하는 것**이다.
