# tunaFlow 확장 대비 리팩토링 Phase 1

적용 스킬:
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/react-best-practices`
  - 이유: store와 입력/상태 흐름을 분해하면서 기존 기능 회귀 없이 구조를 정리해야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/composition-patterns`
  - 이유: Sidebar, input, message, orchestrator를 공용 섹션/조합형 패턴으로 재구성해야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/frontend-design`
  - 이유: 구조 분해 후에도 현재 Linear 톤과 프로젝트 중심 UX를 유지해야 함

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\scalabilityRefactorPlan.md`

이번 작업 목표는:
**기능 추가 여지가 많이 남아 있는 현재 코드베이스를 기준으로, 앞으로 기능이 계속 붙어도 파일이 폭발하지 않도록 책임 경계를 먼저 만드는 리팩토링을 단계적으로 진행하는 것**이다.

중요:
- 실제 코드 기준으로만 작업
- "작게 예쁘게"보다 "앞으로 확장 가능하게"가 우선
- 기존 기능 유지가 최우선
- 이번 단계는 전체를 한 번에 끝내려 하지 말고, Phase 1부터 순차 진행
- 모든 응답과 보고는 한국어로만 작성하라

---

## 이번 세션 기본 원칙

1. mega file를 직접 줄이는 것보다 책임 분리를 우선
2. 프로젝트 중심 설계는 유지
3. UI/상태/백엔드 허브 파일부터 분해
4. 분해 후 테스트를 붙일 seam을 만드는 것을 목표로 함

---

## 리팩토링 대상 우선순위

1. `src/stores/chatStore.ts`
2. `src/components/tunaflow/Sidebar.tsx`
3. `src/components/tunaflow/NewMessageInput.tsx`
4. `src-tauri/src/commands/agents.rs`

중요:
- 이 순서를 기본으로 하되, 한 세션에 하나의 Phase만 크게 끝내는 쪽이 낫다

---

## 이번 Phase 1 목표

우선 **`chatStore.ts` 분해**부터 진행하라.

최소한 아래를 달성하라.

1. 단일 store 파일에 섞인 책임을 slice/section 수준으로 분리
2. 기존 API(surface)는 최대한 유지
3. 프로젝트 선택 / conversation / branch / runtime queue / artifacts / engine models 경계가 더 분명해짐
4. 이후 Sidebar / Input / Harness 기능 추가가 쉬워짐

---

## 먼저 확인할 파일

- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- 필요 시:
  - `D:\privateProject\tunaFlow\src\types\index.ts`
  - `D:\privateProject\tunaFlow\src\components\tunaflow\Sidebar.tsx`
  - `D:\privateProject\tunaFlow\src\components\tunaflow\NewMessageInput.tsx`

---

## 구현 요구사항

### 1. chatStore 책임 분리

권장 분해 단위:

- `project`
- `conversation`
- `branch/thread`
- `runtime`
- `artifact/memo/skills`
- `engine models`

중요:
- Zustand store를 완전히 다른 패턴으로 바꾸지 말 것
- 기존 컴포넌트 사용처를 최소 수정으로 유지하는 방향이 좋다

### 2. API surface 최대한 유지

예:
- `useChatStore()`에서 쓰는 주요 액션 이름
- 주요 state key

는 가능한 한 유지하라.

즉 내부 구조는 정리하되, 외부 사용처 대규모 수정은 피하라.

### 3. helper/module 분리 허용

필요하면 아래를 새 파일로 분리하라.

- `store/chatStore.types.ts`
- `store/chatStore.helpers.ts`
- `store/chatStore.project.ts`
- `store/chatStore.runtime.ts`
- `store/chatStore.branch.ts`

파일명은 실제 코드 스타일에 맞춰 조정 가능하다.

### 4. 프로젝트 중심 설계 유지

중요:
- 프로젝트 전환 시 현재 프로젝트 데이터만 유지하는 원칙은 바꾸지 말 것
- 다중 프로젝트 캐시 확대 금지

### 5. 테스트 seam 고려

이번 단계에서 테스트를 꼭 추가하지 않아도 된다.
다만 다음이 쉬워지게 하라.

- project selection state test
- branch/thread queue test
- engine model load/refresh test

---

## 하지 말 것

- Sidebar 구조 재설계까지 한 번에 같이 하기
- NewMessageInput까지 한 번에 대규모 수정
- agents.rs 리팩토링까지 한 세션에 묶기
- 프로젝트 중심 원칙 변경
- docs 작업 같이 하기

---

## 검증

작업 후 반드시 아래를 설명하라.

1. `chatStore.ts`를 어떤 책임 기준으로 분리했는지
2. 외부 API(surface)를 얼마나 유지했는지
3. 프로젝트 중심 설계를 어떻게 보존했는지
4. 이후 어떤 Phase가 쉬워졌는지
5. 타입체크/빌드/가능한 검증 결과
6. 남은 리스크

---

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. Refactor Structure
### D. Verification
### E. Remaining Risks

바로 코드 수정까지 진행하라.

