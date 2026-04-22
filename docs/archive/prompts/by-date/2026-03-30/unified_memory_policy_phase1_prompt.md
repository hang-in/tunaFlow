# Unified Memory Policy Phase 1

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/plans/structuredMemorySourceStrengtheningPlan_2026-03-30.md`
- `docs/plans/conversationRetrievalPhase1Plan_2026-03-30.md`
- `docs/plans/conversationRetrievalChunkingPlan_2026-03-30.md`
- `docs/plans/conversationRetrievalRankingPolishPlan_2026-03-30.md`
- `docs/plans/unifiedMemoryPolicyPhase1Plan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents_helpers/send_common.rs`
- `src-tauri/src/commands/context_pack.rs`
- retrieval / compressed memory / structured memory 관련 helper 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 단계에서 memory source를 더 추가하는 것보다 policy 통합이 중요한지
- 현재 어떤 충돌이 발생할 수 있는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- ContextPack assembly에서 working / structured / compressed / retrieval memory가 설명 가능한 하나의 정책으로 동작하게 정리하라

구현 범위:
1. memory source priority를 코드 수준에서 더 명확히 고정하라
   - explicit handoff
   - recent context
   - structured memory
   - retrieval
   - compressed memory
   - memo / cross-session
   순서가 기본안이다
2. overlap resolution 규칙을 추가하라
   - structured와 retrieval이 겹치면 structured 우선
   - recent와 retrieval이 겹치면 recent 우선
   - compressed memory는 structured/retrieval과 겹치면 양보
3. budget 부족 시 fallback 순서를 설명 가능하게 정리하라
4. 가능하면 trace/context meta에서 어떤 memory layer가 포함/축소/스킵됐는지 읽을 수 있게 하라

비목표:
- vector embedding
- sqlite-vec
- 외부 long-term memory stack
- 새로운 memory DB
- 대규모 guardrail 재작성

구현 원칙:
- 새 layer를 추가하지 말고 기존 layer selection policy를 정리하라
- 규칙은 설명 가능해야 한다
- 에이전트에게 더 유용한 정보가 우선이며, 사람에게 보기 좋은 긴 설명은 후순위다
- token 절약보다 task relevance가 우선이지만, relevance 없는 반복은 반드시 줄여라

검증:
- `cargo check`
- 가능하면 memory overlap / fallback 관련 unit test 추가
- trace나 meta에서 selection 결과가 더 읽기 쉬워졌는지 확인
- retrieval / structured / compressed memory가 서로 덜 충돌하는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Memory Policy
### E. Verification
### F. Residual Gaps
