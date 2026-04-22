# Git Branch Defaulting Plan

상태: 제안
작성: 2026-03-30

## 목표

이미 표시/수정 가능한 `git_branch` 메타를 branch 생성 시점부터 더 자연스럽게 채우도록 기본값 제안 규칙을 붙인다.

## 현재 상태

- 프로젝트 선택기에서 현재 git branch/dirty 상태 표시 가능
- branch row/drawer에서 linked git branch 표시 가능
- drawer에서 `git_branch` 수동 연결/편집 가능

## 문제

- 지금은 사용자가 branch를 만든 뒤 다시 들어가서 git branch를 수동 입력해야 한다
- branch 생성 시점의 git 맥락이 손실되기 쉽다

## 범위

### 생성 시 기본값

- branch 생성 시 현재 프로젝트 git branch를 기본값 후보로 제안
- 부모 branch에 `git_branch`가 있으면 이를 우선 상속할지, 현재 프로젝트 branch를 쓸지 명확한 규칙 결정

권장 규칙:
- 부모 branch에 linked git branch가 있으면 우선 상속
- 없으면 현재 프로젝트 git branch를 기본값 후보로 사용

### 편집 유지

- 사용자는 생성 후 계속 수동 수정 가능

## 비목표

- git branch 자동 생성
- git checkout
- git branch rename 연동

## 성공 기준

- 새 branch를 만들면 `git_branch`가 빈 값으로 시작하지 않는다
- 기본값은 사용자가 이해할 수 있는 규칙으로 채워진다
- 이후 수동 수정 흐름과 충돌하지 않는다

## 메모

이 단계는 여전히 awareness/polish 범위다. 실제 git automation은 후속이다.
