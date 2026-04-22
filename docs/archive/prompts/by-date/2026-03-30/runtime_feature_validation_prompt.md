# Runtime Feature Validation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/runtimeFeatureValidationPlan_2026-03-30.md`
- `docs/reference/codexProjectReference_2026-03-29.md`
- `docs/reference/projectFirstEntryPolicy_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 지금은 새 기능보다 검증 라운드가 맞는지
- 이번 검증에서 무엇을 가장 먼저 봐야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- 이번 세션에서 추가된 핵심 runtime 기능들을 실제 사용 흐름 기준으로 검증하고, 발견된 버그를 가능한 범위에서 즉시 수정하라

우선 검증 대상:
1. compressed memory
2. conversation retrieval
3. auto mode heuristic
4. budget control UI reflection
5. roundtable core regression

검증 원칙:
- 추측보다 실제 실행/trace/log를 우선하라
- 사용 시나리오를 짧게라도 직접 재현하라
- 버그가 보이면 문서화만 하지 말고 가능한 범위에서 수정까지 하라
- 결과는 “통과/실패/불명확”으로 구분하라

권장 검증 순서:
1. 긴 대화/여러 대화가 있는 테스트 프로젝트 또는 가장 가까운 검증 환경 준비
2. compressed memory 생성 여부 확인
3. retrieval이 다른 대화 chunk를 가져오는지 확인
4. Auto가 Lite/Standard/Full을 적절히 바꾸는지 확인
5. Runtime Settings의 budget override가 실제 trace/meta에 반영되는지 확인
6. RT completion-order / blind verifier / role-blind visibility 회귀 확인

비목표:
- startup UX 구현
- vector retrieval/embedding 도입
- RT preset 설계
- context-hub 자동 주입 확대
- 새 대형 기능 추가

검증 결과 보고 형식:
### A. Opinion
### B. Scenarios Run
### C. Findings
- 통과/실패/불명확을 구분
### D. Fixes Applied
### E. Verification
### F. Residual Gaps

검증:
- 필요한 타입스크립트/러스트 테스트
- `tsc --noEmit`
- `vite build`
- 필요한 경우 관련 Rust 테스트

중요:
- 이 라운드는 새 기능 추가보다 품질 검증이 목적이다
- 결과가 불명확하면 불명확하다고 말하고, 무엇이 부족한지 구체적으로 적어라
