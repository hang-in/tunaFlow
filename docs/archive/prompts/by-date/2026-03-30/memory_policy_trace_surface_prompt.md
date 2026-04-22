# Memory Policy Trace Surface

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/unifiedMemoryPolicyPhase1Plan_2026-03-30.md`
- `docs/plans/unifiedMemoryPolicyThresholdTuningPlan_2026-03-30.md`
- `docs/plans/memoryPolicyTraceSurfacePlan_2026-03-30.md`
- `docs/plans/contextPackVisibilityUiPolishPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src/components/tunaflow/context-panel/TracePanel.tsx`
- `src/components/tunaflow/RuntimeStatusBar.tsx`
- memory policy metadata가 조립되는 Rust 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 단계에서 새 policy보다 trace surface가 중요한지
- 어떤 memory 정보가 사람 눈에 바로 보여야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- Trace/Runtime surface에서 memory policy 결과를 더 직접 읽게 만들어라

구현 범위:
1. TracePanel에 active/skipped memory layer 요약을 추가하라
2. retrieval/compressed memory skip 이유를 짧고 읽기 쉬운 문구로 노출하라
3. 가능하면 applied budget bucket, retrieval threshold, compressed threshold를 Trace에서 읽게 하라
4. RuntimeStatusBar에는 너무 시끄럽지 않게 최소 memory hint만 추가하라
5. memory layer 이름을 UI/로그/메타에서 더 일관되게 정리하라

비목표:
- threshold 재설계
- vector retrieval
- memory policy 직접 조정 UI
- 새 memory layer 추가

구현 원칙:
- 새 정책을 만드는 단계가 아니라, 기존 정책을 읽기 쉽게 드러내는 단계다
- TracePanel은 설명 가능성 중심
- StatusBar는 최소 신호만
- 지나치게 verbose한 개발자용 디버그 문자열을 그대로 노출하지 말라

검증:
- `cargo check`
- `tsc --noEmit`
- `vite build`
- TracePanel에서 memory layer 포함/스킵/이유가 더 읽기 쉬워졌는지 확인
- RuntimeStatusBar가 과하게 시끄러워지지 않았는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Trace Surface
### E. Verification
### F. Residual Gaps
