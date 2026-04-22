# tunaFlow RT 생성 설정 → 첫 실행 연결 Phase 1

적용 스킬:
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/frontend-design`
  - 이유: RT 생성 UX에서 고른 설정이 실제 실행까지 자연스럽게 이어지는 흐름을 만들어야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/composition-patterns`
  - 이유: RT config 저장, 로드, 실행 연결을 대화 생성 UI와 분리된 계층으로 정리해야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/react-best-practices`
  - 이유: `CreateRoundtableDialog`와 `NewMessageInput` 사이 상태 전달을 임시 꼼수가 아닌 구조화된 흐름으로 정리해야 함

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\roundtableCreationConfigPlan.md`

현재 상태:
- `CreateRoundtableDialog`로 RT 제목/mode/participant/model을 고를 수 있음
- RT conversation 생성과 진입도 됨
- 하지만 생성 시 고른 설정이 실제 첫 `roundtable_run`으로 아직 연결되지 않음
- 현재는 `sessionStorage` 저장까지만 있고, `NewMessageInput`에서 읽는 로직은 미구현

이번 작업 목표는:
**RT 생성 dialog에서 고른 participant/mode/model 설정이 실제 첫 RT 실행의 기본값으로 사용되도록 연결하는 것**이다.

중요:
- 실제 코드 기준으로만 작업
- 기존 RT 실행 로직은 최대한 재사용
- 이번 단계는 Phase 1: 생성 설정 → 첫 실행 연결까지만
- 모든 응답과 보고는 한국어로만 작성하라

---

## 목표

최소한 아래를 만족하라.

1. RT 생성 시 고른 participant/mode/model이 conversation 기준으로 저장됨
2. RT conversation 진입 후 `NewMessageInput`이 그 설정을 읽음
3. 첫 `roundtable_run`이 그 설정을 기본 participant로 사용함
4. 설정이 없으면 기존 `ROUNDTABLE_PARTICIPANTS` fallback 유지

---

## 먼저 확인할 파일

- `D:\privateProject\tunaFlow\src\components\tunaflow\CreateRoundtableDialog.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\NewMessageInput.tsx`
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- `D:\privateProject\tunaFlow\src\types\index.ts`

필요 시:
- `D:\privateProject\tunaFlow\src\lib\appStore.ts`
- RT 실행 관련 backend/command 파일

---

## 구현 요구사항

### 1. RT config 타입 정의

최소한 아래를 포함하는 타입을 도입하라.

- `conversationId`
- `mode`
- `participants[]`
  - `name`
  - `engine`
  - `model`

### 2. 생성 시 저장

`CreateRoundtableDialog`에서 RT conversation 생성 직후,
해당 `conversationId` 기준으로 config를 저장하라.

1차 허용:

- `sessionStorage` 유지 가능

단, 조건:

- key를 conversation id 기준으로 명확히 분리
- 단순 임시 전역 상태가 아니라 RT별로 구분

### 3. NewMessageInput 로드

현재 conversation이 RT일 때:

- 저장된 RT config가 있으면 읽어서
  - participant 기본값
  - mode 기본값
에 반영하라

중요:

- 기존 fallback은 유지
- config가 없으면 기존 `ROUNDTABLE_PARTICIPANTS`

### 4. 첫 실행 연결

RT에서 첫 prompt를 보내면,
기본 participant 계산 대신 저장된 config participant를 우선 사용하게 하라.

중요:

- `/follow` 같은 기존 override는 계속 우선 가능
- 생성 config는 “기본값” 역할

### 5. 범위 제한

이번 단계에서는 하지 말 것:

- RT config DB schema 추가
- RT settings 편집 UI 신설
- eval/reviewer 통합
- docs 작업 같이 하기

---

## 검증

작업 후 반드시 아래를 설명하라.

1. RT config를 어디에 어떤 키로 저장했는지
2. `NewMessageInput`이 어떻게 읽는지
3. 첫 `roundtable_run`에 어떻게 반영되는지
4. 기존 fallback과 어떤 관계인지
5. 타입체크/빌드/가능한 검증 결과
6. 남은 리스크

---

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. RT Config Flow
### D. Verification
### E. Remaining Risks

바로 코드 수정까지 진행하라.

