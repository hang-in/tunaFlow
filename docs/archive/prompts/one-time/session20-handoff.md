# 세션 20 핸드오프 프롬프트

> 아래 내용을 새 세션의 첫 메시지로 사용하세요.

---

tunaFlow 세션 20 시작. **장기기억 자동 트리거 배선 + main 머지 준비 세션**.

## 프로젝트 개요

tunaFlow는 **다중 에이전트 오케스트레이션 클라이언트(AOC)**. Tauri 2 + React + TypeScript + Rust + SQLite.
프로젝트 단위로 Claude/Codex/Gemini 에이전트를 실행하며, Roundtable 토론, Branch 분기, Plan/Artifact 관리, ContextPack 맥락 조립 등을 지원한다.

**"Of the agent, By the agent, For the agent"** — 에이전트가 편해야 결과가 좋아진다는 철학.

## 현재 상태

- **브랜치**: `feature/context-tiering` (main에 미머지, origin push 완료)
- **세션 19 성과**: HTTP API E2E 테스트 + Branch/RT 엔드포인트 9개 + 메모리/검색 7개 + 코덱스 리뷰 대응 + ContextPack 주입 수정
- **테스트**: Rust 197 + Integration 19 + Frontend 175 = **391 tests** (전부 통과)
- **DB 버전**: v30 (vec_chunks sqlite-vec)
- **HTTP API**: 총 27개 엔드포인트 (읽기 11 + 쓰기 16)

## 이번 세션의 목표

### 목표 1: 장기기억 자동 트리거 배선 (P0)

현재 장기기억 인프라(테이블, 검색, 압축)는 **전부 구현됐지만** 자동 트리거가 없어서 수동 커맨드로만 동작합니다. send 경로의 에이전트 완료 시점에 자동 배선해야 합니다.

#### 배선해야 할 3개 트리거

| 트리거 | 함수 | 위치 | 시점 |
|--------|------|------|------|
| 메모리 압축 | `compress_memory_blocking()` | `conversation_memory.rs:537` | 에이전트 완료 후, 메시지 12개+ 이상일 때 |
| 세션 링크 갱신 | `discover_related_sessions()` | `session_discovery.rs:32` | 에이전트 완료 후, 비동기 |
| 벡터 인덱싱 | `index_chunks_blocking()` | `vector_search.rs:514` | 에이전트 완료 후, 비동기 |

#### 배선 위치

**Tauri command 경로** (UI에서 사용):
- `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` — `finalize_engine_run()` 함수
- 에이전트 응답을 DB에 저장한 직후 `tokio::spawn(async { spawn_blocking(|| { ... }) })` 패턴으로 비동기 실행
- hot path에 영향 없도록 fire-and-forget

**HTTP API 경로**:
- `src-tauri/src/http_api.rs` — `send_message` 핸들러의 `Ok(Ok(out))` 분기
- assistant 메시지 저장 직후 동일 트리거 추가

#### 주의사항

- `compress_memory_blocking()`은 **Claude를 호출**합니다 (LLM 요약). 에이전트가 방금 완료된 직후에 또 Claude를 호출하면 rate limit 경쟁. **threshold 체크 후 조건부 실행** 필요 (`needs_compression()` 함수가 이미 있음).
- `index_chunks_blocking()`은 **rawq embed**를 호출합니다. rawq daemon이 준비 안 됐으면 skip.
- 세 트리거 모두 실패해도 **메인 응답에 영향 없어야** 합니다 (에러 로깅만).

### 목표 2: main 머지 준비

`feature/context-tiering` 브랜치를 main에 머지하기 전 체크리스트:

1. **빌드 확인**: `cargo check` + `npx tsc --noEmit` + `npx vite build`
2. **테스트 통과**: `cargo test --lib` + `cargo test --test db_integration` + `npx vitest run`
3. **CSP 정리**: `tauri.conf.json`에 DOOM 이스터에그용 `unsafe-eval`이 추가됨 — 제거하거나 주석 표시
4. **CLAUDE.md 갱신**: §5에 세션 19 성과 추가, §8에 HTTP API 엔드포인트 목록 추가
5. **실사용 검증**: 앱에서 대화 → 에이전트 응답 → 브랜치 → RT → 자동 트리거 동작 확인

### 목표 3 (여유 시): agentStreamHelper 실제 적용

`src/stores/slices/agentStreamHelper.ts`에 공용 유틸이 준비됐으나 runtimeSlice/threadSlice에 아직 적용 안 됨. 적용하면 ~100줄 중복 제거.

### 목표 4 (여유 시): DOOM 이스터에그 완성

별도 Tauri WebView 창 방식. js-dos v8이 CSP/CORS 문제로 미동작. 해결 방향:
- `doom` 창의 CSP를 별도 설정 (메인 앱과 격리)
- 또는 js-dos 대신 Chocolate Doom WASM 직접 빌드

## ⚠️ 중요 규칙

1. **숏컷 금지**: 새 실행 경로를 만들 때 기존 프로덕션 경로와 동일한 함수를 호출할 것. `system_prompt: None` 같은 생략 절대 금지.
2. **에러 삼킴 금지**: `.ok()`, `.unwrap_or_default()`, 빈 catch는 명시적 이유 없이 사용하지 말 것.
3. **단일 경로 수정 원칙**: 한 번에 여러 실행 경로를 동시에 바꾸지 않는다.
4. **사이드 이펙트 체크**: Store 상태를 바꿀 때 해당 상태를 읽는 모든 컴포넌트/훅을 grep으로 확인한다.
5. **테스트 대화 label에 `[E2E]` 접두사**: HTTP API 테스트 시.
6. **에이전트 실행 시 비용 주의**: Haiku 또는 Gemini Flash 사용. RT는 다른 엔진 혼용 권장 (claude + gemini + codex).

## 아키텍처 참조

### HTTP API 엔드포인트 (27개)

| 카테고리 | Method | Path | 설명 |
|---------|--------|------|------|
| Health | GET | `/api/health` | 서버 상태 (인증 불필요) |
| **읽기** | GET | `/api/projects` | 프로젝트 목록 |
| | GET | `/api/conversations?projectKey=X` | 대화 목록 |
| | GET | `/api/conversations/:id/messages` | 메시지 목록 |
| | GET | `/api/plans`, `/api/plans/:id`, `/api/plans/:id/events` | Plan CRUD |
| | GET | `/api/artifacts` | Artifact 목록 |
| | GET | `/api/agents/status` | 실행 중 에이전트 |
| | GET | `/api/conversations/:id/branches` | Branch 목록 |
| | GET | `/api/conversations/:id/memory/status` | 메모리 상태 |
| | GET | `/api/conversations/:id/session-links` | 세션 링크 |
| | GET | `/api/conversations/:id/traces` | 트레이스 로그 |
| **쓰기** | POST | `/api/projects` | 프로젝트 생성 |
| | POST | `/api/conversations` | 대화 생성 |
| | POST | `/api/conversations/:id/send` | 메시지 전송 (ContextPack 포함) |
| | POST | `/api/conversations/:id/delete` | 대화 삭제 |
| | POST | `/api/plans/:id/approve` | Plan 승인 |
| | POST | `/api/branches` | Branch 생성 |
| | DELETE | `/api/branches/:id` | Branch 삭제 |
| | POST | `/api/branches/:id/archive` | Branch 아카이브 |
| | POST | `/api/branches/:id/adopt` | Branch 병합 (전체 assistant 요약) |
| | POST | `/api/branches/:id/rename` | Branch 이름 변경 |
| | POST | `/api/roundtables/run` | RT 실행 (background) |
| | POST | `/api/roundtables/:id/cancel` | RT 취소 |
| | POST | `/api/conversations/:id/memory/compress` | 메모리 압축 |
| | POST | `/api/conversations/:id/session-links/refresh` | 세션 링크 갱신 |
| | POST | `/api/conversations/:id/chunks/index` | 벡터 인덱싱 |
| | POST | `/api/conversations/:id/chunks/search` | 벡터 검색 |
| **WS** | GET | `/ws/events` | 실시간 이벤트 |

### 장기기억 시스템 현황

| 영역 | 인프라 | 자동 트리거 | 품질 검증 |
|------|--------|-----------|----------|
| verbatim 저장 | ✅ messages 테이블 | — (항상 저장) | ✅ |
| 토픽별 압축 | ✅ conversation_memory | ❌ 수동만 | ✅ 6 topics 생성 확인 |
| 세션 링크 | ✅ session_links + FTS5 | ❌ 수동만 | ✅ score=1.000 유사 대화 발견 |
| 벡터 검색 | ✅ conversation_chunks + vec_chunks | ❌ 수동만 | ✅ score=0.814 크로스세션 |
| 크로스 세션 recall | ✅ ContextPack 주입 | ⚠️ 세션 링크 의존 | ⚠️ 새 대화 첫 턴에서 링크 없음 |

### 세션 19 E2E 품질 테스트 결과

- **같은 대화 내 맥락 유지**: ✅ Zustand, SSE 결정 기억 (3/3)
- **크로스 세션 recall**: ⚠️ 2/3 기억 (SQLite 미발견 — 세션 링크 부재)
- **ContextPack 빌드**: ✅ prompt=7036chars system=6907chars
- **벡터 검색 품질**: ✅ score 0.50~0.81 범위

### 코덱스 리뷰 대응 현황

| # | 항목 | 상태 |
|---|------|------|
| 1 | 토큰 uuid 교체 | ✅ 완료 |
| 2 | query_map unwrap | ✅ 패턴 준비 |
| 3 | async mutex 패턴 | ✅ with_read_db/with_write_db 헬퍼 |
| 4 | runtime↔thread 중복 | ✅ agentStreamHelper 유틸 (미적용) |
| 5 | 문서 SSOT | ✅ 완료 |
| 6 | 컴포넌트 분할 | ✅ useNavigationChain + useTraceData |
| 7 | 테스트 추가 | ✅ integration 12→19 |

## 참고 파일

| 파일 | 역할 |
|------|------|
| `src-tauri/src/http_api.rs` | HTTP API 서버 (~950줄) |
| `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` | 에이전트 완료 후 DB 저장 (자동 트리거 배선 위치) |
| `src-tauri/src/commands/conversation_memory.rs` | 압축 메모리 (compress_memory_blocking) |
| `src-tauri/src/commands/vector_search.rs` | 벡터 인덱싱/검색 (index_chunks_blocking) |
| `src-tauri/src/commands/session_discovery.rs` | 세션 관계 발견 (discover_related_sessions) |
| `src/stores/slices/agentStreamHelper.ts` | 스트리밍 공용 유틸 (미적용) |
| `src/stores/slices/runtimeSlice.ts` | 메인 채팅 실행 (중복 대상) |
| `src/stores/slices/threadSlice.ts` | 드로어 채팅 실행 (중복 대상) |
| `src/components/tunaflow/DoomModal.tsx` | DOOM 이스터에그 (WIP) |

## 빌드 / 실행 / 테스트

```bash
npm run tauri dev              # 개발 실행
npx tsc --noEmit               # TypeScript 체크
npx vite build                 # Frontend 빌드
cd src-tauri && cargo check    # Rust 체크
cd src-tauri && cargo test --lib        # Rust unit (197 tests)
cd src-tauri && cargo test --test db_integration  # Integration (19 tests)
npx vitest run                 # Frontend (175 tests)
```

## 앱 실행 후 HTTP API 토큰

```bash
# 콘솔에 출력됨
[startup] HTTP API token: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
# 사용
curl -H "Authorization: Bearer $TOKEN" http://127.0.0.1:19840/api/health
```
