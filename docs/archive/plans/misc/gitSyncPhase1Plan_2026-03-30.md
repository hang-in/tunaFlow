# Git Sync Phase 1 Plan

상태: 제안
작성: 2026-03-30

## 목표

tunaFlow의 branch/conversation 구조를 실제 Git 작업 맥락과 느슨하게 연결해,
사용자가 현재 어떤 git 상태 위에서 작업 중인지 이해할 수 있게 만든다.

## 핵심 원칙

- `adopt`는 여전히 대화/결론 채택이다
- `merge`는 git 병합이다
- 이번 단계는 git 조작 자동화가 아니라 **가시화 + 메타 연결**이 목적이다

## 현재 전제

- `branches.git_branch` 필드가 이미 존재한다
- 실제 git 연동 로직은 없다
- 프로젝트는 로컬 경로를 알고 있고, 일부는 git repo일 수 있다

## 범위

### Project 레벨

- 현재 프로젝트가 git repo인지 감지
- git root / current branch / dirty 상태 조회

### Branch 레벨

- tunaFlow branch에 `git_branch` 메타를 연결/표시
- 새 branch 생성 시 `git branch linked` 여부를 선택하거나, 최소한 수동 연결 가능하게 함

### UI 레벨

- 프로젝트/브랜치 화면에서 현재 git branch와 dirty 상태를 확인
- branch detail 또는 badge 수준으로 `git_branch` 연결 여부 표시

## 비목표

- git checkout 자동 실행
- git branch 생성/삭제 자동화
- adopt 시 git merge
- commit/rebase/cherry-pick
- worktree 생성

## 권장 구현

### Phase 1A

- backend command:
  - git repo 여부
  - current branch
  - dirty 여부
  - git root

### Phase 1B

- frontend visibility:
  - Sidebar 또는 project header에 git branch/dirty 표시
  - branch row/detail에 linked git branch 표시

### Phase 1C

- branch 생성/편집 시 `git_branch` 메타 입력 또는 자동 기본값 제안

## 성공 기준

- 사용자가 현재 프로젝트의 git branch와 dirty 상태를 볼 수 있다
- tunaFlow branch가 어떤 git branch와 연결돼 있는지 알 수 있다
- adopt와 merge 개념이 UI/용어에서 혼동되지 않는다

## 메모

이 단계는 git automation이 아니라 git awareness다. 실제 git 명령을 크게 늘리기 전에, 현재 작업 맥락을 오해 없이 보여주는 것이 우선이다.
