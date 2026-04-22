# ContextPack Algorithm Phase 1

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/contextPackAlgorithmImprovementsPlan.md`
- `docs/plans/contextPackAlgorithmPhase1Plan_2026-03-30.md`
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/compression.rs`
- `src-tauri/src/commands/agents_helpers/rawq.rs`
- `src-tauri/src/commands/agents_helpers/guardrail.rs`

작업 시작 전 짧게 의견을 말하라:
- 왜 retrieval보다 먼저 이 저리스크 알고리즘 개선이 필요한지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- ContextPack 파이프라인을 크게 흔들지 않으면서, 중복 제거/포맷 경량화/rawq 다해상도 표현으로 prompt 품질과 예산 효율을 올려라

구현 범위:
1. cross-session 또는 context summary 계열에서 Jaccard 기반 유사 턴/블록 접기
2. 긴 텍스트 섹션의 마크다운 포맷 경량화
3. rawq snippet에서 import/use/from/require 블록 접기
4. rawq 결과를 최소한 full / skeleton / one-line reference 같은 다해상도 표현으로 바꾸는 경로 추가

비목표:
- Claude compression 경로 제거
- 동적 예산 배분 전면 교체
- 큰 알고리즘 프레임워크 도입
- vector retrieval

구현 원칙:
- 기존 품질을 크게 해치지 않는 저리스크 변경만 하라
- aggressive dedup 금지
- 상위 1~2개 rawq 결과의 full snippet은 최대한 유지하라
- 아이디어는 참고하되 외부 의존성은 추가하지 말라

검증:
- `cargo check`
- `tsc --noEmit`
- 관련 테스트가 있으면 실행
- rawq 섹션 커버리지가 늘어났는지
- cross-session/context summary 중복이 줄었는지
- 전체 prompt 가독성이 크게 깨지지 않는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Applied Algorithms
### E. Verification
### F. Residual Gaps
