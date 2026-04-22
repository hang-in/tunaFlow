# tunaFlow Workspace Panel 재설계 Phase 1 실행 프롬프트

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\workspacePanelRedesignPlan.md`
- `D:\privateProject\tunaFlow\docs\plans\harnessEngineeringAdoptionPlan.md`

이번 작업 목표는:
현재 `ContextPanel`의 분류형 구조를,
**3모드 우선의 workflow형 workspace panel**로 재정리하는 것이다.

중요:
- 실제 코드 기준으로만 작업
- 기존 기능을 잃지 말 것
- 새 페이지/라우팅 금지
- 이번 단계는 Phase 1까지만
- 모든 응답과 보고는 한국어로만 작성하라

---

# 목표

1차 MVP에서는 아래 3개만 실제 mode로 만든다.

- `Plan`
- `Artifacts`
- `Trace`

중요:
- `Review`, `Test`는 아직 독립 mode로 만들지 말 것
- 빈 모드/빈 탭 UX를 만들지 말 것

---

# 먼저 확인할 파일

- `D:\privateProject\tunaFlow\src\components\tunaflow\ContextPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\ChatPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\context-panel\PlansPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\context-panel\ArtifactsPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\context-panel\MemosPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\context-panel\SkillsPanel.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\context-panel\BranchesPanel.tsx`
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`

---

# 구현 요구사항

## 1. workspace mode state 도입

현재 `primaryTab / assetSegment` 구조를 재구성해
아래 mode를 표현하라.

- `plan`
- `artifacts`
- `trace`

중요:
- 1차에서는 이 3개만 실제 mode

## 2. 각 mode 매핑

### `Plan`
- 기존 `PlansPanel` 재사용

### `Artifacts`
- 기존 `ArtifactsPanel` 재사용
- `Memos`는 접이식 보조 섹션으로 포함 검토

중요:
- memo는 artifact와 스코프가 다를 수 있으므로
  conversation / project 성격 차이를 해치지 않는 최소 방식으로 넣을 것

### `Trace`
- 실제 코드에 있는 trace/run 상태/log를 최대한 활용
- 충분한 데이터가 없으면 최소 placeholder가 아니라
  현재 running/queue/rawq/engine 상태 요약이라도 보여줄 것

## 3. BranchesPanel 처리

`BranchesPanel`은 우측 패널의 대표 탭에서 내려라.

1차 권장:
- 우측 패널 루트에서는 제거/축소
- 중앙 conversation 헤더 아래의 `branch bar`로 옮길 준비를 하거나,
  최소한 우측 패널에서 비중을 크게 줄일 것

중요:
- branch는 대화 객체이지 workspace panel의 주 mode가 아니다

## 4. SkillsPanel 처리

`SkillsPanel`은 Artifacts 하위에 넣지 말 것.

권장:
- 이번 단계에서는 우측 패널에서 비중 축소
- 후속으로 입력 영역 근처나 settings popover 이동 예정임을 고려한 구조 유지

## 5. 정보 밀도 조절

우측 패널은 "다 보여주기"보다 "지금 필요한 것만 크게"가 목표다.

- 현재 활성 mode만 크게
- 나머지는 작은 버튼/배지
- 긴 적층 스크롤 금지

---

# 하지 말 것

- Review mode 독립 구현
- Test mode 독립 구현
- thread/drawer UX 같이 수정
- 좌우 패널 리사이즈 같이 수정
- docs 작업 같이 하기

---

# 검증

작업 후 반드시 아래를 설명하라.

1. ContextPanel 구조를 어떻게 바꿨는지
2. 왜 3모드만 실제 mode로 선택했는지
3. `Artifacts` 안의 `Memos`를 어떻게 처리했는지
4. `BranchesPanel`을 어떻게 축소/재배치했는지
5. `SkillsPanel`은 어떻게 다뤘는지
6. 타입체크/빌드 결과
7. 남은 리스크

---

# 출력 형식

### A. Changes Made
### B. Files Modified
### C. Workspace Panel Flow
### D. Verification
### E. Remaining Risks

바로 코드 수정까지 진행하라.
