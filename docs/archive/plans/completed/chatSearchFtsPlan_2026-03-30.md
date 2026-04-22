# Chat Search FTS Plan

상태: 제안
작성: 2026-03-30

## 목표

현재 `CenterPanel` 우측 상단의 검색 placeholder를 실제 메시지 검색 UX로 전환한다.

핵심 목표:
- SQLite FTS5 기반 메시지 전문 검색
- 검색 결과 dropdown
- 결과 클릭 시 해당 conversation으로 이동
- 현재 메인 탭/프로젝트 구조와 충돌 없이 붙일 것

## 현재 전제

- `messages_fts`는 문서상 스키마만 존재하는 상태로 정리돼 있다
- `CenterPanel` 우측에 이미 검색 슬롯이 있다
- 현재 제품은 `Chat / Plan / Artifacts / Review / Test` 중심 구조다

## 범위

### Backend

- `messages_fts` virtual table
- insert/update/delete trigger
- `search_messages(query, limit, project_id?)` 또는 동등 command
- 최소 반환 필드:
  - `id`
  - `conversation_id`
  - `content`
  - `timestamp`
  - `rank`
- 가능하면 추가:
  - `role`
  - `engine`
  - `conversation_label`

### Frontend

- `CenterPanel` 우측 검색 placeholder를 실제 입력창으로 전환
- 2글자 이상에서 debounce 검색
- 결과 dropdown
- 로딩/빈 결과 상태
- 결과 클릭 시 conversation 이동

## 권장 UX

- 기본 위치: `CenterPanel` toolbar 우측
- 1차는 project 범위 검색이 가장 자연스럽다
- 결과는 conversation 기준 맥락이 보여야 한다
- 1차에서는 특정 message scroll/jump는 하지 않는다

## 비목표

- command palette 통합
- recent search history
- 특정 message 자동 스크롤
- Artifacts/Plans/Files 통합 검색
- vector search

## 단계

### Phase 1

- FTS table/trigger/command
- 검색 입력 + debounce + dropdown
- conversation 이동

### Phase 2

- snippet/highlight polish
- role/engine/conversation label 표시

### Phase 3

- project-only / all toggle
- message jump

## 검증 기준

- 새 메시지 저장 후 FTS 인덱스가 반영된다
- 2글자 이상 입력 시 결과가 보인다
- 결과 클릭 시 올바른 conversation으로 이동한다
- 기존 탭/레이아웃을 깨지 않는다

## 메모

이 작업은 현재 `Knowledge Sources`보다 우선한다. 지금은 새 지식 공급층보다 이미 쌓인 대화/브랜치/아티팩트를 다시 찾는 능력이 제품 체감에 더 직접적이다.
