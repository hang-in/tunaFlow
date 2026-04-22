# Context Stack Reevaluation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/contextStackReevaluationPlan_2026-03-30.md`
- `docs/plans/contextBudgetScalingPlan.md`
- `docs/plans/contextPackTraceabilityPlan.md`
- `docs/plans/contextHubSidecarIntegrationPlan_2026-03-29.md`
- `docs/plans/chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 `ContextPack / context-hub / flow agent`의 순서를 다시 정해야 하는지

이번 작업 목표:
- `ContextPack`, `context-hub`, `flow agent` 세 축을 다시 평가해 다음 구현 라운드의 우선순위를 좁혀라.

작업 요구사항:
1. 세 축을 각각 `제품 가치 / 선행 조건 / 리스크` 기준으로 평가할 것
2. `P0 / P1 / Hold`로 좁힐 것
3. placeholder 위험이 있는 항목은 명확히 낮출 것
4. 바로 다음 구현 라운드에서 무엇을 먼저 해야 하는지 1개만 고를 것

비목표:
- 실제 기능 구현
- 문서 대량 생성
- Knowledge Sources shell 재추진

출력 형식:
### A. Opinion
### B. Comparison
### C. Priority Decision
### D. Recommended Next Round
### E. Risks
