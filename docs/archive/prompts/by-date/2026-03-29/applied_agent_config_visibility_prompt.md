# tunaFlow Applied Agent Config Visibility 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Applied Agent Config Visibility

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/appliedAgentConfigVisibilityPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/personaRuntimeBindingPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentProfileChatInputBindingPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/tracePanelRuntimeFirstPlan_2026-03-29.md`

우선 짧게 의견부터 말하라:
- applied profile/persona/skills를 어디에 보여주는 것이 가장 자연스러운지
- 왜 1차 위치는 trace보다 assistant message meta가 더 맞는지
- 이번 단계에서 DB 확장이나 full audit log로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. assistant message surface에서 applied profile / persona / skills를 재확인할 수 있게 한다
2. 표시 정보가 전송 시점 기준 snapshot을 반영하도록 한다
3. trace/runtime에는 보조 확인용 연결만 남긴다

## 권장 방향

- 메인 위치는 assistant message meta
- 표시 정보는 compact하게 유지
- 최소 항목은 `profile / persona / skills count`
- engine/model은 이미 충분히 보이면 생략 가능

## 수정 대상 후보

- assistant message 렌더링 컴포넌트
- message 타입/메타 경로
- runtime send path에서 applied config snapshot 전달 경로
- 필요 시 TracePanel의 보조 badge

## 중요

- 이번 단계는 visibility다
- flow agent explainability 전체 구현 금지
- applied docs visibility 동시 도입 금지
- DB 대규모 스키마 변경 금지
- Settings/Agent/Profile UI 재설계 금지

## 검증

- `tsc --noEmit`
- 필요 시 `cargo check`
- assistant message에서 applied profile/persona/skills 확인 가능 여부
- 현재 store 상태와 무관하게 전송 시점 기준으로 보이는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Visibility Model
### E. Verification
### F. Next Recommendation
```
