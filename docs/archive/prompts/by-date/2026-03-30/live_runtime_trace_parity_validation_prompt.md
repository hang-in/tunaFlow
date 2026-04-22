# Live Runtime Trace Parity Validation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/liveRuntimeTraceParityValidationPlan_2026-03-30.md`
- `docs/plans/postParityRuntimeValidationSweepPlan_2026-03-30.md`
- `docs/plans/runtimeFeatureValidationPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

작업 시작 전 짧게 의견을 말하라:
- 왜 이제는 코드 추적보다 live runtime 검증이 필요한지
- 이번 검증에서 무엇을 먼저 봐야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- `tauri dev` 기준 실제 앱 runtime/trace surface에서 4-engine ContextPack parity를 spot-check하라

우선 검증 대상:
1. trace surface parity
2. auto mode runtime behavior
3. budget override reflection
4. retrieval / compressed memory 실제 주입

권장 시나리오:
1. 같은 프로젝트/같은 대화에서 4개 엔진에 같은 입력 전송
2. 짧은 prompt와 긴 구조화 prompt를 각각 보내 auto mode 비교
3. Runtime Settings에서 mode/cap을 바꾸고 재전송
4. trace의 active/skipped/chars/top consumers 비교

비목표:
- 새 기능 구현
- threshold 재조정
- vector retrieval
- RT preset
- startup UX

검증 결과 보고 형식:
### A. Opinion
### B. Scenarios Run
### C. Findings
### D. Fixes Applied
### E. Verification
### F. Residual Gaps

검증:
- 필요 시 `npm run tauri dev`
- `tsc --noEmit`
- `vite build`
- 가능한 경우 trace surface 스크린/로그 근거 포함

중요:
- 이번 라운드는 “코드가 같나”가 아니라 “실행 surface도 같게 보이나”를 보는 라운드다
