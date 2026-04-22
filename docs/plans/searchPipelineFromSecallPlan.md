---
title: 검색 파이프라인 도입 — secall 참고 (Query Expansion + Hybrid RRF + Kiwi/Lindera)
status: planned
priority: P1
created_at: 2026-04-22
related:
  - docs/ideas/searchEnhancementFromSecallIdea.md
  - seCall/crates/secall-core/src/search/
  - src-tauri/src/commands/messages.rs         # search_messages (FTS5)
  - src-tauri/src/commands/document_index.rs   # search_project_docs (vector)
  - src-tauri/src/commands/vector_search/      # bge-m3 vector search
---

# 검색 파이프라인 도입 Plan

> idea `searchEnhancementFromSecallIdea.md` 의 plan 승격. secall 의 search 구현을 tunaFlow 스키마에 맞춰 참고 이식. Phase A→B→C 단계적 PR.

---

## 1. 현재 상태 진단

### 1.1 검색 경로가 세 갈래로 분산되어 있음

| 경로 | 엔트리 | 대상 | 엔진 | 통합 UI 여부 |
|---|---|---|---|---|
| 대화 검색 (FTS5) | `search_messages` (messages.rs) | `messages_fts` | SQLite FTS5 `unicode61` | 헤더 검색창 |
| 문서 검색 (vector) | `search_project_docs` (document_index.rs) | `conversation_chunks (source_type='document')` | bge-m3 cosine | 별도/미노출 |
| 코드 검색 | rawq sidecar | 프로젝트 소스코드 | 자체 embedding | 에이전트 tool-request 전용 |

**즉 사용자 "플랜" 검색 실패의 원인**: 헤더 검색창은 `messages_fts` 만 쿼리 → docs/plans/*.md 는 `source_type='document'` 라 miss. Recall 문제가 아니라 **검색 범위 자체** 가 원인.

### 1.2 FTS5 tokenizer 약점
- 기본 `unicode61` tokenizer 는 공백/구두점 기반 → 한국어 조사/어미 분리 X
- "플랜을 보여줘" → "플랜을" 통째로 토큰화 → "플랜" 매칭 실패
- 영↔한 동의어 매칭 X ("plan" ↔ "플랜" ↔ "계획")

### 1.3 secall 의 해결책 (요약)
| 기능 | secall | 효과 |
|---|---|---|
| Query expansion | `claude-p haiku` 로 동의어/영↔한 확장 + 7일 DB 캐시 | "플랜" → `plan 계획 roadmap...` — recall ↑ |
| Hybrid RRF | BM25 + Vector 결과 rank 기반 합성 (k=60) + 0~1 정규화 | 두 signal 통합 — precision + recall ↑ |
| Kiwi/Lindera | 애플리케이션 레이어 pre-tokenize → FTS5 `unicode61` 이 공백으로 split | 한국어 형태소 단위 인덱싱 |
| Session 다양성 | `diversify_by_session(max_per=2)` | UX — 한 세션 독식 방지 |

---

## 2. 목표

- 사용자가 **"플랜" 검색하면 `docs/plans/*.md`, 대화, 코드 모두** 적절한 가중치로 표시
- 한국어/영어 혼용 쿼리 자연스럽게 작동
- 검색 recall 향상으로 에이전트 tool-request 결과 품질도 덤으로 개선 (dogfood)
- secall 의 검증된 구현 패턴 재사용 → 이식 비용 최소화

---

## 3. Scope 경계

- rawq (코드 검색) 는 **포함 X** — 별도 엔진, 이 plan 대상 아님. 단 Hybrid 엔드포인트가 rawq 결과도 merge 할 수 있도록 extensibility 는 확보.
- Graph filter (topic/file/issue 기반 사전 필터) 는 **별도 plan** — 데이터 모델 추가 필요.
- 영문 stemmer 는 후순위.

---

## 4. Phase A — Query Expansion

> 독립 PR. Phase B/C 없이도 작동. ROI 가장 높음.

### 4.1 범위
- secall 의 `query_expand.rs` 를 tunaFlow 로 이식
- 핵심: `claude-p haiku` subprocess → 동의어/영↔한 키워드 확장
- 7일 DB 캐시로 반복 쿼리 비용 0 수렴
- 실패 시 원본 쿼리 그대로 (safe fallback)

### 4.2 구현 지점
```
src-tauri/src/commands/search/
├── mod.rs
└── query_expand.rs        — secall 참고
DB migration v44           — query_cache (key PK, expanded, cached_at)
```

`search_messages` 에 opt-in wrapping:
```rust
let final_query = if feature_flag() { expand_query(&query, &db)? } else { query };
// 기존 FTS MATCH 로직
```

### 4.3 UX 고려
- Claude CLI 호출은 1~2초 걸림 → 첫 쿼리만 지연, 이후 캐시
- 최초 구현: **sync expansion** (첫 검색 2~3초 지연 수용) → 반응 보고 async 로 승격
- Feature flag: `TUNAFLOW_QUERY_EXPANSION` (default OFF — opt-in). 충분히 돌려본 뒤 ON 승격.

### 4.4 테스트
- 캐시 hit/miss
- `claude` 바이너리 없을 때 safe fallback (원본 쿼리 반환)
- Disabled 상태 — 원본 그대로 통과
- 캐시 만료 (7일 초과) → 재계산

---

## 5. Phase B — Hybrid RRF (플랜 검색 문제 해결의 핵심)

### 5.1 범위
단일 엔트리포인트 `/api/v1/search` + Tauri command `search_unified`:
1. 확장된 쿼리로 **messages_fts** (대화) 검색
2. 같은 쿼리로 **document_vector** (`conversation_chunks WHERE source_type='document'`) bge-m3 검색
3. 두 결과를 RRF (k=60) 로 합성 + 0~1 정규화
4. Session 다양성 후처리 (`max_per=2`)

### 5.2 구현 지점
```
src-tauri/src/commands/search/
├── hybrid.rs              — RRF (secall §1~68 참고)
├── diversify.rs           — session diversity
└── unified.rs             — 단일 엔트리
```

### 5.3 결과 스키마
```rust
pub struct UnifiedResult {
    pub kind: &'static str,   // "conversation" | "document" | "code" (future)
    pub id: String,            // message_id or file_path
    pub snippet: String,
    pub score: f64,            // 정규화 0~1
    pub fts_score: Option<f64>,
    pub vector_score: Option<f64>,
    pub source_label: String,  // "Chat: {conv_label}" or "Doc: {path}"
    pub timestamp: i64,
}
```

### 5.4 Frontend 변경
- 헤더 검색창: `search_messages` 대신 `search_unified` 호출
- 결과 표시에 `kind` 배지 (💬 대화 / 📄 문서) + source label 링크
- 클릭 시:
  - 대화: 해당 conversation + 메시지로 스크롤
  - 문서: 파일 뷰어 or 외부 editor

### 5.5 테스트
- RRF 단위 테스트 (bm25-only / vector-only / both 3 경우 × 결과 순서 정확성)
- 정규화 후 최대값 1.0
- Session 다양성 (같은 session 3개 이상 → 2개로 cap)
- Feature flag OFF 시 기존 동작

---

## 6. Phase C — Kiwi/Lindera Pre-Tokenize

### 6.1 중요 발견
secall 은 **FTS5 external tokenizer (C extension)** 가 아니라 **애플리케이션 레이어에서 pre-tokenize** → FTS5 는 `unicode61` 로 단순 공백 split. 즉:
```rust
let tokenized = tokenizer.tokenize_for_fts(&content);
// "아키텍처를 설계한다" → "아키텍처 설계"
INSERT INTO messages_fts(content) VALUES (tokenized);
```
검색 쿼리도 같은 방식으로 tokenize 한 뒤 FTS 에 넘김. rusqlite 의 복잡한 fts5_api 등록 불필요.

### 6.2 범위
- `src-tauri/src/search/tokenizer/` 모듈:
  - `Tokenizer trait`
  - `KiwiTokenizer` (cfg gate: not(windows), not(all(linux, aarch64)))
  - `LinderaKoTokenizer` (전 플랫폼)
  - `SimpleTokenizer` fallback
  - `create_tokenizer(backend: &str) -> Box<dyn Tokenizer>` with 폴백 체인 (kiwi → lindera → whitespace)
- 같은 태그 필터 (NNG/NNP/NNB/VV/VA/SL) + 1글자 제외 — secall 과 일치

### 6.3 FTS5 재인덱싱
- 기존 `messages_fts` 는 unicode61 whitespace 토큰화 — 형태소 기반으로 교체 시 재구축 필수
- 새 Tauri command: `rebuild_messages_fts` (프로그레스 바 + 취소 가능)
- 대규모 corpus 에선 증분 가능 옵션 검토 (hash 체크)

### 6.4 의존성
```toml
# Cargo.toml
[target.'cfg(not(any(target_os = "windows", all(target_os = "linux", target_arch = "aarch64"))))'.dependencies]
kiwi-rs = "0.x"

[dependencies]
lindera = { version = "0.x", features = ["ko-dic-embed"] }
```
- Kiwi 최초 사용 시 ~50MB 모델 다운로드 → 앱 첫 실행 시 백그라운드 prefetch + 진행률 표시

### 6.5 Settings UI
`Settings > Runtime > Search`:
- Tokenizer backend: `Auto (Kiwi → Lindera)` / `Kiwi only` / `Lindera only` / `Whitespace only`
- Default: Auto
- "Rebuild FTS index" 버튼 + 진행률

### 6.6 테스트
- 한국어 형태소 분해 정확도 (NNG/NNP 추출)
- 영문 + 한글 혼용 쿼리
- SimpleTokenizer fallback (모든 플랫폼)
- Kiwi 모델 다운로드 실패 시 Lindera fallback

---

## 7. Phase 간 의존성

```
Phase A (query expansion)       ──┐
                                  ├──> 실사용 관찰 1~2주
Phase B (hybrid RRF)            ──┘

Phase C (tokenizer) ← independent. 언제든 착수 가능. 단 FTS 재인덱싱 비용으로
                                    보통 B 이후 추천.
```

- A, B 는 독립 PR로 순차 배포 가능
- C 는 독립이나 FTS 재구축 이슈로 후순위가 안전
- A 혹은 B 먼저 머지 후 dogfood → C 도입 여부 판단

---

## 8. 측정 지표 (Phase 6 regression eval 에 연동 가능)

| 지표 | Baseline | 목표 |
|---|---|---|
| "플랜" 쿼리 검색 top-10 내 plan 문서 포함률 | 0% | 80%+ |
| 한/영 혼용 쿼리 recall (내부 golden query 20개) | 측정 예정 | +50% |
| 검색 latency p50 (캐시 hit) | 측정 예정 | < 300ms |
| 검색 latency p50 (캐시 miss, Phase A 포함) | — | < 3000ms |
| Query cache hit rate | 0% | 50%+ (동일 쿼리 반복 시) |

---

## 9. 위험 / 완화

| 위험 | 대응 |
|---|---|
| `claude-p haiku` subprocess 지연 (UX 체감) | 캐시 + 첫 쿼리는 fallback, expansion 은 async (Phase A-part2) |
| Kiwi 최초 모델 다운로드 UX 차단 | 앱 첫 실행 시 background prefetch + 진행률 표시 |
| FTS 재인덱싱 중 검색 블록 | "재구축 중" 상태 + whitespace fallback 유지 |
| RRF 점수 분포 편향 | 정규화 후 반올림 테스트 + 세션 다양성 cap |
| Hybrid 결과에 중복 (message + chunk 가 같은 내용) | ID 기반 dedup |
| 크로스플랫폼 (Windows 배포 시) Kiwi 미지원 | Lindera fallback 자동 동작 + 테스트 매트릭스 |

---

## 10. Subtask 구조

**subtask-01** — Phase A: Query Expansion
- DB migration v44 (query_cache)
- `src/commands/search/query_expand.rs` + `mod.rs`
- `search_messages` 통합 (opt-in)
- Unit tests + feature flag 테스트

**subtask-02** — Phase B: Hybrid RRF + Unified Search
- `src/commands/search/hybrid.rs` + `unified.rs`
- `search_unified` Tauri command
- Frontend 헤더 검색창 전환
- Unit tests (RRF, diversity)

**subtask-03** — Phase C: Kiwi/Lindera Pre-Tokenize
- `src/search/tokenizer/` 모듈
- Cargo 의존성 + cfg gate
- `rebuild_messages_fts` command + UI
- Settings 페이지 tokenizer backend 선택

---

## 11. 후속 과제 (Plan 범위 밖)

- Graph filter (topic/file/issue) — 별도 plan
- rawq 결과 Hybrid 에 merge (code 검색 통합) — 별도 plan
- Cross-session Session 다양성 (최근 N일 cap 등) — UX polish

---

## 12. 관련 참조

- seCall `crates/secall-core/src/search/`
  - `hybrid.rs` — RRF 구현
  - `query_expand.rs` — Claude Haiku 쿼리 확장
  - `tokenizer.rs` — Kiwi + Lindera 2-tier + fallback
  - `store/schema.rs` — FTS5 `tokenize='unicode61'` + 애플리케이션 pre-tokenize
- tunaFlow 현행 검색:
  - `src-tauri/src/commands/messages.rs:381` — `search_messages` (FTS5)
  - `src-tauri/src/commands/document_index.rs:802` — `search_project_docs` (vector)
  - `src-tauri/src/commands/vector_search/query.rs` — bge-m3 쿼리 경로
