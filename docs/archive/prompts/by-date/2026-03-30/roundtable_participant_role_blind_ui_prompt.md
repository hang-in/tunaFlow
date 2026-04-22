# Roundtable Participant Role / Blind UI

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/roundtableBlindVerifierPhasePlan_2026-03-30.md`
- `docs/plans/roundtableParticipantRoleBlindUiPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src/components/tunaflow/CreateRoundtableDialog.tsx`
- `src/components/tunaflow/RoundtableView.tsx`
- `src/components/tunaflow/input/RoundtableControls.tsx`
- `src/types/index.ts`
- RT config 저장/로드 관련 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 backend capability만 있고 UI가 없으면 RT 기능이 반쪽인지
- blind verifier와 role이 왜 participant 설정 표면에 보여야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- RT participant 설정 UI에서 role / blind / 필요 시 max token override를 다룰 수 있게 하라

구현 범위:
1. participant role 선택 UI 추가
2. participant blind 토글 추가
3. 필요하면 max token override는 최소 advanced 입력으로 추가
4. 저장된 RT config에 이 값들이 유지되게 하라
5. RT 뷰나 participant 표면에서 role/blind를 최소 수준으로 확인 가능하게 하라

비목표:
- lead decomposition
- verifier scoring
- hard max token enforcement
- 대규모 RT UX 재설계

구현 원칙:
- 설정 UI는 가볍고 명확하게
- 기본은 role-based default cap
- manual override는 보조 기능
- blind verifier는 쉽게 켜고 확인할 수 있어야 한다

검증:
- `tsc --noEmit`
- `vite build`
- RT 생성 후 저장/재진입 시 role/blind가 유지되는지 확인
- 실제 실행 payload에 값이 반영되는지 설명하라

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. UI Flow
### E. Verification
### F. Residual Gaps
