# Git Branch Link Visibility Plan

상태: 제안
작성: 2026-03-30

## 목표

프로젝트 레벨 git awareness 다음 단계로, 각 tunaFlow branch가 어떤 git branch와 연결되는지 사용자에게 보이게 한다.

## 현재 상태

- 프로젝트 선택기에서 현재 git branch / dirty 상태는 표시 가능
- `branches.git_branch` 필드는 이미 존재
- 하지만 UI에서는 branch별 git 연결이 거의 보이지 않는다

## 범위

### 표시

- branch row 또는 branch detail에 linked git branch badge 표시
- 연결이 없으면 비어 있음을 명확히 구분

### 입력

- branch 생성 시 `git_branch` 기본값 제안 또는 수동 입력
- branch 편집 시 `git_branch` 수정 가능

## 비목표

- git checkout
- git branch 자동 생성/삭제
- merge/rebase/cherry-pick
- worktree

## 권장 UX

### 목록

- branch label 옆에 작은 git badge
- 예: `b1  ↔ feature/jwt-auth`

### detail/edit

- `Linked Git Branch`
- read/write 텍스트 필드 또는 간단 edit affordance

## 성공 기준

- 사용자가 각 tunaFlow branch의 linked git branch를 볼 수 있다
- 필요하면 수동으로 연결/수정할 수 있다
- git automation 없이도 branch 작업 맥락이 더 명확해진다

## 메모

이번 단계는 여전히 awareness다. 실제 git branch를 만들거나 전환하는 것은 후속이다.
