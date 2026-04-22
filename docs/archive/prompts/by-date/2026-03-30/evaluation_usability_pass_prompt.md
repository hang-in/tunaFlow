# Evaluation Usability Pass

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/evaluationRunCreationUiPlan_2026-03-30.md`
- `docs/plans/evaluationRunExecutionLinkagePlan_2026-03-30.md`
- `docs/plans/evaluationRunExecutionRealWiringPlan_2026-03-30.md`
- `docs/plans/evaluationUsabilityPassPlan_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 지금은 새 평가 기능보다 usability pass가 먼저인지

이번 작업 목표:
- Evaluation을 반복 실행 가능한 실제 도구처럼 느껴지게 만드는 최소 사용성 보강을 구현하라.

구현 범위:
1. 실행 중 `Cancel`
2. 완료/실패 후 `Retry`
3. failed / empty / partial 상태 표시 개선
4. 가능하면 결과 복사 또는 forward 액션 추가
5. 가능하면 participant 구성 재확인/최소 편집

비목표:
- scoring
- judge
- rubric
- benchmark dashboard

검증:
- tsc --noEmit
- 필요 시 cargo check
- run cancel/retry 흐름 확인
- 결과 활용 액션 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. UX Changes
### D. Verification
### E. Residual Gaps
