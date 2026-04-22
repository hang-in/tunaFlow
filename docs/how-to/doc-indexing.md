---
title: Document Indexing Operations
updated_at: 2026-04-22
canonical: true
status: active
owner: tunaFlow-core
---

# Document Indexing Operations

tunaFlow 의 project 문서 RAG 인덱스 운영 가이드. 검색 품질의 전제 = **file system 과 DB 인덱스가 일치** 하는 것.

## 개념

- 인덱싱 대상: 프로젝트 root 의 `CLAUDE.md`, `README.md` 등 top-level `*.md` + `docs/**/*.md` 전부.
- 저장 위치: `conversation_chunks` (source_type='document'), `vec_chunks` (embedding), `document_edges`, `document_index_status`.
- 검색: `POST /api/v1/projects/{key}/documents/search` 또는 Tauri `search_project_docs`.

## 일상 동작

- 프로젝트 선택 시 1회 `ensure_rawq_index` 호출 → incremental indexing (SHA change detection).
- fs watcher 가 파일 변경 감지 시 자동 재인덱싱 (일부 이벤트 놓칠 수 있음).
- 따라서 **대규모 reorganization 이후** 에는 수동 재인덱싱 권장.

## 재인덱싱 (bulk sync)

문서 대량 이동 / 삭제 / 생성 후 DB 재동기화:

```bash
# 기본: cleanup (stale 정리) + force reindex (SHA check 우회)
export TUNAFLOW_TOKEN=<Settings > Mobile 에서 복사>
node scripts/reindex-docs.mjs tunaflow

# stale cleanup 만 skip (파일이 실제로 정리된 게 아닐 때)
node scripts/reindex-docs.mjs tunaflow --no-cleanup

# force 끄고 incremental 만 (SHA 변경된 파일만)
node scripts/reindex-docs.mjs tunaflow --no-force
```

응답 202 accepted 후 background 진행. 결과는 WS event `document:indexed` 로 발행.

## 검증

재인덱싱 후 지표 확인:

```bash
# 1. fs 의 .md 파일 수
find docs -name "*.md" | wc -l
# 프로젝트 root 의 README, CLAUDE.md 등은 별도 카운트

# 2. DB 에 인덱싱된 고유 파일 수
sqlite3 ~/.tunaflow/db/tunaflow.db \
  "SELECT COUNT(DISTINCT file_path) FROM conversation_chunks
   WHERE project_key='tunaflow' AND source_type='document';"

# 3. chunks 수
sqlite3 ~/.tunaflow/db/tunaflow.db \
  "SELECT COUNT(*) FROM conversation_chunks
   WHERE project_key='tunaflow' AND source_type='document';"

# 4. stale 후보 (DB 에는 있지만 fs 에는 없는 것)
sqlite3 ~/.tunaflow/db/tunaflow.db \
  "SELECT DISTINCT file_path FROM conversation_chunks
   WHERE project_key='tunaflow' AND source_type='document'" \
  | while read p; do
      [ ! -f "$p" ] && echo "STALE: $p"
    done | head -20
```

정상 지표:
- fs 파일 수 ≈ DB distinct file_path 수 (차이 ±5 내외는 OK — indexing 중일 수도)
- stale 파일 수 = 0 (cleanup 후)
- 파일당 chunks 는 문서 길이에 비례 (보통 3-50 chunks)

## 문제 진단

**"인덱싱됐다고 보고하지만 DB 에는 적다"**:
- rawq daemon 죽었는지 확인: `rawq daemon status`
- embedder 초기화 실패: dev terminal 의 `[embedder]` 로그
- 파일 읽기 권한: `ls -la <file>`

**"stale chunks 가 계속 남는다"**:
- fs watcher 가 삭제 이벤트를 놓쳤을 가능성. `--cleanup` 포함 재인덱싱 실행.

**"embedding=NULL 로 남는 chunks"**:
- bge-m3 ONNX 추론 실패. `crate::commands::vector_search::backfill` 가 앱 시작 시 자동 복구 시도.
- 수동 복구: 해당 project 재인덱싱 (`--force`).

## 관련 명령

| 계층 | 엔드포인트 |
|------|-----------|
| Tauri | `index_project_docs({ projectKey, force })` |
| Tauri | `cleanup_project_stale_docs({ projectKey })` |
| HTTP | `POST /api/v1/projects/{key}/documents/index?force=true&cleanup=true` |
| HTTP | `POST /api/v1/projects/{key}/documents/search` (body: `{ query, limit }`) |
| HTTP | `GET /api/v1/projects/{key}/documents/status` |
