# Mode-Specific Section Heuristics

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/topHeavySectionTuningPlan_2026-03-30.md`
- `docs/plans/modeSpecificSectionHeuristicsPlan_2026-03-30.md`
- `docs/plans/unifiedMemoryPolicyThresholdTuningPlan_2026-03-30.md`
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- mode / budget / threshold 관련 helper 파일
- TracePanel 관련 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 단계에서 mode별 heuristic 분리가 필요한지
- Lite / Standard / Full이 section 해상도 차이까지 가져야 하는 이유가 무엇인지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- Lite / Standard / Full / Auto가 section inclusion과 해상도 수준에서 더 분명히 다른 context assembly profile이 되게 하라

구현 범위:
1. mode별 section policy를 정리하라
2. section별 full / summary / excerpt / reference 해상도를 mode별로 차등 적용하라
3. Lite는 더 집중되고, Full은 더 풍부하지만 여전히 noise를 억제하는 방향으로 조정하라
4. Auto가 현재 조건에 따라 어느 profile에 가까운지 설명 가능한 규칙을 만들거나 정리하라
5. 가능하면 Trace에서 mode 때문에 달라진 section behavior를 읽을 수 있게 하라

비목표:
- vector retrieval
- 새로운 memory source 추가
- per-section 사용자 수동 설정 UI
- mode 체계 전면 재설계

구현 원칙:
- mode는 단순 budget bucket이 아니라 context assembly profile이다
- structured memory 우선 원칙은 유지한다
- Lite는 더 적게가 아니라 더 집중되게
- Full은 더 많이가 아니라 더 풍부하되 여전히 agent-friendly하게

검증:
- `cargo check`
- 가능하면 unit test 추가
- Lite / Standard / Full 차이가 실제 section 해상도에서 드러나는지 확인
- Trace에서 mode 차이를 읽을 수 있는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Mode Policy
### E. Verification
### F. Residual Gaps
