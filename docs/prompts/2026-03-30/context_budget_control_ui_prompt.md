# Context Budget Control UI

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/contextBudgetScalingPlan.md`
- `docs/plans/contextPackP0Phase1Plan_2026-03-30.md`
- `docs/plans/contextBudgetControlUiPlan_2026-03-30.md`

먼저 확인할 파일:
- `Settings > Runtime` 관련 UI 파일
- context budget/guardrail 관련 backend 파일
- trace/runtime visibility 관련 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 budget control UI를 열 수 있게 되었는지
- 왜 숫자 슬라이더 하나보다 mode + policy가 함께 보여야 하는지

이번 작업 목표:
- `Settings > Runtime`에 Context Budget 조정 UI를 추가하라.

구현 범위:
1. `Lite / Standard / Full` mode control
2. total budget cap 표시 및 안전 범위 내 조정
3. 각 모드의 section policy 설명
4. 가능하면 현재 trace/runtime과 연결된 확인 지점 제공

비목표:
- per-section 자유 편집
- 엔진별 고급 budget 튜닝
- retrieval policy 편집

검증:
- `tsc --noEmit`
- 필요 시 `cargo check`
- 모드/한도 변경 후 Runtime/Trace에서 변화 확인 가능 여부

출력 형식:
### A. Opinion
### B. Files Changed
### C. UI Flow
### D. Verification
### E. Residual Gaps
