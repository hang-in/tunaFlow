# tunaFlow 메시지 쌍 삭제 구현 Phase 1

적용 스킬:
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/react-best-practices`
  - 이유: 메시지 삭제가 store, UI, backend command를 함께 건드리므로 상태 일관성을 먼저 지켜야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/composition-patterns`
  - 이유: MessageItem 액션과 삭제 확인 흐름을 공통 패턴으로 넣어야 함
- `D:\privateProject\tunaFlow\agents\_skills\_stage1_tunaflow\ui-build/frontend-design`
  - 이유: 삭제 액션이 강한 destructive 동작이므로 UI 표현과 경고 톤이 명확해야 함

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\messagePairDeletionPlan.md`

현재 상태:
- 메시지 목록 조회/생성/상태 업데이트 command는 있음
- 하지만 특정 질문/응답 쌍을 삭제하는 기능은 없음
- 일반 채팅에서는 질문만 남거나 답변만 남는 것보다 user+assistant 한 쌍을 함께 지우는 UX가 더 자연스럽다

이번 작업 목표는:
**일반 chat conversation에서 user+assistant 인접 한 쌍을 삭제하는 기능을 추가하고, 삭제 후 해당 메시지가 이후 컨텍스트팩에서도 빠지게 만드는 것**이다.

중요:
- 실제 코드 기준으로만 작업
- 이번 단계는 Phase 1: 일반 chat conversation의 인접 쌍 삭제만
- RT/branch 특수 케이스는 다루지 말 것
- memo/artifact cascade delete도 하지 말 것
- 모든 응답과 보고는 한국어로만 작성하라

---

## 목표

최소한 아래를 만족하라.

1. user 메시지 기준으로 직후 assistant 응답까지 함께 삭제 가능
2. assistant 메시지에서 삭제해도 대응 user와 함께 삭제 가능
3. 삭제는 DB row를 실제 삭제
4. 이후 컨텍스트팩에는 그 메시지들이 포함되지 않음
5. UI에서 안전하게 확인 후 삭제 가능

---

## 먼저 확인할 파일

### backend
- `D:\privateProject\tunaFlow\src-tauri\src\commands\messages.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\lib.rs`
- 필요 시:
  - `D:\privateProject\tunaFlow\src-tauri\src\commands\context_queries.rs`
  - `D:\privateProject\tunaFlow\src-tauri\src\commands\agents_helpers\context_pack.rs`

### frontend
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- `D:\privateProject\tunaFlow\src\components\tunaflow\MessageItem.tsx`

---

## 구현 요구사항

### 1. backend command 추가

새 command를 추가하라.

권장 이름:
- `delete_message_pair`

입력:
- `messageId`

동작:

1. 기준 메시지 조회
2. 같은 conversation 안에서 짝 메시지 찾기
3. 일반 chat 대화에서 인접 user+assistant 한 쌍 삭제

권장 규칙:

- 기준이 `user`이면:
  - 바로 뒤의 첫 `assistant` 메시지까지 삭제
- 기준이 `assistant`이면:
  - 바로 앞의 `user` 메시지와 함께 삭제

중요:
- transaction 사용
- 짝을 못 찾으면 에러 또는 명확한 no-op 처리
- 1차에서는 `role in ('user','assistant')` 완료 메시지만 대상으로 단순화 가능

### 2. frontend store 액션 추가

`chatStore`에 메시지 쌍 삭제 액션을 추가하라.

권장:
- backend command 호출
- 성공 시 현재 conversation 메시지 reload

### 3. MessageItem 삭제 액션

`MessageItem` 액션에 삭제 항목을 추가하라.

권장:
- assistant/user 모두에서 보이게 가능
- 라벨은 `Delete pair` 또는 `질문/응답 삭제`
- destructive style
- `window.confirm` 수준의 1차 확인 허용

### 4. 범위 제한

이번 단계에서는 하지 말 것:

- RT conversation pair delete
- branch shadow conversation 특수 규칙
- memo/artifact/checkpoint cascade
- soft delete/undo
- docs 작업 같이 하기

---

## 검증

작업 후 반드시 아래를 설명하라.

1. 어떤 기준으로 짝 메시지를 찾는지
2. 왜 일반 chat conversation 1차 범위만 택했는지
3. 삭제 후 컨텍스트팩에서 왜 빠지게 되는지
4. UI에서 어떻게 노출했는지
5. 타입체크/빌드/가능한 검증 결과
6. 남은 리스크

---

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. Message Pair Deletion Flow
### D. Verification
### E. Remaining Risks

바로 코드 수정까지 진행하라.

