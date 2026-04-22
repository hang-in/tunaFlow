# Real Workflow Memory Quality Validation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/realWorkflowMemoryQualityValidationPlan_2026-03-30.md`
- `docs/plans/runtimeFeatureValidationPlan_2026-03-30.md`
- `docs/plans/liveRuntimeTraceParityValidationPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 이제는 trace correctness보다 실제 응답 품질 검증이 중요한지
- 이번 검증에서 무엇을 먼저 봐야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- 실제 프로젝트 기반 시나리오에서 compressed memory / retrieval / auto mode / budget override의 품질 효과를 검증하라

우선 검증 대상:
1. long chat continuity
2. cross-conversation recall
3. auto mode contrast
4. budget contrast

검증 원칙:
- trace만 보지 말고 실제 응답 품질을 함께 보라
- 도움이 되는 사례와 noise 사례를 둘 다 기록하라
- 결과는 통과/실패/혼합으로 구분하라
- 명확한 품질 문제는 가능한 범위에서 바로 수정하라

비목표:
- 새 memory layer 추가
- vector retrieval 도입
- startup UX
- RT preset

출력 형식:
### A. Opinion
### B. Scenarios Run
### C. Findings
### D. Fixes Applied
### E. Verification
### F. Residual Gaps

검증:
- 필요한 경우 `npm run tauri dev`
- `tsc --noEmit`
- `vite build`
- 필요한 경우 관련 테스트

중요:
- 이번 라운드는 “잘 표시되는가”가 아니라 “실제 답변이 더 좋아졌는가”를 보는 라운드다
