# tunaFlow 좌측 사이드바 프로젝트 트리 재설계 Phase 1

적용 스킬:
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/frontend-design`
  - 이유: 좌측 사이드바를 프로젝트 네비게이터 관점에서 더 정돈된 3섹션 구조로 재설계해야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/composition-patterns`
  - 이유: `Projects / Roundtables / Branches / Files`를 독립 섹션 컴포넌트로 정리해 역할 충돌을 줄여야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/react-best-practices`
  - 이유: 현재 프로젝트, conversation, branch, 파일 목록을 파생 상태로 다루면서 Sidebar 복잡도를 통제해야 함

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\sidebarThreeSectionPlan.md`
- `D:\privateProject\tunaFlow\docs\plans\panelDrawerUxPlan.md`
- `D:\privateProject\tunaFlow\docs\plans\harnessEngineeringAdoptionPlan.md`

현재 상태:
- 좌측 `Sidebar`는 프로젝트/대화 중심 탐색에는 충분하지만
  branch/RT와 코드베이스 탐색까지 담기엔 구조가 약함
- branch는 중앙 `BranchBar`와 drawer에서 잘 보이지만,
  좌측에서 프로젝트 단위 작업 목록처럼 스캔하기는 어렵다
- 파일 탐색 진입점이 없어 프로젝트 코드베이스와 대화 흐름을 오가기 어렵다

이번 작업 목표는:
**좌측 Sidebar를 `Projects / Roundtables / Branches / Files` 4섹션의 프로젝트 네비게이터로 재구성하는 것**이다.

중요:
- 실제 코드 기준으로만 작업
- 기존 기능은 유지
- 이번 단계는 Phase 1: 4섹션 구조와 최소 파일 탐색기까지만
- 새 페이지/라우팅 추가 금지
- 모든 응답과 보고는 한국어로만 작성하라

---

## 목표

최소한 아래를 만족하라.

1. 좌측 사이드바에 `Projects` 섹션이 명확히 보임
2. 같은 프로젝트의 `Roundtables`와 `Branches` 섹션이 분리되어 보임
3. `Files` 섹션에서 현재 프로젝트 루트의 얕은 파일/폴더 목록을 볼 수 있음
4. 깊은 단일 트리가 아니라 4개의 섹션으로 읽힘
5. 기존 conversation 전환과 branch 열기 흐름은 유지됨

---

## 먼저 확인할 파일

- `D:\privateProject\tunaFlow\src\components\tunaflow\Sidebar.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\BranchBar.tsx`
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- `D:\privateProject\tunaFlow\src\types\index.ts`
- 필요 시:
  - 프로젝트/branch/conversation 관련 Tauri command 파일
  - 파일 시스템 접근에 이미 쓰는 command/util

---

## 구현 요구사항

### 1. Sidebar를 4섹션 구조로 재정렬

아래 순서로 보이게 하라.

1. `Projects`
2. `Roundtables`
3. `Branches`
4. `Files`

중요:
- 하나의 깊은 트리로 만들지 말 것
- 섹션 헤더와 섹션 본문이 분명해야 함

### 2. Projects 섹션

현재 Sidebar의 프로젝트 목록과 main conversation 목록을 이 섹션에 정리하라.

유지되어야 할 것:
- 현재 프로젝트 표시/전환
- 현재 conversation 선택
- custom label 표시
- rename UI
- 기존 main conversation 접근성

권장:
- 프로젝트 노드 아래에 현재 선택 프로젝트의 conversations를 즉시 표시
- 다른 프로젝트 클릭 시 즉시 전환 + expand
- 프로젝트 추가 액션도 이 섹션 안으로 넣을 수 있으면 좋다

### 3. Roundtables 섹션

현재 프로젝트에 속한 RT branch만 별도 섹션으로 보여라.

권장:
- `mode === "roundtable"` 기준으로 필터
- `parentBranchId` 기반 하위 RT 계층 표시
- 현재 active/open RT 강조
- 클릭 시 기존 흐름에 맞게 branch 열기

### 4. Branches 섹션

현재 프로젝트에 속한 일반 branch만 별도 섹션으로 보여라.

권장:
- RT를 제외한 branch만 표시
- `parentBranchId` 기반 하위 branch 계층 표시
- 현재 active/open branch 강조
- 클릭 시 기존 흐름에 맞게 branch 열기

중요:
- 좌측은 이동/열기 중심
- adopt/delete 같은 무거운 액션은 여기서 과하게 늘리지 말 것

### 5. Files 섹션

현재 프로젝트 루트 기준의 얕은 파일/폴더 목록을 보여라.

1차 권장 범위:
- 루트 폴더명
- 1~2단 정도의 얕은 트리 또는 루트+직계 항목
- 폴더/파일 아이콘 정도의 최소 구분

중요:
- 전체 IDE 수준 파일 탐색기까지 가지 말 것
- 1차는 "프로젝트 코드베이스 진입점"만 확보하면 충분
- 프로젝트가 선택되지 않았거나 경로가 없으면 빈 상태를 명확히 보여라

### 6. 접이식 섹션

`Roundtables`, `Branches`, `Files`는 접기/펼치기가 가능하면 좋다.
이번 단계에서 구현 비용이 크지 않으면 `Projects`도 접이식으로 맞춰라.

### 7. 프로젝트 중심 데이터 로드 원칙

store는 현재 선택된 프로젝트의 conversations/branches/artifacts만 유지한다.
이것은 제한사항이 아니라 **의도된 프로젝트 단위 설계**다.

원칙:
- 프로젝트 전환 = 컨텍스트 스위치. 이전 프로젝트 데이터는 해제되어도 된다
- 검색은 현재 프로젝트 내부만 대상이다
- 다중 프로젝트 동시 가시성은 제품 목표가 아니다
- `conversationsByProject` 같은 전역 캐시는 현재 원칙상 불필요하다

### 8. 기존 흐름 유지

아래는 유지되어야 한다.
- 프로젝트 전환
- conversation 전환
- conversation rename
- branch 열기
- sidebar resize

### 9. 범위 제한

이번 단계에서는 하지 말 것:
- 깊은 파일 검색기 구현
- 파일 편집기/미리보기 추가
- branch 전체 액션 패널화
- docs 작업 같이 하기

---

## 구현 우선순위

권장:
1. 섹션 구조 뼈대
2. Projects 정리
3. Roundtables / Branches 분리
4. Files 최소 탐색기 추가
5. 접이식 섹션 polish

---

## 검증

작업 후 반드시 아래를 설명하라.

1. Sidebar를 어떻게 4섹션으로 재구성했는지
2. `Projects / Roundtables / Branches`를 어떻게 나눴는지
3. `Files` 섹션을 어디까지 구현했는지
4. 프로젝트별 conversation 로드 정책을 어떻게 판단했는지
5. 왜 깊은 단일 트리가 아니라 섹션형 구조로 갔는지
6. 기존 기능을 어떻게 유지했는지
7. 타입체크/빌드/가능한 검증 결과
8. 남은 리스크

---

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. Sidebar Project Tree Flow
### D. Verification
### E. Remaining Risks

바로 코드 수정까지 진행하라.
