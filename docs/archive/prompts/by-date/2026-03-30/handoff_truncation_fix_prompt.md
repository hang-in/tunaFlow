# Handoff Truncation Fix

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/personaVsHandoffValidationPlan_2026-03-30.md`
- `docs/plans/handoffTruncationFixPlan_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents.rs`
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- `src/components/tunaflow/context-panel/ArtifactsPanel.tsx`
- `src/components/tunaflow/chat/MessageActions.tsx`

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 truncation 이슈를 먼저 고쳐야 하는지
- 단순 상향과 artifact 전용 handoff 중 어떤 방식이 더 맞는지

이번 작업 목표:
- 긴 artifact/message를 다음 agent로 넘길 때 800자 truncation 때문에 맥락이 깨지지 않도록 handoff 경로를 보강하라.

권장 방향:
- 일반 followup은 보호적으로 유지
- artifact handoff는 전문 또는 더 큰 상한을 쓰는 별도 경로 우선

비목표:
- context pack 전체 재설계
- vector retrieval
- Knowledge Sources 구현

검증:
- 긴 artifact를 handoff했을 때 reviewer/tester가 실제 본문을 참조하는지 확인
- cargo check
- tsc --noEmit

출력 형식:
### A. Opinion
### B. Files Changed
### C. Handoff Path
### D. Verification
### E. Residual Risks
