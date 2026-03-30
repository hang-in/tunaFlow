# Git Sync Phase 2 Guarded Actions

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/gitSyncPhase1Plan_2026-03-30.md`
- `docs/plans/gitBranchLinkVisibilityPlan_2026-03-30.md`
- `docs/plans/gitBranchDefaultingPlan_2026-03-30.md`
- `docs/plans/gitSyncPhase2GuardedActionsPlan_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 merge/rebase보다 guarded branch create/checkout이 먼저인지
- dirty workspace에서 어떤 보호 장치가 필요한지

이번 작업 목표:
- tunaFlow에서 linked git branch에 대해 제한적인 안전 액션만 제공하라.

구현 범위:
1. linked git branch 생성 액션
2. linked git branch checkout 액션
3. dirty 상태 보호 또는 경고

비목표:
- merge
- delete
- rebase
- worktree
- 자동 adopt↔merge 연결

검증:
- cargo check
- tsc --noEmit
- clean repo에서 create/checkout 동작 확인
- dirty repo에서 보호 장치 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Action Flow
### D. Verification
### E. Residual Gaps
