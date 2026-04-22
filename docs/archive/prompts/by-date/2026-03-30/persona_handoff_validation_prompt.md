# Persona and Handoff Validation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/personaBehaviorValidationPlan_2026-03-30.md`
- `docs/plans/personaVsHandoffValidationPlan_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 persona 검증과 handoff 검증을 분리해야 하는지

이번 작업 목표:
- `persona validation`과 `handoff validation`을 분리해서 다시 수행하고 결과를 정리하라.

## Track A. Persona Validation

1. 동일한 질문을 `General / Reviewer / Tester`에 각각 독립 실행
2. 아래를 비교
   - tone
   - output structure
   - task focus
3. 가능하면 4개 엔진 중 최소 2개 이상에서 확인

권장 공통 입력:
`GraphQL API에 JWT 인증을 붙이는 방향을 제안해줘. 구현 순서와 주의점을 간단히 정리해줘.`

## Track B. Handoff Validation

1. 한 persona가 산출물을 만든다
2. 그 결과를 artifact 또는 명시적 source로 넘긴다
3. 다른 persona가 그 산출물을 실제로 참조하는지 본다

중요:
- 자동 인용이 안 되면 실패로만 쓰지 말고, 현재 제품에서 필요한 명시적 handoff 방식으로 해석할 것

비목표:
- 새 persona 추가
- editor 확장
- prompt 시스템 리팩토링

출력 형식:
### A. Opinion
### B. Track A Setup
### C. Track A Results
### D. Track B Setup
### E. Track B Results
### F. Recommendation
