# Project-First Startup UX Plan

상태: 중요 / 제안
작성: 2026-03-30

## 배경

`projectFirstEntryPolicy_2026-03-30.md`에서
tunaFlow의 최종 제품은 **프로젝트 선택부터 시작해야 한다**는 원칙을 고정했다.

지금까지의 개발 과정에서는 예외적으로:
- 프로젝트가 없는 상태에서
- 테스트 대화가 먼저 시작되는 경우가 있었고
- 그 결과 memory policy / retrieval / budget 수치가 정상 사용 시나리오를 대표하지 못했다.

즉 다음 단계는 이 원칙을 실제 시작 UX로 옮기는 것이다.

## 목표

앱 시작 시 프로젝트가 선택되지 않았다면,
일반 작업 화면이 아니라 `프로젝트 선택 / 추가 / 최근 프로젝트 진입` 중심의 startup UX를 먼저 보여준다.

핵심은:
- projectless chat 시작을 정상 경로에서 제거
- 사용자가 항상 project-scoped state에서 진입하게 하는 것

이다.

## 왜 필요한가

### 1. tunaFlow는 범용 채팅 앱이 아니다

tunaFlow의 작업 단위는 기본적으로:
- project
- branch
- artifact
- retrieval scope
- git state

위에서 정의된다.

프로젝트 없는 시작은 제품 개념과 어긋난다.

### 2. memory / retrieval / rawq / context-hub 품질이 project scope에 의존한다

프로젝트가 선택되어야:
- rawq 검색 범위
- conversation retrieval 범위
- plan/artifact relevance
- git awareness

가 의미를 가진다.

### 3. 최종 dogfood와 품질 해석의 기준을 세울 수 있다

나중에 dogfood test를 시작할 때도,
project-first entry가 있어야 정상 사용 흐름 기준으로 검증할 수 있다.

## 이번 단계에서 할 것

### 1. startup state 정의

최소 상태:
- no project selected
- recent projects available
- add/open project

### 2. project-first startup 화면 또는 overlay 도입

조건:
- 프로젝트 미선택 상태에서는 chat/plan/artifacts/test 메인 workflow를 바로 열지 않는다
- 대신 project selector / onboarding surface를 먼저 보여준다

### 3. 기존 Sidebar/Workspace와 충돌 없는 진입 흐름 정리

이미 있는 프로젝트 선택기와 관계를 정리한다.

원칙:
- startup 진입점은 project-first
- 선택 이후에는 기존 Sidebar selector가 전환/추가 진입점 역할

### 4. projectless path 정리

개발 중 예외 경로는 남아 있더라도,
최종 사용자 흐름에서는 projectless 대화 시작이 불가능하게 만든다.

## 이번 단계에서 하지 않을 것

- dogfood test project 운영 시작
- onboarding 카피 대량 작성
- 프로젝트 import 마법사 확장
- workspace 전체 IA 재설계

## 구현 원칙

- 시작 UX는 단순해야 한다
- project를 고른 뒤에만 agent workflow가 열려야 한다
- 기존 메인 workflow는 유지하되, 진입 순서만 바로잡는다
- projectless state를 정상 제품 모드처럼 보이게 하지 말라

## 성공 기준

- 첫 실행 또는 미선택 상태에서 project selector/onboarding이 먼저 보인다
- 프로젝트를 고르기 전에는 일반 chat workflow가 시작되지 않는다
- 프로젝트 선택 후 기존 workflow로 자연스럽게 진입한다

## 후속

이 단계 다음은:

1. recent projects polish
2. import/open project UX 보강
3. 충분히 안정화된 뒤 dogfood test project 운영 시작

순으로 이어진다.
