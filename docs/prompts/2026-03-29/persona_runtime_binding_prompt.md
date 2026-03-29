# tunaFlow Persona Runtime Binding 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Persona Runtime Binding

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/personaRuntimeBindingPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/how-to/tunaflow_persona_baseline_6.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfileChatInputBindingPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/engineFeatureParityClassificationPlan.md`

우선 짧게 의견부터 말하라:
- persona를 기존 prompt 조립 경로에 어떤 section 형태로 넣는 것이 가장 안전한지
- applied persona를 사용자에게 어디서 보여주는 것이 가장 덜 거슬리고 가장 이해하기 쉬운지
- 이번 단계에서 persona editor 확장이나 추천 로직으로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. 선택된 persona의 `promptFragment`가 실제 runtime prompt에 반영된다
2. 4개 엔진 모두 동일한 persona section 개념을 공유한다
3. 사용자는 현재 요청에 어떤 persona가 적용됐는지 최소 한 곳에서 확인할 수 있다

## 권장 방향

- persona는 최종 system prompt 전체를 대체하지 말고
  `Persona` 또는 `Role Contract` section으로 삽입한다
- 기존 normalized prompt / provider별 조립 흐름을 최대한 재사용한다
- frontend에는 applied persona를 가볍게 표시한다

## 수정 대상 후보

- `src-tauri/src/commands/agents_helpers/*`
- provider prompt 조립 관련 Rust 파일
- persona store / input binding 경로
- `NewMessageInput.tsx` 또는 message/trace meta 표시 UI

## 중요

- 이번 단계는 runtime binding이다
- persona editor 확장 금지
- auto persona recommendation 금지
- auto skill selection 금지
- Agent Profile UI 재설계 금지

## 검증

- `cargo check`
- `tsc --noEmit`
- 선택 persona가 prompt 조립 경로에 어떻게 포함되는지 설명
- 4개 엔진 모두 persona 개념이 유지되는지 설명
- 사용자가 applied persona를 어디서 확인할 수 있게 되었는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Runtime Binding Model
### E. Verification
### F. Next Recommendation
```
