# Project-First Startup UX

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/reference/projectFirstEntryPolicy_2026-03-30.md`
- `docs/plans/projectFirstStartupUxPlan_2026-03-30.md`
- `docs/plans/sidebarWorkspaceHierarchyPlan_2026-03-29.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src/App.tsx`
- `src/components/tunaflow/AppShell.tsx`
- project selector / sidebar 관련 컴포넌트
- project store / selection 관련 slice

작업 시작 전 짧게 의견을 말하라:
- 왜 project-first entry가 지금 필요한지
- 왜 projectless chat을 정상 제품 경로로 두면 안 되는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- 프로젝트가 선택되지 않은 상태에서는 project selector/onboarding이 먼저 보이게 하고, 일반 chat workflow는 시작되지 않게 하라

구현 범위:
1. startup state에서 project selected / not selected를 명확히 구분하라
2. projectless state에서는 selector/onboarding surface를 먼저 보여라
3. 프로젝트 선택 후 기존 메인 workflow로 자연스럽게 진입하게 하라
4. 기존 Sidebar selector와 충돌하지 않게 역할을 정리하라

비목표:
- dogfood project 운영 시작
- import wizard 확장
- onboarding 카피 대량 작성
- workspace 전체 재설계

구현 원칙:
- 시작 UX는 단순해야 한다
- project를 고른 뒤에만 agent workflow가 열린다
- projectless state를 정상 모드처럼 보이게 하지 말라

검증:
- `tsc --noEmit`
- `vite build`
- 첫 실행/미선택 상태에서 selector가 먼저 뜨는지 확인
- 프로젝트 선택 후 기존 workflow로 정상 진입하는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Startup Flow
### E. Verification
### F. Residual Gaps
