# Evaluation Run Execution Linkage

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/evaluationUiConnectionPlan_2026-03-30.md`
- `docs/plans/evaluationRunCreationUiPlan_2026-03-30.md`
- `docs/plans/evaluationRunExecutionLinkagePlan_2026-03-30.md`

먼저 확인할 파일:
- evaluation backend command 파일
- 현재 agent 실행 경로
- `Test > Evaluation` 관련 frontend UI 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 scoring보다 run execution linkage가 먼저인지
- 1차는 왜 sequential mode 우선이 맞는지

이번 작업 목표:
- 생성된 evaluation run이 실제 agent 실행 결과로 채워지도록 최소 실행 연결을 구현하라.

구현 범위:
1. `Run` 또는 `Execute` 액션
2. 선택된 participant/engine에 prompt 전송
3. 결과를 `eval_results`에 저장
4. run status 갱신
5. 상세 뷰 실시간 또는 실행 후 갱신

비목표:
- scoring/rubric
- auto judge
- 대규모 orchestration 재설계
- distributed execution

검증:
- tsc --noEmit
- 필요 시 cargo check
- run 생성 → 실행 → 결과 저장 → 상세 비교 흐름 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Execution Flow
### D. Verification
### E. Residual Gaps
