# Auto Mode Heuristic Polish

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/modeSpecificSectionHeuristicsPlan_2026-03-30.md`
- `docs/plans/autoModeHeuristicPolishPlan_2026-03-30.md`
- `docs/plans/unifiedMemoryPolicyThresholdTuningPlan_2026-03-30.md`
- `docs/reference/projectFirstEntryPolicy_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- Trace/Runtime 관련 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 최종 기본 mode는 Auto여야 하는지
- Lite/Standard/Full이 좋아졌어도 Auto가 설명 가능해야 하는 이유가 무엇인지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- Auto mode가 현재 요청 맥락을 보고 Lite / Standard / Full 중 하나에 가까운 profile을 설명 가능하게 선택하도록 다듬어라

구현 범위:
1. Auto 판단 입력 신호를 정리하라
   - prompt 길이
   - handoff source
   - structured memory 존재
   - retrieval hit
   - branch/RT 여부
   등 설명 가능한 입력만 사용
2. Auto → Lite/Standard/Full 매핑 규칙을 보강하라
3. Trace에서 실제 선택 결과를 읽게 하라
4. Runtime surface에는 최소한의 표시만 추가하라

비목표:
- 사용자 mode UI 재설계
- mode 추가
- vector retrieval
- memory priority 재설계

구현 원칙:
- Auto는 black box가 아니라 설명 가능한 규칙이어야 한다
- 기본은 Standard에 가깝되 필요할 때만 Lite/Full로 이동하게 하라
- 사용자가 mode를 자주 손대지 않아도 되게 하는 것이 목표다

검증:
- `cargo check`
- 가능하면 unit test 추가
- 짧은 follow-up / 일반 작업 / 복합 handoff 작업에서 Auto가 다른 profile을 고르는지 확인
- Trace에서 Auto 선택 결과가 보이는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Auto Heuristic
### E. Verification
### F. Residual Gaps
