# 세션 21 핸드오프 프롬프트

> 아래 내용을 새 세션의 첫 메시지로 사용하세요.

---

tunaFlow 세션 21 시작. **bge-m3 임베딩 모델 도입 + PTY 안정화 + 문서 정리 세션**.

## 프로젝트 개요

tunaFlow는 **다중 에이전트 오케스트레이션 클라이언트(AOC)**. Tauri 2 + React + TypeScript + Rust + SQLite.
프로젝트 단위로 Claude/Codex/Gemini 에이전트를 실행하며, Roundtable 토론, Branch 분기, Plan/Artifact 관리, ContextPack 맥락 조립 등을 지원한다.

**"Of the agent, By the agent, For the agent"** — 에이전트가 편해야 결과가 좋아진다는 철학.

## 현재 상태

- **브랜치**: `feature/context-tiering` (main에 미머지)
- **세션 20 성과**: 장기기억 자동 트리거 + 문서 RAG + write lock 버그 수정 5건 + MCP 도구 3개 + plans/index 재분류
- **테스트**: Rust 216 + Integration 25 + Frontend 175 = **416 tests** (전부 통과)
- **DB 버전**: v31 (document_edges + document_index_status + conversation_chunks 확장)
- **HTTP API**: 총 37개 엔드포인트 (기존 27 + 문서 RAG 5 + MCP 도구 5)

## 이번 세션의 목표

### 목표 1: bge-m3 ONNX 임베딩 모델 도입 (P0)

현재 문서 RAG 검색 품질이 낮습니다.

| 문제 | 원인 |
|------|------|
| 관련 문서 score 54-70%, 무관 문서 score 57-58% | snowflake-arctic-embed-s(384dim)는 코드 검색용 소형 모델 |
| 관련/무관 구분 불가 (차이 12%p) | multilingual 문서 검색에 부적합 |
| 한국어 검색 품질 낮음 | 한국어 최적화 안 된 모델 |

#### 해결: seCall의 bge-m3 ONNX 구현 포팅

seCall(`/Users/d9ng/privateProject/seCall`)에 완전한 구현이 있습니다:

| 구성요소 | seCall 위치 | 설명 |
|---------|-----------|------|
| ONNX Runtime | `ort = "2.0.0-rc.10"` (Cargo.toml) | session pool(4개), GraphOptimization Level3 |
| bge-m3 모델 | `~/.cache/secall/models/bge-m3-onnx/` | 1024dim, HuggingFace 자동 다운로드 |
| Embedder trait | `crates/secall-core/src/search/embedding.rs:11-19` | `embed()`, `embed_batch()`, `is_available()`, `dimensions()` |
| OrtEmbedder | `crates/secall-core/src/search/embedding.rs:106-375` | mean pooling + L2 정규화, attention mask |
| 모델 관리 | `crates/secall-core/src/search/model_manager.rs` | 자동 다운로드 + SHA256 검증 |
| kiwi-rs | `crates/secall-core/src/search/tokenizer.rs:68-133` | BM25/FTS5용 한국어 형태소 분석 |

#### 적용 계획

1. **Cargo.toml**: `ort`, `tokenizers`, `ndarray` 의존성 추가
2. **새 모듈**: `src-tauri/src/agents/embedder.rs` — seCall의 OrtEmbedder 포팅
3. **DB 스키마**: vec_chunks를 384dim→1024dim으로 교체 (v32 마이그레이션)
4. **역할 분리**: rawq = 코드 검색 전용, bge-m3 = 문서 RAG + 대화 벡터 검색
5. **기존 인덱스 재생성**: conversation_chunks + document chunks 전부 재임베딩
6. **모델 파일 공유**: seCall과 같은 `~/.cache/` 경로 사용 (중복 다운로드 방지)

#### 주의사항

- bge-m3 모델은 ~1.1GB — 첫 실행 시 자동 다운로드
- ONNX Runtime은 native 바이너리 — Tauri 빌드 시 dylib 포함 필요
- `embed_batch()`로 배치 처리하면 인덱싱 속도 10x+ 개선 가능 (현재 rawq는 1건씩 subprocess)
- vec_chunks 교체 시 기존 벡터 인덱스 무효화 — 마이그레이션에서 처리

### 목표 2: PTY 안정화 (P0)

현재 PTY 모드에서 메시지가 전달되지 않는 경우가 있습니다.

#### 발견된 문제

| 문제 | 원인 | 현재 상태 |
|------|------|----------|
| `pty_write` 후 Claude가 응답 안 함 | bracket paste가 전달됐지만 Claude가 인식 못함 (idle 상태 불일치) | 미수정 |
| streaming 상태가 영구 지속 | 에러 시 orphan streaming 복구 안 됨 | 미수정 |
| 전달 확인 없음 | fire-and-forget — 성공/실패 모름 | 미수정 |

#### 필요한 수정

1. **전달 확인**: `pty_write` 후 `pty:screen`에서 idle prompt(`❯`)가 사라졌는지 체크
2. **시작 타임아웃**: N초 내 JSONL에 새 항목 안 나오면 재전송 또는 에러 표면화
3. **Orphan 복구**: streaming 상태에서 PTY 프로세스가 idle이면 자동 정리
4. **Fallback**: PTY 전달 실패 시 -p 모드로 자동 전환

#### 관련 파일

| 파일 | 역할 |
|------|------|
| `src/stores/slices/ptyMessageSender.ts` | 핵심 — PTY write + JSONL 폴링 + 완료 감지 |
| `src/stores/ptyStore.ts` | PTY 세션 상태 + 완료 감지(detectCompletion) |
| `src/stores/slices/conversationSlice.ts:21-102` | PTY spawn/resume 로직 |

### 목표 3: 2-Layer Skill System 도입

`docs/ideas/agentSkillsReferenceIdea.md` §7에 설계 완료. `agent-skills` 레포의 anatomy 패턴 적용.

| 레이어 | 용도 | 주입 조건 |
|--------|------|-----------|
| Reference skill (현재) | 라이브러리 사용법 | 키워드 매칭 |
| Procedural skill (신규) | 행동 절차 + 검증 기준 | workflow phase 전환 시 |

### 목표 4: 문서 정리 (에이전트에게 시키기)

문서 RAG가 동작하므로 에이전트에게 문서 정리를 시킬 수 있습니다.

```bash
# HTTP API로 문서 인덱싱 트리거
curl -X POST -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:19840/api/projects/tunaflow/documents/index

# Orphan 문서 확인 (91개)
curl -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:19840/api/projects/tunaflow/documents/orphans

# 문서 그래프 (298 edges)
curl -H "Authorization: Bearer $TOKEN" \
  http://127.0.0.1:19840/api/projects/tunaflow/documents/graph
```

## ⚠️ 중요 규칙

1. **숏컷 금지**: 새 실행 경로를 만들 때 기존 프로덕션 경로와 동일한 함수를 호출할 것.
2. **에러 삼킴 금지**: `.ok()`, `.unwrap_or_default()`, 빈 catch는 명시적 이유 없이 사용하지 말 것.
3. **프로덕션 레벨**: 모든 핵심 기능(장기기억, RT, 워크플로우, 검색)은 정확하게 동작해야 함. 대충 넘어가면 안 됨.
4. **검색 품질**: 관련 문서 score와 무관 문서 score의 차이가 최소 20%p 이상이어야 함. 현재 12%p → bge-m3로 개선 필요.
5. **PTY 안정성**: fire-and-forget 패턴 금지. 전달 확인 + 타임아웃 + fallback 필수.

## 세션 20에서 해결한 것

### 장기기억 자동 트리거

| 트리거 | 시점 | 위치 |
|--------|------|------|
| 메모리 압축 | 에이전트 완료 후, 12+ 메시지 | `spawn_post_completion_tasks()` |
| 세션 링크 | 에이전트 완료 후 | read lock으로 discover → write lock으로 save |
| 벡터 인덱싱 | 에이전트 완료 후 | rawq daemon ready 시에만 |

### 문서 RAG

| 항목 | 상태 |
|------|------|
| DB v31 | ✅ conversation_chunks 확장 + document_edges + document_index_status |
| 마크다운 파서 | ✅ ## 섹션 분할 + 링크 추출 + 파일명 멘션 추출 |
| 인덱싱 | ✅ 366파일, 6430 chunks, 298 edges (tunaflow), 비동기 백그라운드 |
| 검색 | ✅ vec0 KNN, threshold 0.5 — 하지만 품질 부족 (모델 한계) |
| ContextPack 주입 | ✅ "Related project documentation" 섹션, Standard+ 모드 |
| HTTP API | ✅ 5개 (index/search/graph/orphans/status) |
| MCP 도구 | ✅ 3개 (search_documents/get_document_graph/get_orphan_documents) |
| Tauri commands | ✅ 5개 |

### Write Lock 버그 수정

| 문제 | 수정 |
|------|------|
| compress_memory_blocking이 write lock으로 읽기 | read lock으로 변경 |
| refresh_links가 discover+save를 하나의 write lock | read/write 분리 |
| 문서 인덱싱 Phase 3 전체 파일 lock | 파일 단위 lock/unlock |
| 에이전트 완료마다 문서 재인덱싱 | post-completion에서 제거 |
| is_daemon_ready 타임아웃 없음 | 3초 타임아웃 추가 |

### 기타

- plans/index.md 재분류: 15개 `진행 예정→완료`, 2개 상향
- agentSkillsReferenceIdea.md에 2-Layer Skill System 설계 추가
- 인덱싱을 HTTP 동기→비동기 백그라운드로 전환

## 아키텍처 참조

### 검색 시스템 현황

| 경로 | 모델 | 대상 | 품질 |
|------|------|------|------|
| **rawq** | snowflake-arctic-embed-s (384dim) | 코드 파일 | ✅ 코드 검색에 적합 |
| **conversation vector** | snowflake-arctic-embed-s (384dim) | 대화 메시지 | ⚠️ 한국어 약함 |
| **document RAG** | snowflake-arctic-embed-s (384dim) | 프로젝트 문서 | ❌ 품질 부족 |
| **FTS5** | SQLite built-in | 대화 키워드 | ⚠️ 한국어 형태소 미지원 |
| **context-hub** | CLI search | 외부 라이브러리 | ✅ |

→ bge-m3(1024dim) 도입 후: conversation vector + document RAG를 bge-m3로 교체. rawq는 코드 검색 전용 유지.

### DB 스키마 (v31)

세션 20에서 추가된 테이블:

| 테이블 | 용도 |
|--------|------|
| `document_edges` | 문서 간 참조 관계 (source_path, target_path, relation='link'|'mention') |
| `document_index_status` | SHA-256 변경 감지 (project_key, file_path, content_hash) |

conversation_chunks 확장 컬럼:
- `source_type` TEXT DEFAULT 'conversation' — 'conversation' | 'document'
- `file_path` TEXT — 문서 경로
- `section_title` TEXT — ## 섹션 제목

sentinel 대화: `__doc__:{projectKey}` (FK 만족용, usage_status='archived')

### HTTP API 엔드포인트 (37개)

기존 27개 + 세션 20 추가:

| Method | Path | 설명 |
|--------|------|------|
| POST | `/api/projects/:key/documents/index` | 문서 인덱싱 (비동기, 즉시 ACCEPTED) |
| POST | `/api/projects/:key/documents/search` | 문서 벡터 검색 |
| GET | `/api/projects/:key/documents/graph` | 에지 그래프 |
| GET | `/api/projects/:key/documents/orphans` | orphan 문서 |
| GET | `/api/projects/:key/documents/status` | 인덱스 상태 |

### MCP 서버 도구 (8개)

기존 5개 + 세션 20 추가:

| 도구 | 설명 |
|------|------|
| `search_documents` | 프로젝트 문서 LIKE 검색 (벡터 미지원 — MCP에서 rawq 호출 불가) |
| `get_document_graph` | 에지 그래프 |
| `get_orphan_documents` | orphan 문서 |

### 알려진 이슈

| 이슈 | 우선순위 | 상태 |
|------|---------|------|
| 문서 삭제 시 DB 잔존 | P2 | 메모리에 기록 |
| MCP search_documents가 LIKE (벡터 아님) | P1 | bge-m3 도입 후 개선 |
| rawq daemon 과부하 시 embed 느려짐 | P2 | bge-m3 분리로 해소 |
| PTY 메시지 전달 실패 (fire-and-forget) | P0 | 목표 2에서 수정 |
| kiwi-rs 한국어 형태소 (FTS5 개선) | P1 | seCall에서 포팅 |

## 참고 파일

| 파일 | 역할 |
|------|------|
| `src-tauri/src/commands/document_index.rs` | 문서 RAG 핵심 (파서 + 인덱싱 + 검색 + 그래프) |
| `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` | 자동 트리거 (spawn_post_completion_tasks) |
| `src-tauri/src/commands/agents_helpers/send_common/context_loading.rs` | ContextPack 문서 검색 주입 |
| `src-tauri/src/agents/rawq.rs` | rawq CLI 래퍼 (embed_text, is_daemon_ready) |
| `src-tauri/src/http_api.rs` | HTTP API 서버 (~1100줄) |
| `mcp-server/index.mjs` | MCP 서버 (8개 도구) |
| `src/stores/slices/ptyMessageSender.ts` | PTY 메시지 전송 (수정 필요) |
| `src/stores/ptyStore.ts` | PTY 세션 상태 |
| `docs/ideas/agentSkillsReferenceIdea.md` | 2-Layer Skill System 설계 |
| `/Users/d9ng/privateProject/seCall/crates/secall-core/src/search/embedding.rs` | bge-m3 OrtEmbedder 레퍼런스 |
| `/Users/d9ng/privateProject/seCall/crates/secall-core/src/search/model_manager.rs` | 모델 자동 다운로드 레퍼런스 |
| `/Users/d9ng/privateProject/seCall/crates/secall-core/src/search/tokenizer.rs` | kiwi-rs 한국어 토크나이저 레퍼런스 |

## 빌드 / 실행 / 테스트

```bash
npm run tauri dev              # 개발 실행
npx tsc --noEmit               # TypeScript 체크
npx vite build                 # Frontend 빌드
cd src-tauri && cargo check    # Rust 체크
cd src-tauri && cargo test --lib        # Rust unit (216 tests)
cd src-tauri && cargo test --test db_integration  # Integration (25 tests)
npx vitest run                 # Frontend (175 tests)
```

## 앱 실행 후 HTTP API 토큰

```bash
# 콘솔에 출력됨 (stderr)
[startup] HTTP API token: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
# 사용
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:19840/api/health
```
