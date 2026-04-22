# tunaFlow Knowledge Sources Settings Shell 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Knowledge Sources Settings Shell MVP

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/knowledgeSourcesSettingsShellPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/settingsSkillsKnowledgeSourcesPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/contextHubSidecarIntegrationPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29.md`

우선 짧게 의견부터 말하라:
- 왜 지금 단계에서 실제 context-hub 연동보다 `Knowledge Sources` shell이 먼저 필요한지
- `Skills`와 `Knowledge Sources`의 차이를 UI에서 어떻게 가장 짧고 명확하게 보여줄지
- 이번 단계에서 flow agent나 context-hub health check로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. Settings 좌측 nav에 `Knowledge Sources` 섹션 추가
2. `Skills`와 `Knowledge Sources`의 차이를 사용자에게 명확히 설명
3. 향후 context-hub / fetched docs / flow agent가 들어갈 자리를 shell 형태로 확보

## 권장 방향

- 구현된 기능처럼 보이지 않게 shell/placeholder 중심으로 만든다
- `local skills` 와 `external sources`의 차이를 설명하는 copy가 중요하다
- 정보 구조가 우선이고 실제 integration은 다음 단계로 남긴다

## 수정 대상 후보

- Settings 관련 컴포넌트
- settings nav
- Skills section 설명 텍스트

## 중요

- 이번 단계는 product shell이다
- context-hub 실제 연동 금지
- CLI health check 금지
- fetched docs runtime 적용 금지
- flow agent 구현 금지

## 검증

- `tsc --noEmit`
- Settings에 `Knowledge Sources`가 새 섹션으로 보이는지
- `Skills`와의 차이가 UI에서 읽히는지 설명
- 다음 단계로 무엇이 붙을 수 있게 되었는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. UX Model
### E. Verification
### F. Next Recommendation
```
