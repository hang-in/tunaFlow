# Evaluation Run Creation UI

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/evaluationUiConnectionPlan_2026-03-30.md`
- `docs/plans/evaluationUnderTestPlan_2026-03-30.md`
- `docs/plans/evaluationRunCreationUiPlan_2026-03-30.md`

먼저 확인할 파일:
- evaluation 관련 backend command 파일
- `Test > Evaluation` 관련 frontend UI 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 단계에서 scoring보다 run 생성 UI가 먼저인지

이번 작업 목표:
- `Test > Evaluation` 화면에서 새 evaluation run을 직접 생성할 수 있게 하라.

구현 범위:
1. `New Run` 진입 버튼
2. 최소 생성 폼
   - title
   - prompt
   - mode
   - rounds
   - participants
3. 생성 후 목록 갱신 + 자동 선택
4. 빈 결과 상태 처리

비목표:
- auto execution orchestration
- scoring/rubric
- template library

검증:
- tsc --noEmit
- 필요 시 cargo check
- run 생성 후 목록/상세 갱신 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Creation Flow
### D. Verification
### E. Residual Gaps
