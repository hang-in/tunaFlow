# context-hub Explicit Handoff

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/reference/contextHubSourcePolicy_2026-03-30.md`
- `docs/plans/contextHubMinimalIntegrationPlan_2026-03-30.md`
- `docs/plans/contextHubExplicitHandoffPlan_2026-03-30.md`

먼저 확인할 파일:
- context-hub search/get UI 파일
- 현재 message/artifact handoff 관련 UI 파일
- trace/message meta 관련 표시 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 auto injection보다 explicit handoff가 먼저인지
- 현재 tunaFlow IA에서 어떤 표면이 가장 자연스러운 handoff 지점인지

이번 작업 목표:
- `context-hub`에서 조회한 문서를 사용자가 명시적으로 현재 작업 흐름에 넘길 수 있게 하라.

구현 범위:
1. 문서 미리보기 액션:
   - `Copy`
   - `Send to Current Context`
   - 가능하면 `Save as Artifact`
2. handoff된 문서가 현재 대화/입력 흐름에 연결되게 할 것
3. 전달된 문서가 최소한 message meta 또는 trace에 남게 할 것

비목표:
- auto ContextPack injection
- background fetch
- flow agent 자동 선택
- public source fetch

검증:
- `tsc --noEmit`
- 필요 시 `cargo check`
- 검색 → get → explicit handoff 흐름 확인
- 전달된 문서 흔적이 남는지 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Handoff Flow
### D. Verification
### E. Residual Gaps
