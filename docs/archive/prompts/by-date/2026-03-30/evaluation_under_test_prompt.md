# Evaluation Under Test

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/evaluationUiConnectionPlan_2026-03-30.md`
- `docs/plans/evaluationUnderTestPlan_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 Evaluation을 메인 탭보다 `Test` 하위로 두는 것이 현재 IA에 더 맞는지

이번 작업 목표:
- 이미 연결된 Evaluation UI가 있다면 메인 탭에서 빼고, `Test` 탭 하위의 서브 뷰로 재배치하라.

구현 범위:
1. 메인 탭 `Eval` 제거
2. `Test` 내부에 `Reports / Evaluation` 서브 뷰 추가 또는 동등 구조로 재배치
3. evaluation run 목록/상세/refresh/create 기능 유지

비목표:
- evaluation backend 변경
- 평가 기능 제거
- scoring/rubric 도입

검증:
- `Test` 탭 안에서 evaluation run 접근 가능
- 메인 탭 수 감소
- 기존 evaluation UI 기능 유지
- `tsc --noEmit`

출력 형식:
### A. Opinion
### B. Files Changed
### C. New IA
### D. Verification
### E. Residual Gaps
