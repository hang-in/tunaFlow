# Evaluation Run Execution Real Wiring

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/evaluationRunExecutionLinkagePlan_2026-03-30.md`
- `docs/plans/evaluationRunExecutionRealWiringPlan_2026-03-30.md`

먼저 확인할 파일:
- evaluation 관련 frontend/backend 실행 코드
- 현재 agent 실행 경로
- agent profile/model/persona/default skills 연결 코드

작업 시작 전 짧게 의견을 말하라:
- 왜 placeholder execution으로는 evaluation이 아직 완성되지 않았는지
- 왜 1차는 sequential mode + 기존 실행 경로 재사용이 맞는지

이번 작업 목표:
- evaluation run execution skeleton을 실제 agent 실행 경로와 연결하라.

구현 범위:
1. agent profile 기준 실제 실행 경로 호출
2. 실제 응답을 `eval_results`에 저장
3. run status를 실행 결과에 맞게 갱신
4. 상세 뷰에 실제 결과 반영

비목표:
- scoring/judge
- distributed execution
- 대규모 orchestration 재설계

검증:
- tsc --noEmit
- 필요 시 cargo check
- 실제 eval run 실행 후 결과가 저장/표시되는지 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Real Execution Flow
### D. Verification
### E. Residual Gaps
