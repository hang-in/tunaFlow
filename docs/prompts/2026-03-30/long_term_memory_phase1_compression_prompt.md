# Long-Term Memory Phase 1 Compression

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/plans/longTermMemoryPhase1CompressionPlan_2026-03-30.md`
- `docs/explanation/agentscopeAnalysis.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/compression.rs`
- `src-tauri/src/commands/agents_helpers/context_queries.rs`
- `src-tauri/src/commands/send_common.rs`
- 관련 schema/model 파일

skill 지시:
- 관련 skill이 있더라도 현재 작업에 직접 필요한 것만 최소로 사용하라
- 과도한 skill 로딩 금지
- 이번 작업의 핵심은 long-term memory compression 구조이므로, 구현 품질에 직접 기여하는 skill만 고려하라

작업 시작 전 짧게 의견을 말하라:
- 왜 recent window를 단순히 10~12개로 늘리는 것으로는 부족한지
- 왜 compressed conversation memory가 지금 필요한지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- recent window 밖으로 밀린 오래된 대화를 구조화 요약 memory로 유지하는 최소 long-term memory compression 경로를 구현하라

구현 범위:
1. 오래된 conversation context를 구조화 요약으로 압축하는 최소 데이터 구조를 설계하라
2. recent messages를 대체하지 않고, 별도 `compressed memory` source로 ContextPack에 붙여라
3. 요약 형식은 최소한 아래를 포함하도록 하라:
   - Task Overview
   - Current State
   - Important Discoveries
   - Decisions
   - Open Questions
   - Context to Preserve
4. 원본 메시지는 계속 유지하고, compressed memory는 continuity 보조층으로만 사용하라
5. trace/runtime에서 compressed memory 포함 여부를 최소 수준으로 확인 가능하게 하라

비목표:
- vector retrieval 도입
- mem0/ReMe 같은 외부 long-term memory stack 도입
- generic memory OS 구축
- project 전역 semantic graph
- artifact/plan 시스템 재설계

구현 원칙:
- AgentScope 전체를 들여오지 말고 memory compression 패턴만 차용하라
- recent window를 늘리는 단순 대응으로 대체하지 말라
- artifact/plan은 structured memory source로 남기고, compressed memory와 역할을 섞지 말라
- DB를 과하게 뒤집지 말고, 현재 구조에 자연스럽게 얹는 최소 경로를 우선하라

검증:
- `cargo check`
- `tsc --noEmit`
- 가능하면 긴 대화/멀티에이전트 대화 시나리오에서 recent window 밖 맥락이 요약 memory로 유지되는지 확인
- compressed memory가 ContextPack metadata/trace에서 구분되는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Compression Memory Design
### E. Verification
### F. Residual Gaps
