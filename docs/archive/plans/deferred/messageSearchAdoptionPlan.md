# tunaFlow Message Search Adoption Plan

## 목적

`tunaDish`의 채팅 검색 UX를 참고해 `tunaFlow`에 **대화/메시지 전문 검색**을 도입한다.

핵심 목표:
- 상단 검색 입력 또는 헤더 검색 UI에서 빠르게 메시지 검색
- 검색 결과 클릭 시 해당 conversation으로 이동
- 현재 `tunaFlow`의 로컬 DB 구조와 충돌 없이 붙일 것

## tunaDish 참고 구현

참고 파일:
- `D:\privateProject\tunaDish\client\src\lib\db.ts`
- `D:\privateProject\tunaDish\client\src\components\layout\TopNav.tsx`
- `D:\privateProject\tunaDish\client\src\components\layout\MessageSearchResults.tsx`
- `D:\privateProject\tunaDish\client\src-tauri\src\lib.rs`
- `D:\privateProject\tunaDish\client\package.json`

확인 결과:
- 저장소: `tauri-plugin-sql`
- 검색 방식: SQLite `FTS5`
- 구현 포인트:
  - `messages_fts` 가상 테이블
  - insert/update/delete trigger
  - `searchMessages(query)` helper
  - 헤더 입력창 + dropdown 결과 UI

## tunaFlow 적용 판단

중요:
- `tunaDish`는 프론트에서 `@tauri-apps/plugin-sql`로 SQLite를 직접 다룸
- `tunaFlow`는 이미 Rust backend에서 [lib.rs](/D:/privateProject/tunaFlow/src-tauri/src/lib.rs) + `rusqlite`로 DB를 관리 중

따라서 `tunaFlow`는 **`plugin-sql`을 새로 넣어 따라가기보다**, 아래처럼 가는 것이 맞다.

### 권장 방식

1. `tunaDish`의 **FTS 설계**는 참고
2. 실제 구현은 `tunaFlow`의 기존 Rust DB 레이어에 추가
3. 프론트는 새 Tauri command를 호출

즉:
- 참고 대상: `tunaDish`의 UX와 SQL 패턴
- 실제 구현 대상: `tunaFlow`의 existing `rusqlite` schema/commands

## 왜 plugin-sql을 그대로 안 쓰는가

1. `tunaFlow`는 이미 단일 진실원으로 Rust DB를 사용 중
2. 여기에 프론트용 SQL 플러그인을 추가하면 DB 접근 계층이 이중화될 수 있음
3. 권한/스키마/마이그레이션/에러 처리 흐름이 분산됨

즉 이 기능은:
- `tunaDish`: plugin-sql
- `tunaFlow`: Rust command

으로 구현 경로만 다르고, UX는 유사하게 맞추는 것이 가장 자연스럽다.

## 1차 범위

### Backend

1. `messages_fts` 가상 테이블 추가
2. `messages`와 연동하는 insert/update/delete trigger 추가
3. `search_messages(query, limit)` command 추가

반환 최소 필드:
- `id`
- `conversation_id`
- `content`
- `timestamp`
- `rank`

### Frontend

1. 검색 입력창 추가
2. debounce 적용
3. 결과 dropdown 표시
4. 결과 클릭 시 해당 conversation 선택
5. 검색어 하이라이트 + snippet 표시

## 추천 UI 위치

1차는 `tunaDish`처럼 **상단 헤더 검색창**이 가장 무난하다.

이유:
- 프로젝트 전체/대화 전체 검색이라는 성격에 맞음
- 우측 패널/입력창과 역할이 덜 충돌함
- 나중에 command palette와 합치기 쉬움

## SQL 방향

`tunaDish`의 핵심 패턴:

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS messages_fts USING fts5(
  content,
  content='messages',
  content_rowid='rowid',
  tokenize='unicode61'
);
```

`tunaFlow`도 비슷한 방향이 맞다.

검색 쿼리 방향:

```sql
SELECT m.id, m.conversation_id, m.content, m.timestamp, rank
FROM messages_fts fts
JOIN messages m ON m.rowid = fts.rowid
WHERE messages_fts MATCH ?
ORDER BY rank
LIMIT ?
```

## 적용 순서

### Phase A
- backend FTS schema + trigger + command
- 간단한 결과 조회 검증

### Phase B
- 헤더 검색 입력
- debounce + dropdown
- 클릭 시 conversation 이동

### Phase C
- snippet/highlight polish
- 검색 결과 개수/빈 상태
- keyboard UX

## 주의사항

1. `messages` 테이블의 `rowid` 사용 가능성 확인
- 현재 schema가 `INTEGER PRIMARY KEY`가 아니어도 SQLite rowid는 존재할 수 있으나,
  실제 `tunaFlow` schema 기준으로 안전하게 확인 후 적용해야 함

2. migration idempotency
- 이미 있는 DB에 안전하게 붙어야 함

3. FTS와 status/progress_content 관계
- 1차는 `content`만 인덱싱
- `progress_content`는 제외

4. project-scoped filter는 후속
- 1차는 전체 검색이어도 괜찮지만,
  가능하면 current project 기준 필터를 넣는 것이 UX상 더 좋음

5. conversation 이동 후 message scroll
- 1차는 conversation 이동만 해도 충분
- 특정 message로 정확히 jump하는 건 후속

## 1차 성공 기준

아래가 되면 1차 성공으로 본다.

1. 메시지 저장 시 FTS 인덱스가 자동 반영됨
2. 검색어 2글자 이상 입력 시 결과가 보임
3. 결과 클릭 시 해당 conversation으로 이동함
4. build/typecheck 통과

## 후속 작업

1. 현재 project만 검색 / 전체 검색 toggle
2. 특정 message로 scroll/jump
3. 검색 결과에서 role/engine 배지
4. recent search history
5. command palette 통합
