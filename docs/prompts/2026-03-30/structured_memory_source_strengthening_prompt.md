# Structured Memory Source Strengthening

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/plans/compressedMemoryOperationalPolishPlan_2026-03-30.md`
- `docs/plans/structuredMemorySourceStrengtheningPlan_2026-03-30.md`
- `docs/plans/threadContextInheritancePlan.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/context_queries.rs`
- `src-tauri/src/commands/send_common.rs`
- `src-tauri/src/commands/plans.rs`
- `src-tauri/src/commands/artifacts.rs`
- `src-tauri/src/commands/memos.rs`

작업 시작 전 짧게 의견을 말하라:
- 왜 compressed memory 다음 단계가 structured memory 강화인지
- artifact/plan/findings/memo/cross-session의 역할을 왜 더 분리해야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- ContextPack에서 structured memory source를 더 명확하고 예측 가능하게 사용하도록 우선순위와 inclusion policy를 보강하라

구현 범위:
1. 현재 plan / findings / artifacts / memo / cross-session inclusion 경로를 먼저 확인하라
2. 아래 원칙을 가능한 범위에서 반영하라:
   - explicit source 우선
   - current plan / active subtask 우선
   - findings 우선
   - relevant recent artifacts 우선
   - compressed memory는 continuity 보조층
   - memo는 lightweight pin/reference
   - cross-session은 relevance가 약하면 뒤로
3. artifact / findings / memo / cross-session이 서로 어떤 역할인지 Trace/Runtime/metadata 관점에서도 더 헷갈리지 않게 하라
4. 가능하면 inclusion policy가 실제로 달라졌는지 최소 검증 시나리오를 남겨라

비목표:
- vector retrieval
- memo 시스템 전면 재설계
- artifact editor 대형 확장
- plan schema 재설계
- knowledge graph

구현 원칙:
- “대화 요약”보다 “현재 작업에 직접 연결된 구조화 source”를 우선하라
- compressed memory와 structured memory의 역할을 섞지 말라
- 기존 artifact/plan/memo UI를 크게 흔들지 말고 ContextPack/assembly 정책 중심으로 보강하라

검증:
- `cargo check`
- `tsc --noEmit`
- 가능하면 current task / artifact / findings가 compressed memory보다 우선 반영되는지 확인
- trace/runtime 메타가 더 설명 가능해졌는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Structured Memory Policy
### E. Verification
### F. Residual Gaps
