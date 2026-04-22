# Conversation Retrieval Phase 1

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/conversationVectorSearchPlan.md`
- `docs/plans/conversationRetrievalPhase1Plan_2026-03-30.md`
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/context_queries.rs`
- `src-tauri/src/commands/send_common.rs`
- 관련 schema/model 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 이제 retrieval이 필요한지
- rawq / recent window / compressed memory와 retrieval의 역할 차이가 무엇인지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- 프로젝트 범위 안에서 현재 질문과 의미적으로 연관된 과거 대화 chunk를 회수해 ContextPack에 붙이는 최소 retrieval 실험을 구현하라

구현 범위:
1. 메시지 단건이 아니라 turn/chunk 단위 구조를 우선 검토하고, 가능한 최소 구현으로 시작하라
2. current project scoped retrieval만 허용하라
3. retrieval 결과를 `Relevant prior conversation` 같은 별도 섹션으로 recent context 뒤에 붙여라
4. recent window와 중복되는 chunk는 제외하라
5. top 3~5 chunk 정도의 작은 범위로 제한하라

비목표:
- sqlite-vec 최적화
- 외부 vector DB
- project 밖 retrieval
- generic semantic memory engine
- retrieval importance 학습

구현 원칙:
- recent window를 대체하지 말라
- rawq와 문제를 섞지 말라
- compressed memory와 retrieval의 역할을 구분하라
- 먼저 작은 실험으로 noise와 품질을 확인하라

검증:
- `cargo check`
- `tsc --noEmit`
- 가능하면 recent window 밖의 관련 대화를 실제로 회수하는 시나리오를 최소 1회 이상 확인
- retrieval 결과가 과도한 noise를 만들지 않는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Retrieval Design
### E. Verification
### F. Residual Gaps
