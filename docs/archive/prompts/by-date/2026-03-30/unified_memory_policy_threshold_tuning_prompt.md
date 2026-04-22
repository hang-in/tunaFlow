# Unified Memory Policy Threshold Tuning

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/plans/unifiedMemoryPolicyPhase1Plan_2026-03-30.md`
- `docs/plans/unifiedMemoryPolicyThresholdTuningPlan_2026-03-30.md`
- `docs/plans/conversationRetrievalRankingPolishPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- `src-tauri/src/commands/context_pack.rs`
- trace/context meta 관련 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 threshold tuning이 필요한지
- 현재 `4000 / 2000` cutoff의 장단점이 무엇인지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- retrieval / compressed memory inclusion threshold를 현재 사용 패턴에 더 잘 맞게 조정하라

구현 범위:
1. retrieval/compressed memory included/skipped 빈도와 이유를 더 읽기 쉽게 계측하라
2. `remaining > 4000` retrieval cutoff를 검토하고, 필요하면 더 적절한 규칙으로 조정하라
3. `remaining > 2000` compressed memory cutoff도 검토하고 조정하라
4. 가능하면 hardcoded cutoff를 상수/주석/규칙 수준으로 더 읽기 쉽게 정리하라
5. structured memory 우선 원칙은 유지하라

비목표:
- vector embedding
- 새로운 memory layer
- memory source priority 재설계
- 사용자 threshold 조정 UI

구현 원칙:
- policy correction이지 architecture rewrite가 아니다
- retrieval / compressed memory는 여전히 보조층이다
- 항상 더 많이 넣는 것이 아니라, 도움이 되는 상황에서 더 잘 살아남게 만드는 것이 목적이다

검증:
- `cargo check`
- 가능하면 관련 unit test 추가
- trace/meta에서 skip/include 이유가 더 설명 가능해졌는지 확인
- retrieval / compressed memory가 적절한 상황에서 더 잘 포함되는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Threshold Rule
### E. Verification
### F. Residual Gaps
