# Git Sync Phase 1

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/gitAwareBranchModelPlan.md`
- `docs/plans/gitSyncBranchModelPlan_2026-03-29.md`
- `docs/plans/gitSyncPhase1Plan_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/branches.rs`
- `src-tauri/src/db/models.rs`
- `src-tauri/src/db/schema.rs`
- 프로젝트/브랜치 관련 frontend UI 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 단계에서 git automation보다 git awareness가 먼저인지
- `adopt`와 `merge`를 왜 계속 분리해야 하는지

이번 작업 목표:
- 실제 git 조작 자동화 없이, 프로젝트와 branch의 git 상태/연결 정보를 사용자에게 보여주는 Phase 1을 구현하라.

구현 범위:
1. 현재 프로젝트의 git repo 여부 / current branch / dirty 상태 조회
2. UI에 git branch/dirty 상태 표시
3. branch의 `git_branch` 메타 표시 또는 최소 편집 경로 제공

비목표:
- git checkout 자동화
- git branch 생성/삭제 자동화
- merge/rebase/cherry-pick
- worktree

검증:
- cargo check
- tsc --noEmit
- git repo 프로젝트에서 branch/dirty 상태가 보이는지 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Git Awareness Flow
### D. Verification
### E. Residual Gaps
