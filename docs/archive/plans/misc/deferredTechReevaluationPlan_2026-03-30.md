# Deferred Tech Reevaluation Plan

상태: 제안
작성: 2026-03-30

## 목표

최근 제품 중심축 정리 이후, 보류 중이던 기술과 새로 검토할 기술 후보를 다시 분류해 다음 구현 라운드의 우선순위를 재설정한다.

## 왜 지금 필요한가

- `Agent Profile / Persona / Runtime / Artifacts / Search / Git sync`의 1차 축이 정리됐다
- 초기 보류 판단은 과거 구조 기준일 수 있다
- 새 기술 후보를 바로 구현하기보다, 현재 제품 구조에 맞는지 다시 평가해야 한다

## 평가 기준

### 1. 제품 가치

- 사용자가 바로 체감하는가
- 현재 워크플로를 더 명확하게 만드는가

### 2. 구조 적합성

- 지금 tunaFlow IA와 잘 맞는가
- 기존 `Agent / Persona / Artifacts / Runtime / Git` 축과 충돌하지 않는가

### 3. 구현 리스크

- 범위가 과도하게 커지지 않는가
- 기존 안정화한 기능을 흔들지 않는가

### 4. 선행 조건

- 이미 필요한 셸/데이터/메타가 준비됐는가
- 먼저 풀어야 할 기술 부채가 남아 있는가

## 분류 방식

- `P0`: 지금 바로 다음 구현 라운드로 올릴 후보
- `P1`: 선행 조건 일부 충족 후 진행
- `P2`: 아이디어는 좋지만 아직 이름만 유지
- `Hold`: 현재 구조나 시점상 보류 유지

## 추천 검토 대상

- ContextPack 고도화
- Context budget scaling
- context-hub / Knowledge Sources
- flow agent 고도화
- evaluation 확장
- chat virtualization
- code-review-graph / rawq 후속
- plugin adoption 후속

## 산출물

1. 후보 기술 목록
2. 각 후보의 현재 가치/리스크/선행 조건
3. `P0 / P1 / P2 / Hold` 재분류
4. 다음 2~3개 구현 라운드 추천

## 메모

이 작업은 새 기능 구현이 아니라, 다음 기술 라운드를 잘 고르기 위한 제품/기술 포트폴리오 정리다.
