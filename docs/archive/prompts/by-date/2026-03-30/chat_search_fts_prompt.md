# Chat Search FTS

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

먼저 읽을 문서:
- `docs/plans/chatSearchFtsPlan_2026-03-30.md`
- `docs/plans/messageSearchAdoptionPlan.md`

먼저 확인할 파일:
- `src/components/tunaflow/CenterPanel.tsx`
- `src-tauri/src/db/schema.rs`
- `src-tauri/src/db/migrations.rs`
- `src-tauri/src/commands/messages.rs`
- `src-tauri/src/lib.rs`

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 `Knowledge Sources`보다 `Chat Search`가 먼저인지
- 왜 기존 Rust DB 레이어에 FTS를 붙이는 것이 맞는지

이번 작업 목표:
- `CenterPanel` 우측 검색 placeholder를 실제 메시지 검색 UX로 바꾸고, SQLite FTS5를 Rust DB 레이어에 붙여라.

구현 범위:
1. `messages_fts` virtual table + insert/update/delete trigger
2. `search_messages(query, limit, project_id?)` 또는 동등 command
3. `CenterPanel` 검색 입력창 + debounce + dropdown
4. 결과 클릭 시 해당 conversation으로 이동
5. 가능하면 결과에 conversation label과 snippet을 보여줄 것

비목표:
- command palette
- recent history
- artifacts/plans/files 통합 검색
- message scroll/jump
- vector search
- Knowledge Sources 구현

검증:
- cargo check
- tsc --noEmit
- 새 메시지 저장 후 검색 반영 확인
- 결과 클릭 시 conversation 이동 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Search Flow
### D. Verification
### E. Residual Gaps
