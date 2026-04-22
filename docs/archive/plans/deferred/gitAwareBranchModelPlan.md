# Git 연동을 고려한 브랜치 모델 설계

작성자: OpenAI Codex  
작성일: 2026-03-26

## 목적

현재 `tunaFlow`의 브랜치를 단순 대화 분기가 아니라, 향후 git branch와 연결될 수 있는 `작업 브랜치(work branch)` 개념으로 확장 가능하게 설계한다.

## 핵심 판단

브랜치는 앞으로 다음 두 역할을 동시에 가질 가능성이 높다.

1. 대화/작업 흐름의 분기
2. 실제 git 작업 단위의 연결점

따라서 지금부터 브랜치를 "UI용 대화 탭" 정도로 좁게 정의하면 안 된다.

## 현재 상태

현재 스키마의 `branches`에는 이미 `git_branch` 컬럼이 있다.

즉 구조상 git 연동을 받을 준비는 어느 정도 되어 있다.

현재 핵심 필드:

- `id`
- `conversation_id`
- `label`
- `status`
- `checkpoint_id`
- `parent_branch_id`
- `session_id`
- `git_branch`

## 장기 방향

브랜치는 아래처럼 이해하는 것이 자연스럽다.

- 프로젝트 종속
- conversation에서 시작되거나 다른 branch에서 파생
- plan / artifacts / memos / roundtable의 작업 단위
- 필요 시 실제 git branch와 연결

즉 branch는 "대화 분기"이면서도 "작업 공간"이다.

## 권장 확장 필드

장기적으로 있으면 좋은 필드:

- `git_branch`
- `git_base_branch`
- `git_commit_at_fork`
- `git_sync_status`
- `linked_worktree_path`

이번 단계에서 전부 넣을 필요는 없지만, 현재 branch 로직은 이 확장을 막지 않아야 한다.

## UX 방향

현재:

- 메시지에서 branch 생성
- branch stream 열기

미래:

- 메시지에서 작업 브랜치 생성
- 옵션:
  - 대화 브랜치만 생성
  - git branch도 같이 생성
- 브랜치 안에서:
  - 대화
  - plan
  - artifact
  - roundtable
  - 필요 시 실제 파일 작업

## adopt / merge 개념

현재 `adopted`는 "대화 결과 채택" 의미에 가깝다.

나중에 git merge가 들어오면 개념 충돌을 피해야 한다.

권장:

- `adopt` = 대화/결론 채택
- `merge` = git 변경 병합

즉 용어를 지금부터 분리하는 것이 좋다.

## 프로젝트와의 관계

브랜치는 프로젝트 종속 개념이어야 한다.

프로젝트 컨텍스트가 앞으로 제공해야 하는 정보:

- git repo 여부
- git root
- current branch
- default base branch

이 정보가 있어야 나중에 branch 생성 시 git 연결 여부를 판단할 수 있다.

## 완료 기준

장기 목표 기준으로 아래가 중요하다.

1. branch는 프로젝트 종속 작업 단위로 유지된다
2. `git_branch`는 장식 필드가 아니라 실제 연결 지점으로 취급된다
3. adopt와 merge의 의미가 혼동되지 않는다
4. 향후 git 연동 시 현재 branch 모델을 뒤엎지 않아도 된다

