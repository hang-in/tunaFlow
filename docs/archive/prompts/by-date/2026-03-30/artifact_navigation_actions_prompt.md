# Artifact Navigation Actions

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/artifactDetailViewPlan_2026-03-30.md`
- `docs/plans/artifactProvenanceWorkflowPlan_2026-03-30.md`
- `docs/plans/artifactNavigationActionsPlan_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- provenance를 표시만 하는 것과 실제 navigation entry point로 쓰는 것의 차이
- 이번 단계에서 search보다 navigation이 왜 우선인지

이번 작업 목표:
- Artifacts 탭과 상세 모달의 provenance/workflow 정보를 실제 탐색 액션으로 연결하라.

구현 범위:
1. artifact 카드 provenance line에서 source conversation/branch/RT를 클릭 가능하게 만들 것
2. artifact 상세 모달 Source 행에서도 같은 이동을 제공할 것
3. `subtaskId`가 있으면 Plans 탭의 해당 subtask로 이동하는 액션을 추가할 것
4. `Forward` 액션은 현재 의미가 더 잘 보이도록 최소한의 라벨/툴팁 보강을 할 것

비목표:
- artifact 검색
- artifact graph
- artifact 자동 승격
- artifact → plan 자동 생성
- cross-project 이동

출력 형식:
### A. Opinion
### B. Files Changed
### C. Navigation Flow
### D. Verification
### E. Residual Gaps
