# tunaFlow Message Search 도입 프롬프트

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\messageSearchAdoptionPlan.md`

참고 구현:
- `D:\privateProject\tunaDish\client\src\lib\db.ts`
- `D:\privateProject\tunaDish\client\src\components\layout\TopNav.tsx`
- `D:\privateProject\tunaDish\client\src\components\layout\MessageSearchResults.tsx`

이번 작업 목표는:
`tunaDish`의 채팅 검색 UX를 참고하되,
`tunaFlow`의 기존 Rust `rusqlite` 구조를 유지하면서 **메시지 전문 검색(FTS5)** 를 도입하는 것이다.

중요:
- 실제 코드 기준으로만 작업
- `tunaFlow`에 `plugin-sql`을 새로 넣지 말 것
- 기존 Rust DB 레이어를 재사용할 것
- 새 페이지/라우팅 추가 금지
- 모든 응답과 보고는 한국어로만 작성하라

---

## 목표

최소한 아래를 만족하라.

1. `messages` 내용이 FTS로 검색 가능
2. 상단 헤더 또는 현재 구조상 가장 자연스러운 위치에 검색 입력창 제공
3. 입력 2글자 이상 + debounce 후 결과 dropdown 표시
4. 결과 클릭 시 해당 conversation 선택
5. 1차는 특정 message jump 없이 conversation 이동까지만

---

## 먼저 확인할 파일

### Backend
- `D:\privateProject\tunaFlow\src-tauri\src\db\schema.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\db\migrations.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\db\models.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\commands\messages.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\lib.rs`

### Frontend
- `D:\privateProject\tunaFlow\src\components\tunaflow\AppShell.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\Sidebar.tsx`
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- 필요 시 상단 헤더 관련 컴포넌트

---

## 구현 요구사항

### 1. Backend FTS

아래를 구현하라.

1. `messages_fts` virtual table
2. insert/update/delete trigger
3. `search_messages(query, limit, project_key?)` command

권장:
- 1차는 current project 기준 필터를 넣는 것이 더 좋다
- 어렵다면 전체 검색으로 시작하고 project filter는 후속으로 남겨도 된다

### 2. 검색 반환 형태

최소 반환 필드:
- `id`
- `conversation_id`
- `content`
- `timestamp`
- `rank`

가능하면:
- `role`
- `engine`
도 포함 가능

### 3. Frontend UI

검색 UI는 `tunaDish`처럼 상단 헤더 기반이 가장 자연스럽다.

필수:
- 입력창
- 2글자 이상에서만 검색
- 250~300ms debounce
- 결과 dropdown
- 빈 결과/로딩 상태

### 4. 결과 클릭 동작

1차는:
- 해당 conversation 선택
- 검색창 닫기

까지면 충분하다.

특정 message scroll/jump는 이번 단계에서 하지 말 것.

### 5. Snippet / Highlight

1차에서도 snippet과 highlight는 넣는 것이 좋다.

권장:
- markdown/plain text 섞여 있어도 대략적인 텍스트 snippet
- query 하이라이트

### 6. 범위 제한

이번 단계에서는 하지 말 것:
- `plugin-sql` 도입
- 별도 검색 페이지
- 특정 message로 자동 스크롤
- recent search history
- command palette 통합
- docs 작업 같이 하기

---

## 검증

작업 후 반드시 아래를 설명하라.

1. `tunaDish`의 어떤 구조를 참고했고, `tunaFlow`에서는 왜 Rust DB 방식으로 구현했는지
2. FTS 테이블/트리거를 어떻게 추가했는지
3. 검색 UI를 어디에 붙였는지
4. 결과 클릭 시 어떤 이동을 하는지
5. 타입체크/빌드/가능한 검증 결과
6. 남은 리스크

---

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. Message Search Flow
### D. Verification
### E. Remaining Risks

바로 코드 수정까지 진행하라.
