# Conversation Retrieval Chunking

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/conversationRetrievalPhase1Plan_2026-03-30.md`
- `docs/plans/conversationRetrievalChunkingPlan_2026-03-30.md`
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/context_queries.rs`
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- retrieval 관련 schema/model/helper 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 message 단건 retrieval이 부족한지
- 왜 pair/chunk 단위가 지금 필요한지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- retrieval 결과를 message 단건이 아니라 질문+응답 또는 의미 단위 chunk로 재조립해 `Relevant prior conversation` 품질을 올려라

구현 범위:
1. 최소 chunk 규칙을 정의하라:
   - user + 직후 assistant = pair chunk
   - branch anchor = anchor chunk
   - RT brief = brief chunk
2. FTS5 hit가 어느 메시지에 걸리든 최종 retrieval 결과는 pair/chunk 단위로 재구성하라
3. 같은 pair/chunk가 중복 hit로 반복되면 dedup 하라
4. recent window와 겹치는 pair/chunk는 제외하라
5. 가능하면 `Relevant prior conversation` 섹션에서 kind(pair/anchor/brief)를 최소 수준으로 드러내라

비목표:
- vector embedding 도입
- sqlite-vec
- retrieval learning/reranking
- project 밖 retrieval

구현 원칙:
- 검색 엔진은 당장 FTS5를 그대로 써도 된다
- 핵심은 retrieval 결과 단위 개선이다
- recent / compressed / structured memory와 역할 충돌을 만들지 말라

검증:
- `cargo check`
- `tsc --noEmit`
- 가능하면 질문만/응답만 걸리던 시나리오가 pair/chunk로 회수되는지 확인
- `Relevant prior conversation`이 이전보다 읽기 쉬워졌는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Chunking Rule
### E. Verification
### F. Residual Gaps
