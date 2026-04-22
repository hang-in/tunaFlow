# Conversation Retrieval Ranking Polish

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/conversationRetrievalPhase1Plan_2026-03-30.md`
- `docs/plans/conversationRetrievalChunkingPlan_2026-03-30.md`
- `docs/plans/conversationRetrievalRankingPolishPlan_2026-03-30.md`
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/context_queries.rs`
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- retrieval 관련 helper/model 파일

작업 시작 전 짧게 의견을 말하라:
- 현재 retrieval chunking 이후 남은 문제는 무엇인지
- 왜 ranking + dedup이 지금 단계에서 필요한지
- 왜 이번 단계에서 vector/embedding을 넣지 말아야 하는지

이번 작업 목표:
- `Relevant prior conversation`의 품질을 높이기 위해 retrieval chunk를 더 잘 정렬하고, 중복과 겹침을 줄여라

구현 범위:
1. chunk 단위 점수화 규칙을 추가하라
   - query hit 수
   - recency
   - kind 가중치
   - overlap penalty
   정도의 설명 가능한 규칙 기반 휴리스틱이면 충분하다
2. 같은 pair/anchor/brief 중복을 제거하라
3. 높은 유사도의 chunk는 Jaccard 등 가벼운 규칙으로 dedup 하라
4. recent context와 강하게 겹치는 retrieval chunk는 down-rank 또는 제외하라
5. current plan / findings / artifacts / compressed memory와 겹치는 retrieval chunk도 과하게 반복되지 않게 하라
6. 초기 hit 집합을 더 넓게 잡더라도 최종 ContextPack에는 top 3~5 정도만 남기도록 재선택하라

비목표:
- vector embedding 도입
- sqlite-vec
- semantic reranker 모델
- cross-project retrieval
- ML 기반 ranking

구현 원칙:
- 현재 FTS5 + chunk retrieval 구조를 유지하라
- ranking은 설명 가능한 규칙 기반이어야 한다
- retrieval이 structured memory보다 앞서는 인상을 만들지 말라
- retrieval은 “새로운 관련 기억”을 가져와야지, 이미 위 섹션에 있는 내용을 반복하면 안 된다

검증:
- `cargo check`
- retrieval 결과가 이전보다 덜 반복되는지 확인
- recent/structured/compressed memory와 겹치는 retrieval이 줄었는지 확인
- 가능하면 간단한 unit test를 추가해 ranking/dedup 규칙을 보호하라

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Ranking Rule
### E. Verification
### F. Residual Gaps
