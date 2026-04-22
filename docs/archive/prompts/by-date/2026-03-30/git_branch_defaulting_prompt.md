# Git Branch Defaulting

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/gitSyncPhase1Plan_2026-03-30.md`
- `docs/plans/gitBranchLinkVisibilityPlan_2026-03-30.md`
- `docs/plans/gitBranchDefaultingPlan_2026-03-30.md`

먼저 확인할 파일:
- branch 생성 관련 backend/frontend 코드
- 현재 git status/project metadata 연결 코드
- `git_branch` 편집 UI 코드

작업 시작 전 짧게 의견을 말하라:
- branch 생성 시 어떤 `git_branch` 기본값 규칙이 가장 자연스러운지

이번 작업 목표:
- 새 branch 생성 시 `git_branch` 기본값을 제안/채워 넣어, 이후 수동 편집 부담을 줄여라.

권장 규칙:
1. 부모 branch에 `git_branch`가 있으면 우선 상속
2. 없으면 현재 프로젝트 git branch를 기본값 후보로 사용

비목표:
- git branch 자동 생성
- git checkout
- git rename 연동

검증:
- 새 branch 생성 시 `git_branch`가 기본값으로 채워지는지 확인
- 기존 수동 편집 흐름이 유지되는지 확인
- `tsc --noEmit`
- 필요 시 `cargo check`

출력 형식:
### A. Opinion
### B. Files Changed
### C. Defaulting Rule
### D. Verification
### E. Residual Gaps
