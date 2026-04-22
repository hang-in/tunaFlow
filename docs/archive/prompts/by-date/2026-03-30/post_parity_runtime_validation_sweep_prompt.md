# Post-Parity Runtime Validation Sweep

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/postParityRuntimeValidationSweepPlan_2026-03-30.md`
- `docs/plans/runtimeFeatureValidationPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 이번 수정이 단순 버그픽스가 아니라 검증 기준 자체를 바꾸는지
- 왜 Claude를 먼저 재검증해야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- Claude가 통합 ContextPack 파이프라인을 타게 된 이후, parity fix가 실제로 효과를 냈는지 재검증하라

우선 검증 대상:
1. Claude send / stream parity
2. compressed memory
3. retrieval
4. auto mode / budget override
5. cross-engine spot check

검증 원칙:
- 코드 읽기만으로 끝내지 말고 실제 실행/trace를 보라
- Claude 우선
- 결과는 통과/실패/불명확으로 구분하라
- regressions가 보이면 가능한 범위에서 바로 수정하라

비목표:
- 새 기능 추가
- threshold 재조정
- startup UX
- RT preset
- context-hub 확장

검증 결과 보고 형식:
### A. Opinion
### B. Scenarios Run
### C. Findings
### D. Fixes Applied
### E. Verification
### F. Residual Gaps

검증:
- 필요한 Rust 테스트
- `tsc --noEmit`
- `vite build`
- 실제 Trace/Runtime 표면 확인

중요:
- 이번 라운드는 “기능이 있나?”보다 “Claude parity fix 이후 실제로 같은 파이프라인이 동작하나?”를 보는 라운드다
