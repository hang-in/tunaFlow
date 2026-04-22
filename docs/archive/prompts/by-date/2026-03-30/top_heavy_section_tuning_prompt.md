# Top Heavy Section Tuning

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/memorySectionBudgetBreakdownPlan_2026-03-30.md`
- `docs/plans/topHeavySectionTuningPlan_2026-03-30.md`
- `docs/plans/unifiedMemoryPolicyThresholdTuningPlan_2026-03-30.md`
- `docs/plans/contextPackAlgorithmPhase1Plan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- section별 formatter/helper 파일
- TracePanel 관련 파일

작업 시작 전 짧게 의견을 말하라:
- 최근 trace 기준 top heavy section이 무엇인지
- 왜 지금은 policy보다 section별 targeted tuning이 중요한지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- top heavy section 2~3개를 골라, agent 품질 손실이 적은 방식으로 budget 기여도를 줄여라

구현 범위:
1. 실제 trace 기준 상위 budget consumer section 2~3개를 식별하라
2. 각 section에 맞는 축소 전략을 적용하라
   - artifacts: summary/excerpt 중심
   - rawq: 해상도 재조정
   - cross-session: 더 공격적 fold
   - findings/retrieval: 중복/저신호 축소
   중 실제 상위 section에 맞게 선택
3. full / summary / excerpt / reference 해상도 규칙을 section별로 더 명확히 하라
4. 변경 후 top consumers와 총 input이 실제로 줄었는지 확인하라

비목표:
- memory policy 전체 재설계
- vector retrieval
- per-section 사용자 설정 UI
- token accounting 전면 재작성

구현 원칙:
- 실제로 무거운 section부터 다뤄라
- 모든 section을 똑같이 줄이지 말라
- agent가 작업하는 데 필요한 핵심 정보는 유지하고, 반복/장식/저신호 정보부터 줄여라

검증:
- `cargo check`
- 가능하면 관련 unit test 추가
- trace에서 top heavy section 기여도가 줄었는지 확인
- 큰 input run에서 총 input이 내려갔는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Heavy Sections
### E. Verification
### F. Residual Gaps
