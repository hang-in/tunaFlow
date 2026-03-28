# tunaFlow 4-Engine Feature Parity 분류 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: Wave 1 완료, Wave 2 완료 (token/cost는 frontend parity + backend partial)

## 목적

`Claude / Codex / Gemini / OpenCode`를 동등한 1급 엔진으로 취급하려면,
기능 차이를 "관찰"하는 수준에서 끝내면 안 된다.

어떤 기능을 반드시 동일하게 맞출지,
어떤 기능은 엔진 특성에 맞는 동등 개념으로 허용할지,
어떤 기능은 후순위로 둘지 먼저 분류해야 한다.

이 문서는 그 기준 문서다.

## 현재 판단

현재 구조는 여전히 Claude 중심이다.

- Claude: richer prompt assembly, resume token, stronger system prompt path
- Codex/Gemini/OpenCode: lite context 중심
- Gemini: streaming 일부 존재
- Codex/OpenCode: 일부 기능은 one-shot 수준

즉 "4-engine feature parity"는 아직 미완료다.

## 분류 기준

### P0. 반드시 동등해야 하는 기능

이 범주는 제품 일관성에 직접 영향을 준다.
엔진을 바꿨을 때 사용자가 기능 상실을 느끼면 안 된다.

1. skills 적용
2. rich context / full context pack
3. collaboration sections
   - plan
   - findings
   - artifact handoff
   - thread inheritance
   - cross-session summary
4. rawq context injection
5. streaming 경험
6. token/cost usage visibility

### P1. 동등 개념으로 맞춰야 하는 기능

엔진마다 구현 방식이 달라도,
사용자 관점에서 비슷한 결과를 내야 한다.

1. system prompt 전달 방식
2. conversation continuation / resume

예:
- Claude는 native resume token
- 다른 엔진은 thread replay, conversation anchor, synthetic continuation

### P2. 후순위 기능

동등하면 좋지만, 현재 parity 기준의 차단 요소는 아니다.

1. provider 고유 추가 메타데이터
2. 엔진별 세부 성능 튜닝
3. provider-native advanced flags

## 문서 묶음

이 분류에 따라 아래 개별 계획 문서를 순차 실행 대상으로 둔다.

1. `skillsEngineParityPlan.md`
2. `contextPackEngineParityPlan.md`
3. `collaborationContextEngineParityPlan.md`
4. `rawqEngineParityPlan.md`
5. `streamingEngineParityPlan.md`
6. `tokenCostTrackingEngineParityPlan.md`
7. `resumeContinuationEngineParityPlan.md`

## 권장 실행 순서

### Wave 1. Prompt/context parity

1. skills
2. context pack
3. collaboration sections
4. rawq

### Wave 2. Runtime/session parity

5. streaming
6. token/cost tracking
7. resume/continuation

이 순서가 맞는 이유:

- 앞 4개가 실제 응답 품질 차이를 가장 크게 만든다.
- 뒤 3개는 실행 상태와 운영 가시성 차이다.

## 완료 기준

다음 조건이 충족되면 parity 기준이 서기 시작한 것으로 본다.

1. 각 기능에 대해 "현재 차이 / 목표 / 구현 순서 / 검증 방법"이 개별 문서로 정리됨
2. Claude가 그 문서를 기준으로 순차 작업 가능함
3. `implementationStatus.md`의 provider 차이 표를 줄일 수 있는 방향이 명확해짐

