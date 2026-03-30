# context-hub Minimal Integration

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/contextHubSidecarIntegrationPlan_2026-03-29.md`
- `docs/plans/chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29.md`
- `docs/reference/contextHubSourcePolicy_2026-03-30.md`
- `docs/plans/contextHubMinimalIntegrationPlan_2026-03-30.md`

먼저 확인할 파일:
- `context-hub` 관련 기존 계획 문서
- Runtime/diagnostics 관련 UI 파일
- sidecar/CLI 호출 관련 Rust command 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금은 Knowledge Sources shell보다 minimal integration path가 먼저인지
- 왜 health/search/get까지만 여는 것이 적절한지

이번 작업 목표:
- `context-hub`를 실제 공급층으로 붙이는 최소 연동을 구현하라.

구현 범위:
1. health check
2. 허용 source 범위 내 search
3. result get
4. source policy 위반 차단

비목표:
- Knowledge Sources shell
- public source 자동 조회
- auto ContextPack injection
- flow agent

검증:
- `cargo check`
- `tsc --noEmit`
- health/search/get 최소 경로 확인
- public auto-fetch가 기본 동작이 아님을 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Integration Flow
### D. Verification
### E. Residual Gaps
