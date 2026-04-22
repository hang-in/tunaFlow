# Memory Section Budget Breakdown

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/unifiedMemoryPolicyThresholdTuningPlan_2026-03-30.md`
- `docs/plans/memoryPolicyTraceSurfacePlan_2026-03-30.md`
- `docs/plans/memorySectionBudgetBreakdownPlan_2026-03-30.md`
- `docs/plans/contextPackVisibilityUiPolishPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- `src-tauri/src/commands/context_pack.rs`
- `src/components/tunaflow/context-panel/TracePanel.tsx`
- `src/components/tunaflow/RuntimeStatusBar.tsx`

작업 시작 전 짧게 의견을 말하라:
- 왜 active/skipped만으로는 98k input 같은 케이스를 설명할 수 없는지
- 왜 section별 budget breakdown이 지금 필요한지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- ContextPack 각 section의 budget 기여도를 Trace/Runtime에서 읽을 수 있게 하라

구현 범위:
1. ContextPack 조립 시 section별 chars 길이 메타를 기록하라
2. 가능하면 approximate tokens도 계산하되, 어렵다면 chars 우선으로 끝내라
3. TracePanel에서 top budget consumers 3~5개를 보여줘라
4. oversized section은 active/skipped와 별개로 드러나게 하라
5. RuntimeStatusBar에는 최소 힌트만 추가하라

비목표:
- token accounting 전면 재작성
- section hard cap 재설계
- per-section budget 조정 UI
- vector retrieval

구현 원칙:
- policy 변경이 아니라 observability 강화 단계다
- Trace는 상세, StatusBar는 최소 요약
- chars 기준이 먼저고, token 근사는 후속이어도 된다

검증:
- `cargo check`
- `tsc --noEmit`
- `vite build`
- 큰 input run에서 어떤 section이 가장 budget을 먹는지 Trace에서 바로 보이는지 확인
- StatusBar가 과하게 시끄러워지지 않는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Budget Breakdown
### E. Verification
### F. Residual Gaps
