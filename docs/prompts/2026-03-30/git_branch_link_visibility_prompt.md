# Git Branch Link Visibility

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/gitSyncPhase1Plan_2026-03-30.md`
- `docs/plans/gitBranchLinkVisibilityPlan_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/branches.rs`
- branch 관련 frontend row/detail/create/edit UI 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 git automation보다 branch-level linked git branch visibility가 먼저인지

이번 작업 목표:
- 각 tunaFlow branch의 `git_branch` 연결 상태를 UI에서 보이게 하고, 최소한 수동 편집 가능하게 하라.

구현 범위:
1. branch row/detail에 linked git branch 표시
2. branch 생성/편집 시 `git_branch` 입력 또는 기본값 제안
3. 기존 프로젝트 git awareness와 자연스럽게 이어지게 만들 것

비목표:
- git checkout
- git branch 자동 생성
- merge/rebase/cherry-pick

검증:
- cargo check
- tsc --noEmit
- branch에 linked git branch가 표시/수정되는지 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Branch Git Link UX
### D. Verification
### E. Residual Gaps
