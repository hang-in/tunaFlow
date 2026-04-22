# Persona CLI Validation Plan

상태: 제안
작성: 2026-03-30

## 목표

Claude CLI를 직접 호출해 `General / Reviewer / Tester` persona가 실제 응답 차이를 만드는지 확인한다.

## 왜 지금 CLI인가

- prompt 조립만 보는 unit test는 `persona section이 들어갔는지`만 검증한다
- 지금 필요한 것은 실제 응답 차이 검증이다
- 사용자가 API 비용을 허용했으므로, 이번 단계에서는 CLI 기반 실응답 검증이 가장 직접적이다

## 범위

### Track A

- 동일한 질문을 `General / Reviewer / Tester` persona로 각각 3회 실행
- 가능하면 Claude CLI 기준으로 먼저 검증
- 비교 항목:
  - tone
  - output structure
  - task focus
  - first-paragraph priority

### Track B

- full UI handoff 대신, 현재 코드 경로와 truncation 제약을 함께 기록
- 필요하면 긴 artifact 본문을 handoff payload 형태로 수동 시뮬레이션해 reviewer/tester 반응을 확인
- 다만 이것은 앱 UI end-to-end 검증이 아니라 보조 검증으로 분리 표기

## 권장 공통 입력

`GraphQL API에 JWT 인증을 붙이는 방향을 제안해줘. 구현 순서와 주의점을 간단히 정리해줘.`

## 성공 기준

- `General / Reviewer / Tester` 사이에 반복 가능한 차이가 관찰된다
- persona fragment가 단순 설정이 아니라 실제 출력 행동에 영향을 준다고 판단할 수 있다
- reviewer/tester가 긴 handoff 본문을 못 보는 문제는 별도 handoff 제약으로 분리 정리된다

## 메모

이번 검증은 제품 품질 판단용이다. 구현 변경이 필요 없으면 문서화로 끝낼 수 있고, 차이가 약하면 persona fragment 보강 또는 prompt 조립 규칙 재조정이 다음 단계가 된다.
