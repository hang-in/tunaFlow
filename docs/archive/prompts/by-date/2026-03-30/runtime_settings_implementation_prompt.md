# tunaFlow Runtime Settings 우선 구현 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Runtime Settings Implementation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/runtimeSettingsImplementationPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/settingsShellIaPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/rawqRequiredSidecarPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/contextBudgetScalingPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/implementationStatus.md`

우선 짧게 의견부터 말하라:
- 왜 지금 `Knowledge Sources`보다 `Runtime` 실제화를 먼저 하는 것이 제품적으로 더 맞는지
- Runtime에서 지금 당장 구현된 것과 placeholder로 남겨야 할 것을 어떻게 나눌지
- 이번 단계에서 context-hub나 flow agent로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. `Settings > Runtime`을 실제 진단/설정 섹션으로 만든다
2. rawq / Model Catalog / Context Budget / Background/Daemon을 분리된 카드나 섹션으로 보여준다
3. 사용자가 최소한 몇 가지 runtime 상태를 실제로 확인하거나 새로고침할 수 있게 한다

## 권장 방향

- 이미 존재하는 상태와 command를 최대한 재사용한다
- 구현된 것은 실제 상태로 보여주고, 미구현은 과장하지 않는다
- placeholder 문구 대신 product-like status cards를 만든다

## 수정 대상 후보

- `src/components/tunaflow/SettingsPanel.tsx`
- 관련 runtime status helper / selector
- 필요 시 rawq/model catalog invoke 연결

## 중요

- 이번 단계는 Runtime 우선 구현이다
- Knowledge Sources 추가 금지
- context-hub 연동 금지
- flow agent 구현 금지
- Settings 전체 재설계 금지

## 검증

- `tsc --noEmit`
- 필요 시 `cargo check`
- Runtime 섹션에서 실제로 무엇을 볼 수 있고 무엇을 할 수 있는지 설명
- rawq / model catalog / context budget / daemon이 어떻게 구분되어 보이는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Runtime UX Model
### E. Verification
### F. Next Recommendation
```
