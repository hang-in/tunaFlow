# Persona CLI Validation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/personaBehaviorValidationPlan_2026-03-30.md`
- `docs/plans/personaVsHandoffValidationPlan_2026-03-30.md`
- `docs/plans/personaCliValidationPlan_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 이번 단계에서 unit test보다 CLI 실응답 검증이 더 중요한지

이번 작업 목표:
- Claude CLI를 직접 호출해 `General / Reviewer / Tester` persona의 실제 응답 차이를 검증하고 결과를 정리하라.

## Track A. Persona Response Validation

1. 공통 입력 1개를 정한다
2. `General / Reviewer / Tester` persona 각각으로 3회씩 실행한다
3. 아래를 비교한다
   - tone
   - output structure
   - task focus
   - first-paragraph priority
4. 반복 가능한 차이가 있는지 판단한다

권장 입력:
`GraphQL API에 JWT 인증을 붙이는 방향을 제안해줘. 구현 순서와 주의점을 간단히 정리해줘.`

## Track B. Handoff Note

- 현재 handoff는 코드 경로와 truncation 제약을 함께 정리하라
- 필요하면 긴 artifact 본문을 reviewer/tester에게 수동 시뮬레이션으로 전달해 반응을 확인하되, 이것을 full UI handoff 검증으로 과장하지 말 것

## 주의

- 이번 단계는 persona 차이 검증이 중심이다
- 비용이 발생하더라도 CLI 실응답 비교를 우선한다
- 결과가 약하면 과장하지 말고 약하다고 적을 것

출력 형식:
### A. Opinion
### B. Validation Setup
### C. Results by Persona
### D. Handoff Note
### E. Recommendation
