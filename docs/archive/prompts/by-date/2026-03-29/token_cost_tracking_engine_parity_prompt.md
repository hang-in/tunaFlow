# tunaFlow Token/Cost Tracking Engine Parity 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Token/Cost Tracking Engine Parity

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/engineFeatureParityClassificationPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/tokenCostTrackingEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/implementationStatus.md`

우선 짧게 의견부터 말하라:
- 4개 엔진 usage/cost parity를 맞출 때 exact / estimated / unavailable 중 어떤 모델이 현실적인지
- OpenCode와 Gemini에서 무엇을 이번 단계에 실제로 달성할 수 있는지
- 이번에 같이 묶어야 할 것과 묶지 말아야 할 것

그 다음 실제 작업을 진행하라.

## 현재 상태

- Claude: usage/cost 비교적 풍부
- Codex: usage/cost 존재
- Gemini: partial
- OpenCode: 없음

## 목표

모든 엔진이 최소한 아래 중 하나의 상태로 usage를 남기게 하라.

1. exact
2. estimated
3. unavailable

그리고 그 상태를 코드/문서/UI에서 구분 가능하게 하라.

## 권장 방향

1. provider가 exact usage를 주면 그대로 사용
2. provider가 일부만 주면 partial가 아니라 exact/estimated로 재정의
3. provider가 전혀 안 주면 unavailable reason을 남긴다
4. "0"을 무의미하게 저장하는 것과 "unavailable"을 구분한다

## 수정 대상 후보

- `src-tauri/src/agents/*.rs`
- `src-tauri/src/commands/agents.rs`
- 필요 시 DB persistence 로직
- frontend usage 표시 코드
- `docs/reference/implementationStatus.md`
- `docs/plans/tokenCostTrackingEngineParityPlan.md`

## 중요

- 이번 세션은 token/cost tracking parity만 다룬다
- resume/continuation parity로 범위를 넓히지 말 것
- streaming parity를 다시 건드리지 말 것
- context pack 재작업 금지

## 검증

- `cargo check`
- 가능하면 frontend type check
- 각 provider별로 exact / estimated / unavailable 상태를 설명
- conversation 누적 usage가 깨지지 않는지 확인

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Usage Model by Provider
### E. Verification
### F. Residual Risks
### G. Next Recommendation
```

