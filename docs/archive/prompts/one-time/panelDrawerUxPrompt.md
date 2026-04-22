# tunaFlow 패널 / 드로어 UX 재설계 실행 프롬프트

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\panelDrawerUxPlan.md`

이번 작업 목표는:
현재 `tunaFlow`의 고정폭 3패널 + 중앙 branch panel 구조를,
**리사이즈 가능한 Sidebar / Workspace Panel + overlay thread/RT drawer** 구조로 재설계하는 것이다.

중요:
- 실제 코드 기준으로만 작업
- 추측 금지
- 새 페이지/라우팅 금지
- 기존 기능을 잃지 말 것
- 모든 응답과 보고는 한국어로만 작성하라

---

# 전체 범위

이번 작업은 아래 4단계를 순서대로 진행한다.

1. 패널 상태 구조 정의
2. Sidebar / Workspace Panel 리사이즈
3. BranchThreadPanel → drawer 전환
4. 우측 패널 정보 구조 1차 정리

---

# 1단계. 패널 상태 구조 정의

## 먼저 확인할 파일

- `D:\privateProject\tunaFlow\src\components\tunaflow\AppShell.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\Sidebar.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\ContextPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\BranchThreadPanel.tsx`
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- `D:\privateProject\tunaFlow\src\lib\appStore.ts`

## 목표

아래 상태를 둘 수 있는 가장 작은 구조를 정의하라.

- `sidebarWidth`
- `workspacePanelWidth`
- `threadDrawerWidth`
- 필요 시 `workspaceMode`

중요:
- width는 persistence 가능하면 저장
- 과한 전역 상태 재설계 금지

1단계가 끝나면 2단계로 진행하라.

---

# 2단계. Sidebar / Workspace Panel 리사이즈

## 목표

Sidebar와 우측 패널을 IDE처럼 제한된 범위 내에서 리사이즈 가능하게 하라.

## 요구사항

### Sidebar
- 최소: `220px`
- 기본: 현재 폭 근처
- 최대: `360px`

### Workspace Panel
- 최소: `280px`
- 기본: 현재보다 조금 넓은 범위 허용
- 최대: `520px`

## 구현 원칙

- 드래그 handle 추가
- 범위 clamp 필수
- 화면 전체를 깨지 않게 할 것

## 완료 기준

- 두 패널 모두 사용자 드래그로 폭 조절 가능
- 새로고침 후 width 복원 가능하면 포함

2단계가 끝나면 3단계로 진행하라.

---

# 3단계. BranchThreadPanel → overlay drawer 전환

## 목표

현재 `BranchThreadPanel`을 중앙 영역의 일반 flex item이 아니라,
**우측 패널 왼쪽에서 펼쳐지는 overlay drawer**로 전환하라.

## 핵심 원칙

- 메인 채팅 위의 상위 레이어 느낌
- 부모 메시지 preview 상단 고정 유지
- 닫기 / Adopt / Open Full 유지
- drawer resize 가능

## 폭 규칙

- 최소: `360px`
- 기본: `420px ~ 520px`
- 최대:
  - 중앙 채팅 가용 폭의 `90%`
  - 또는 왼쪽에 최소 시야(`120~160px`)가 남도록 clamp

중요:
- 메인 채팅을 완전히 가리면 안 된다
- Sidebar 기준 최소 10% 이상이 항상 보여야 하는 방향이면 좋다

## 구현 원칙

- 지금처럼 `AppShell`에서 일반 flex item으로 두지 말고
- 중앙 메인 영역 내부 absolute/overlay drawer 구조로 바꾸는 방향을 우선 검토하라
- 기존 branch 기능, 메시지 렌더, 입력, adopt, open full은 유지

## 완료 기준

- thread/RT panel이 한 단계 위 레이어처럼 보인다
- resize 가능하다
- 최대 폭 제한이 정상 동작한다

3단계가 끝나면 4단계로 진행하라.

---

# 4단계. 우측 패널 정보 구조 1차 정리

## 목표

우측 패널이 기능 정보를 단순 적층하지 않도록,
workspace panel 형태로 1차 정리하라.

## 요구사항

우측 패널은 다음 모드를 가질 수 있어야 한다.

- `Plan`
- `Reviews`
- `Tests`
- `Artifacts`
- `Trace`

중요:
- 중앙 상단 탭으로 올리지 말 것
- 우측 패널 내부의 mode 전환으로 다룰 것
- 한 번에 하나의 주 모드만 크게 보여줄 것

## 구현 원칙

- 현재 `ContextPanel.tsx`의 `Branches / Assets` 구조를 완전히 버릴 필요는 없음
- 그러나 harness 방향에 맞게, 작업 중심 mode 전환으로 재배치하라
- 1차에선 placeholder mode라도 괜찮지만 구조는 future-friendly해야 한다

## 완료 기준

- 우측 패널이 작업 패널처럼 보이기 시작한다
- 향후 harness 정보가 들어와도 과적재를 줄일 수 있는 구조가 된다

---

# Claude 협업용 자료 정리

이번 작업 중 UI/정보 구조 판단이 애매하면,
아래 자료를 정리해서 별도 Claude 검토에 넘겨도 된다.

1. 현재 화면 스크린샷
   - 기본 메인 채팅
   - thread 열린 상태
   - RT 상태
   - 우측 패널 현재 상태
2. 컴포넌트 구조
   - AppShell
   - Sidebar
   - ChatPanel
   - BranchThreadPanel
   - ContextPanel
3. 문제 목록
   - 안 보이는 정보
   - 경쟁하는 메타 정보
   - 현재 액션이 흐려지는 지점

중요:
- Claude에게는 "예쁘게 해줘"가 아니라
  "작업 흐름상 무엇이 모호한지"를 설명할 것
- 결과는
  - 문제 진단
  - 제안 레이아웃
  - 버릴 것
  - MVP 우선순위
  형식으로 받는 것이 좋다

---

# 테스트와 검증

작업 후 반드시 아래를 확인하라.

## A. 타입/빌드

- `tsc --noEmit`
- `vite build`
- 필요 시 `cargo check`

## B. 인터랙션

1. Sidebar resize
2. Workspace panel resize
3. thread drawer 열기/닫기
4. thread drawer resize
5. drawer 최대 폭 제한
6. RT branch도 같은 규칙으로 보이는지

## C. 회귀

아래 기능이 깨지지 않는지 확인하라.

- branch 열기
- adopt
- open full
- plan panel
- artifacts panel
- 자연어 handoff
- RT branch

---

# 출력 형식

### A. Changes Made
### B. Files Modified
### C. Layout / Drawer Flow
### D. UI Information Architecture Changes
### E. Verification
### F. Remaining Risks

바로 실제 코드 수정까지 진행하라.
