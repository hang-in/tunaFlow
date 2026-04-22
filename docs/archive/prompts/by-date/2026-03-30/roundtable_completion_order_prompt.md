# Roundtable Deliberative Completion-Order

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/roundtableCompletionOrderPlan_2026-03-30.md`
- `docs/plans/threadModelRoundtableRedesign.md`
- `docs/plans/masterTestPlan.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/roundtable_helpers/executor.rs`
- `src-tauri/src/commands/roundtable.rs`
- `src-tauri/src/commands/roundtable_helpers/persist.rs`
- RT 관련 테스트 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 현재 Deliberative 구현이 straggler 병목인지
- 왜 이번 단계는 completion-order만 고치고 다른 RT 설계는 건드리지 말아야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- Deliberative 모드에서 participant 결과를 배열 순서가 아니라 실제 완료 순서대로 collect/persist/emit 하라

구현 범위:
1. participant subprocess fan-out은 유지하라
2. 결과 수집을 participant order 기반 `join`이 아니라 completion-order 기반으로 바꿔라
3. `roundtable:participant_status`, `roundtable:progress`, DB persist가 실제 완료 순서를 반영하게 하라
4. Deliberative prompt semantics는 유지하라
   - same-round peer context 없음
   - current_round_refs 비어 있음
5. 가능하면 관련 테스트를 추가하라

비목표:
- Sequential 재설계
- blind verifier phase
- role-based output cap
- lead decomposition

구현 원칙:
- Deliberative의 의미는 유지하고 reduce 병목만 제거하라
- 완료 순서가 곧 UI/DB 반영 순서가 되게 하라
- participant 배열 순서는 config 의미만 유지하고, 표시 순서를 강제하지 않게 하라

검증:
- `cargo check`
- 가능하면 `cargo test`
- Deliberative에서 빠른 participant가 먼저 표시되는지 설명하라
- prompt semantics가 바뀌지 않았는지 확인하라

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Completion-Order Flow
### E. Verification
### F. Residual Gaps
