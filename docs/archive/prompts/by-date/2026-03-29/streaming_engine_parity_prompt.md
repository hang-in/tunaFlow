# tunaFlow Streaming Engine Parity 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Streaming Engine Parity

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/engineFeatureParityClassificationPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/streamingEngineParityPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/implementationStatus.md`

우선 짧게 의견부터 말하라:
- Codex/OpenCode streaming parity를 맞추는 가장 현실적인 방법
- 이번 단계에서 native streaming을 꼭 강제할지, synthetic streaming을 허용할지
- 이번에 같이 묶어야 할 것과 묶지 말아야 할 것

그 다음 실제 작업을 진행하라.

## 현재 상태

- Claude: streaming 지원
- Gemini: streaming 지원
- Codex: one-shot 중심
- OpenCode: one-shot 중심

이 상태는 4-engine parity 기준에서 미완료다.

## 목표

모든 엔진에서 최소한 아래 UX/state contract를 동일하게 제공하라.

1. placeholder assistant message
2. partial content 또는 partial-like progress 업데이트
3. cancel 동작
4. completed / failed 상태 전이

## 권장 방향

1. provider-native streaming이 있으면 우선 사용
2. 없으면 synthetic streaming 또는 incremental relay 허용
3. frontend는 엔진별 차이를 최대한 숨기고 동일 event contract를 받게 정리

## 수정 대상 후보

- `src-tauri/src/commands/agents.rs`
- `src-tauri/src/agents/codex.rs`
- `src-tauri/src/agents/opencode.rs`
- 관련 frontend runtime/event handling 코드
- 필요 시 `docs/reference/implementationStatus.md`

## 중요

- 이번 세션은 streaming parity만 다룬다
- token/cost tracking parity로 범위를 넓히지 말 것
- resume/continuation parity로 범위를 넓히지 말 것
- context pack 재작업으로 돌아가지 말 것
- 무관한 UI 재설계 금지

## 검증

- `cargo check`
- 가능하면 streaming 경로별 수동 검증 또는 최소 테스트
- Codex/OpenCode가 어떤 방식으로든 partial state를 emit하는지 설명
- cancel / failure / completion 상태가 기존 UI와 맞는지 확인

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Streaming Strategy by Provider
### E. Verification
### F. Residual Risks
### G. Next Recommendation
```

