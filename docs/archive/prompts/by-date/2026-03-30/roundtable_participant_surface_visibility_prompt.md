# Roundtable Participant Surface Visibility

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/roundtableParticipantRoleBlindUiPlan_2026-03-30.md`
- `docs/plans/roundtableParticipantSurfaceVisibilityPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src/components/tunaflow/RoundtableView.tsx`
- `src/components/tunaflow/CreateRoundtableDialog.tsx`
- RT participant status/event를 받는 프론트 파일
- `src/types/index.ts`

작업 시작 전 짧게 의견을 말하라:
- role/blind가 생성 UI에만 있고 실행 표면에 없으면 왜 반쪽 기능인지
- blind verifier가 실행 중에도 보여야 하는 이유
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- RT 실행/조회 표면에서 participant별 role / blind를 다시 확인 가능하게 하라

구현 범위:
1. `RoundtableView` 또는 인접한 RT 표면에서 participant role 표시 추가
2. blind participant는 shield/icon/badge 등으로 명확히 구분
3. 진행 상태 또는 결과 표면에서 blind verifier 여부를 최소 수준으로 반영
4. 기존 RT 레이아웃은 크게 흔들지 말 것

비목표:
- RT 생성 다이얼로그 재설계
- hard max token enforcement
- verifier scoring
- lead decomposition
- 대규모 RT UX 개편

구현 원칙:
- 스캔 가능한 짧은 badge/pill 중심
- role은 짧은 label로
- blind는 시각적으로 바로 구분 가능하게
- “설정 화면”이 아니라 “실행 표면 가시화”에 집중

검증:
- `tsc --noEmit`
- `vite build`
- RT 실행/재조회 시 role/blind를 다시 확인할 수 있는지 설명하라
- blind participant status/result가 어디에서 어떻게 보이는지 설명하라

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. UI Flow
### E. Verification
### F. Residual Gaps
