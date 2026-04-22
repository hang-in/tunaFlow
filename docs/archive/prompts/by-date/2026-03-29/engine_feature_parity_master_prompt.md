# tunaFlow 4-Engine Feature Parity 마스터 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow 4-Engine Feature Parity 정렬 작업

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/engineFeatureParityClassificationPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/skillsEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/contextPackEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/collaborationContextEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/rawqEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/streamingEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/tokenCostTrackingEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/resumeContinuationEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/implementationStatus.md`

핵심 전제:
- Claude만 더 잘 되는 구조를 유지하지 말 것
- Claude / Codex / Gemini / OpenCode를 동등한 1급 엔진으로 취급할 것
- provider별 구현 차이는 허용되지만, 사용자 경험과 기능 의미는 동등해야 한다
- 추측하지 말고 실제 코드 기준으로만 수정할 것
- 무관한 대규모 리팩토링 금지

## 작업 방식

아래 7개 작업을 **순차적으로** 수행하라.
각 단계는 가능한 한 독립적으로 끝내고, 다음 단계로 넘어가라.

1. Skills parity
2. ContextPack parity
3. Collaboration context parity
4. rawq parity
5. Streaming parity
6. Token/cost tracking parity
7. Resume/continuation parity

## 단계별 공통 규칙

각 단계마다 반드시:

1. 현재 코드 기준 차이를 다시 확인
2. 최소 수정 범위로 parity를 높이는 변경을 적용
3. 문서가 어긋나면 함께 정리
4. 검증 결과를 남긴다

## 수정 우선순위

### 1. 먼저 맞출 것

- 공통 prompt/context assembly
- 공통 runtime state model
- 공통 usage/continuation contract

### 2. 이번에 하지 말 것

- 새 엔진 추가
- 대형 UI 재설계
- rawq 내부 로직 재구현
- unrelated store refactor

## 단계별 기대 산출물

### Step 1. Skills parity

- activeSkills가 4개 엔진 모두 실제 prompt/context에 반영
- applied skills trace 또는 debug 가능

### Step 2. ContextPack parity

- non-Claude 경로도 full-equivalent normalized context payload 사용

### Step 3. Collaboration context parity

- plan/findings/artifacts/thread inheritance/cross-session이 4개 엔진 모두 반영

### Step 4. rawq parity

- rawq section inclusion과 diagnostics가 엔진별로 갈리지 않음

### Step 5. Streaming parity

- 최소한 동일한 streaming UX/state contract 확보

### Step 6. Token/cost tracking parity

- 4개 엔진 모두 exact/estimated/unavailable 중 하나로 usage 남김

### Step 7. Resume/continuation parity

- Claude native resume + non-Claude synthetic continuation으로 conversation continuity 맞춤

## 검증

각 단계 후 가능한 검증을 수행하라.

- `cargo check`
- 관련 frontend type check 또는 test
- 필요 시 엔진별 호출 경로 점검

검증이 불가능하면 왜 불가능한지 적어라.

## 결과 보고 형식

최종 보고는 반드시 아래 형식을 따를 것.

### A. Overall Decision

### B. Step-by-Step Results

각 step마다 아래 5줄을 포함:
- Current Gap
- Changes Made
- Verification
- Residual Risk
- Follow-up

### C. Files Changed

### D. Provider Parity Status After This Work

기능별로 Claude / Codex / Gemini / OpenCode 상태를 짧게 표기:
- equal
- near-equal
- partial
- blocked

### E. Remaining Deferred Work

작업을 중간에 멈추지 말고,
위 순서대로 가능한 범위까지 계속 진행하라.
```
