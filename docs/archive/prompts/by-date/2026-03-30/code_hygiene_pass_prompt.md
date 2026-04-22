# Code Hygiene Pass

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/codeHygienePassPlan_2026-03-30.md`
- `docs/reference/implementationStatus.md`

먼저 확인할 파일:
- 최근 리팩토링 후 dead code 후보 UI 파일
- `smoke-sidebar`, `smoke-workspace` 테스트 파일
- 관련 mock/store fixture 파일

작업 시작 전 짧게 의견을 말하라:
- 지금 dead code 정리와 smoke test 복구를 함께 하는 것이 왜 적절한지

이번 작업 목표:
- 최근 구조 변경 이후 남은 dead code를 정리하고, 깨진 smoke test를 현재 구조에 맞게 복구하라.

구현 범위:
1. dead code 후보 파일/참조를 실제 사용 여부 기준으로 정리
2. `smoke-sidebar`, `smoke-workspace`를 새 구조에 맞게 수정
3. obsolete import/mock/store expectation을 함께 정리

비목표:
- 신규 기능 추가
- token/cost DB 확장
- Knowledge Sources 구현

검증:
- `tsc --noEmit`
- 관련 smoke test 실행
- 가능하면 현재 실패 테스트가 모두 통과하는지 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Dead Code Removed
### D. Test Fixes
### E. Verification
### F. Residual Gaps
