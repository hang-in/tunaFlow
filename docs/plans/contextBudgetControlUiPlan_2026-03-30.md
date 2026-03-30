# Context Budget Control UI Plan

상태: 제안
작성: 2026-03-30

## 목표

`Settings > Runtime`에서 현재 ContextPack budget을 읽기만 하는 상태에서 벗어나, 안전한 범위 내에서 조정 가능한 UI를 제공한다.

## 왜 지금 가능한가

- 4-engine context metadata parity 완료
- section visibility/traceability 완료
- compression/truncation 가시화 완료
- rawq 후처리와 typed compression까지 들어가, budget을 조정해도 이유를 설명할 기반이 생김

## 원칙

- 단순 숫자 슬라이더만 두지 않는다
- `mode + total cap + section policy`를 함께 보여준다
- 과도하게 자유로운 조정은 막는다

## 1차 범위

### A. Mode control

- `Lite / Standard / Full`
- 현재 선택 모드 표시
- 각 모드의 성격 설명

### B. Total budget cap

- 현재 총 문자수 한도 표시/조정
- 안전한 범위만 허용

### C. Section policy visibility

- 각 모드에서 어떤 section이 보통 포함되는지 설명
- 현재 trace 기반 실측과 함께 보여주면 좋다

## 비목표

- 완전 자유형 per-section 편집기
- 엔진별 고급 budget 튜닝
- retrieval policy 편집

## 권장 UX

- `Settings > Runtime > Context Budget`
- mode 세그먼트
- total cap control
- 현재 정책 설명 카드
- “advanced”는 아직 만들지 않음

## 성공 기준

- 사용자가 Lite/Standard/Full 의미를 이해할 수 있다
- 안전 범위 안에서 total cap을 조정할 수 있다
- 조정 결과가 trace/runtime에서 확인 가능하다

## 메모

이 단계는 “아무나 튜닝하게 한다”가 아니라, 현재 잘 보이게 된 ContextPack을 안전한 범위에서 조절 가능하게 만드는 것이다.
