# Git Sync Phase 2 Guarded Actions Plan

상태: 제안
작성: 2026-03-30

## 목표

Git sync Phase 1의 awareness를 넘어, 제한적이고 안전한 git 조작만 tunaFlow에서 수행할 수 있게 한다.

## 전제

- 프로젝트 git branch/dirty 상태 가시화 완료
- branch별 `git_branch` 표시/수정/기본값 제안 완료
- 아직 실제 git automation은 없음

## 이번 단계의 원칙

- destructive action 금지
- merge/rebase/cherry-pick 금지
- branch create / checkout 정도의 작은 액션만 허용
- 사용자가 명시적으로 눌러야 동작

## 권장 범위

### Action 1. Create linked git branch

- tunaFlow branch에 `git_branch`가 설정되어 있고 실제 git branch가 없으면 생성 가능
- 생성 전 현재 base branch를 보여준다

### Action 2. Checkout linked git branch

- linked git branch가 있으면 checkout 가능
- dirty 상태면 경고하거나 차단

## 비목표

- merge
- delete
- rebase
- worktree
- multi-branch orchestration

## UX 방향

- drawer 헤더 또는 branch detail에서 action 제공
- 예:
  - `Create Git Branch`
  - `Checkout`
- dirty workspace면 destructive하지 않은 안내를 먼저 보여준다

## 성공 기준

- 사용자가 linked git branch를 실제로 생성할 수 있다
- 사용자가 linked git branch로 전환할 수 있다
- dirty 상태에서 무리한 전환이 일어나지 않는다
- adopt와 merge 개념은 여전히 분리된다

## 메모

이 단계는 git automation의 시작이지만, 아직 안전한 범위로 제한해야 한다. branch visibility가 끝났다고 바로 merge까지 가면 제품이 너무 빨리 위험해진다.
