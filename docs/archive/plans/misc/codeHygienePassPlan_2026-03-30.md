# Code Hygiene Pass Plan

상태: 제안
작성: 2026-03-30

## 목표

최근 대규모 UI/IA 리팩토링 이후 남아 있는 dead code와 깨진 smoke test를 정리해 코드베이스 신뢰도를 회복한다.

## 범위

### 1. Dead Code 정리

- 더 이상 사용되지 않는 구 UI/패널 컴포넌트 제거
- import/selector/utility 잔재 정리
- 현재 구조와 충돌하는 obsolete 파일 확인

우선 후보:
- `ContextPanel`
- `StatusBar`
- `ChatObjectTabs`
- 관련 테스트 mock/fixture의 구 구조 참조

### 2. Smoke Test 복구

- 현재 알려진 `smoke-sidebar`, `smoke-workspace` 실패 원인 정리
- 새 `CenterPanel / Settings / RuntimeStatusBar / Sidebar hierarchy` 구조에 맞게 mock과 expectation 갱신

## 비목표

- 신규 기능 추가
- 대규모 테스트 프레임워크 교체
- token/cost DB 스키마 확장

## 권장 순서

### Phase 1

- dead code 후보를 실제 참조 기준으로 확인
- 삭제 가능한 것과 아직 테스트에서만 남아 있는 것을 구분

### Phase 2

- smoke test 두 개를 현재 구조에 맞게 수정
- 필요하면 obsolete component 참조 제거

### Phase 3

- 남은 dead code 정리
- build/test 재확인

## 성공 기준

- 현재 구조에서 더 이상 쓰지 않는 주요 UI dead code가 제거된다
- `smoke-sidebar`, `smoke-workspace`가 통과한다
- `tsc --noEmit` 및 관련 테스트가 현재 구조 기준으로 맞춰진다

## 메모

이 단계는 기능 확장보다 코드 정합성 회복이 목적이다. 최근 제품 중심축이 크게 바뀐 상태라, 여기서 테스트를 복구하지 않으면 이후 변경의 회귀 검출력이 약해진다.
