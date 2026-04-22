# context-hub Source Policy

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/contextHubSidecarIntegrationPlan_2026-03-29.md`
- `docs/plans/chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29.md`
- `docs/reference/contextHubSourcePolicy_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 `model network access`와 `knowledge source network access`를 분리해 봐야 하는지

이번 작업 목표:
- 향후 context-hub 연동 작업에서 공개 레포/공개 registry 자동 접근이 기본 동작이 되지 않도록, source 정책을 기준 문서로 반영하라.

핵심 정책:
1. `context-hub`는 sidecar/CLI/MCP로 붙인다
2. 기본은 `bundled/local/private only`
3. public source 자동 조회/fetch는 금지

비목표:
- context-hub 실제 구현
- Knowledge Sources shell 재도입
- remote sync 기능 추가

출력 형식:
### A. Opinion
### B. Policy Summary
### C. Allowed Sources
### D. Disallowed Defaults
### E. Follow-up Constraints
