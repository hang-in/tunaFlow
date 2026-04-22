# Persona Behavior Validation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/how-to/tunaflow_persona_baseline_6.md`
- `docs/plans/personaRuntimeBindingPlan_2026-03-29.md`
- `docs/plans/personaBehaviorValidationPlan_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 현재 persona가 “설정값”을 넘어서 실제 행동 차이를 만드는지 왜 지금 검증해야 하는지

이번 작업 목표:
- `General`, `Reviewer`, `Tester` persona가 실제 응답 차이로 이어지는지 검증하고 결과를 문서화하라.

작업 요구사항:
1. 같은 성격의 입력을 persona만 바꿔 비교할 것
2. 가능하면 4개 엔진 모두에서 확인할 것
3. 최소한 아래를 비교할 것
   - tone
   - output structure
   - task focus
4. 가능하면 runtime prompt/trace 기준으로 persona section 주입 여부도 확인할 것
5. 구현이 아니라 검증/평가 중심으로 진행할 것

비목표:
- 새 persona 추가
- editor 확장
- persona fragment 구조 개편
- auto skill selection

출력 형식:
### A. Opinion
### B. Validation Setup
### C. Results by Persona
### D. Engine Parity Notes
### E. Recommendation
