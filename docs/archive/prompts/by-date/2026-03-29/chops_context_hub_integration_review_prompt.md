# chops + context-hub 통합 검토 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow chops + context-hub Integration Review

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/contextHubSidecarIntegrationPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/settingsSkillsKnowledgeSourcesPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/agentSkillPersonaIaPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/codexProjectReference_2026-03-29.md`

외부 참고 레포:
- `/Users/d9ng/privateProject/_research/_util/chops`
- `/Users/d9ng/privateProject/_research/_util/context-hub`

우선 짧게 의견부터 말하라:
- chops / context-hub / tunaFlow의 역할 분리가 현실적인지
- 지금 당장 도입해도 되는 최소 범위가 무엇인지
- flow agent 고도화 전에 반드시 먼저 정리해야 할 것이 무엇인지

그 다음 실제 검토를 진행하라.

## 목표

1. chops를 관리층, context-hub를 공급층, tunaFlow를 적용층으로 두는 구조가 타당한지 검토
2. Settings > Skills / Knowledge Sources 분리가 맞는지 판단
3. flow agent 고도화 전에 어떤 선행 작업이 필요한지 정리

## 중요

- 이번 단계는 review / planning이다
- 아직 context-hub를 제품 코드에 붙이지 말 것
- chops 앱 전체 복제 제안 금지
- flow agent 구현으로 바로 새지 말 것

## 반드시 다룰 것

1. local installed skills 와 fetched docs/source의 구분
2. CLI vs MCP 도입 순서
3. applied skills / applied docs visibility 필요성
4. flow agent가 나중에 어디에 붙는지

## 결과 보고 형식

### A. Opinion
### B. What Fits
### C. What Should Be Deferred
### D. Pre-Flow-Agent Prerequisites
### E. Recommendation
```
