# Roundtable Role Terminology Separation

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/roundtableParticipantRoleBlindUiPlan_2026-03-30.md`
- `docs/plans/roundtableParticipantSurfaceVisibilityPlan_2026-03-30.md`
- `docs/plans/roundtableRoleTerminologySeparationPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src/types/index.ts`
- `src/components/tunaflow/CreateRoundtableDialog.tsx`
- `src/components/tunaflow/RoundtableView.tsx`
- RT config 저장/로드 관련 파일
- RT executor 쪽 주석/타입 참조 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 현재 `role`이 프로젝트 워크플로우 역할과 혼동되는지
- 왜 profile과 RT role을 분리해야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- RT 전용 `role`을 `rtRole` 또는 동등한 명시적 용어로 분리하고, UI에서도 `토론 역할`로 읽히게 하라

구현 범위:
1. 가능하면 타입/필드를 `role`에서 `rtRole`로 명확화
2. 최소한 UI 라벨은 `role`이 아니라 `토론 역할` 또는 `RT Role`로 변경
3. `RoundtableView`와 생성 다이얼로그에서 profile과 RT role이 다른 층으로 읽히게 하라
4. 관련 주석/설명도 RT role 의미에 맞게 정리하라

비목표:
- RT orchestration 재설계
- blind verifier 규칙 변경
- role 종류 확장
- lead decomposition
- max token override 기능 확장

구현 원칙:
- 사용자-facing 1급 정체성은 profile
- RT role은 보조 실행 속성
- compatibility를 불필요하게 깨지 말 것
- 필드명/라벨/설명이 같은 의미를 가리키게 할 것

검증:
- `tsc --noEmit`
- `vite build`
- 기존 RT config 로드가 안 깨지는지 설명하라
- 생성 UI와 RT 표면에서 `토론 역할` 의미가 더 명확해졌는지 설명하라

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Terminology Mapping
### E. Verification
### F. Residual Gaps
