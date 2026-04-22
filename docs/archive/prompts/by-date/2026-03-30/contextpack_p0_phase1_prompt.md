# ContextPack P0 Phase 1

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/contextStackReevaluationPlan_2026-03-30.md`
- `docs/plans/contextPackTraceabilityPlan.md`
- `docs/plans/contextBudgetScalingPlan.md`
- `docs/plans/contextPackP0Phase1Plan_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/compression.rs`
- `src-tauri/src/commands/agents_helpers/trace_log.rs`
- 관련 trace/runtime/frontend 표시 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 ContextPack P0에서 budget UI보다 section visibility와 compression이 먼저인지

이번 작업 목표:
- ContextPack P0 1차로 section visibility/traceability와 compression 가시화를 먼저 구현하라.

구현 범위:
1. 실제 포함된 ContextPack section을 trace/runtime surface에 표시
2. compression/truncation이 발생했는지 더 명확히 드러나게 보강
3. 이후 budget scaling UI를 붙일 수 있는 계측/가시화 기반을 만든다

비목표:
- context-hub 연동
- flow agent
- budget slider 직접 노출
- vector retrieval

검증:
- cargo check
- tsc --noEmit
- 각 엔진 실행 후 section inclusion이 보이는지 확인
- compression/truncation 발생 시 표시가 남는지 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. ContextPack Visibility Flow
### D. Verification
### E. Residual Gaps
