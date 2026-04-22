# Token Cost DB Parity

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/tokenCostTrackingEngineParityPlan.md`
- `docs/plans/tokenCostDbParityPlan_2026-03-30.md`

먼저 확인할 파일:
- usage/cost 저장 schema/migration 파일
- trace/runtime 관련 Rust 모델
- `TracePanel.tsx` 및 관련 formatter

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 UI `N/A`만으로는 부족하고 DB 레벨 상태 구분이 필요한지
- nullable 방식과 explicit status 방식 중 어느 쪽이 더 맞는지

이번 작업 목표:
- token/cost를 DB 레벨에서 `0 / unavailable / unknown`이 구분되도록 보강하라.

구현 범위:
1. schema/migration 보강
2. Rust 모델/직렬화 타입 정리
3. frontend trace/runtime 표시와 정합성 맞추기
4. 기존 데이터에 대한 안전한 기본값 처리

비목표:
- cost 추정
- 분석 대시보드
- provider billing 정밀화

검증:
- cargo check
- tsc --noEmit
- usage 미지원 엔진이 더 이상 `0`처럼 보이지 않는지 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Data Model Decision
### D. Verification
### E. Residual Gaps
