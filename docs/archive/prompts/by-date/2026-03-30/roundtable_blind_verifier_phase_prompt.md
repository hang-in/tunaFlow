# Roundtable Blind Verifier Phase

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/roundtableCompletionOrderPlan_2026-03-30.md`
- `docs/plans/roundtableBlindVerifierPhasePlan_2026-03-30.md`
- `docs/plans/threadModelRoundtableRedesign.md`
- `docs/plans/masterTestPlan.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/roundtable_helpers/executor.rs`
- `src-tauri/src/commands/roundtable_helpers/prompt.rs`
- `src-tauri/src/commands/roundtable.rs`
- RT participant 타입/UI 관련 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 현재 RT에서 blind verifier가 필요한지
- 왜 participant 순서나 운영 규칙만으로는 sycophancy를 막기 어려운지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- 특정 participant를 `blind verifier`로 지정하면, 그 participant는 다른 participant 응답을 보지 않고 독립 판단을 내리게 하라

구현 범위:
1. participant 또는 RT config에 blind verifier를 표현할 최소 설정을 추가하라
2. blind verifier는 prior/current transcript 없이 topic 중심 prompt를 받게 하라
3. 기존 Sequential/Deliberative semantics는 최대한 유지하라
4. progress/trace/UI 어디든 최소 수준으로 blind verifier 여부를 드러내라
5. 가능하면 관련 테스트를 추가하라

비목표:
- lead decomposition
- role-based output cap
- verifier scoring/judge
- RT 전체 orchestration 재설계

구현 원칙:
- verifier isolation만 최소 확장으로 추가한다
- participant 순서에 기대지 말고 명시적 설정으로 보장한다
- UI에 새 개념을 과하게 노출하지 않는다

검증:
- `cargo check`
- 가능하면 `cargo test`
- blind verifier가 실제로 prior/current transcript를 안 받는지 설명하라
- 기존 non-blind participant 동작은 안 깨졌는지 확인하라

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Blind Verifier Flow
### E. Verification
### F. Residual Gaps
