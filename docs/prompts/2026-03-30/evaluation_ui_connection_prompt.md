# Evaluation UI Connection

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/evaluationUiConnectionPlan_2026-03-30.md`
- `docs/reference/implementationStatus.md`

먼저 확인할 파일:
- `src-tauri/src/commands/evaluation.rs`
- `src-tauri/src/lib.rs`
- 현재 `CenterPanel` 및 관련 메인 탭 컴포넌트

작업 시작 전 짧게 의견을 말하라:
- evaluation을 어디에 붙이는 것이 현재 IA에 가장 자연스러운지
- 왜 지금 새 평가 프레임워크보다 기존 backend 연결이 먼저인지

이번 작업 목표:
- 기존 evaluation backend를 frontend UI에 연결해, conversation 기준 eval run 목록과 결과 비교 화면을 제공하라.

구현 범위:
1. eval run 목록 조회 UI
2. eval result 상세 보기 UI
3. run status 표시
4. 가능하면 최소 run 생성 UI

권장 방향:
- 메인 탭 `Evaluation` 추가를 우선 검토
- round/agent별 결과를 읽기 쉽게 정리

비목표:
- auto judge
- scoring/rubric
- benchmark dashboard
- 대규모 harness 재설계

검증:
- tsc --noEmit
- 필요한 경우 cargo check
- eval run 생성/조회/상세 표시 흐름 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. UI Flow
### D. Verification
### E. Residual Gaps
