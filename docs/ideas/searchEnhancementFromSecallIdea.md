# 검색 고도화 — secall 에서 가져올 것들 (hybrid / 한국어 tokenizer / query expansion)

> Status: idea
> Created: 2026-04-22
> Trigger: 사용자 관찰 "검색에서 '플랜' 검색하면 plan 이 안 나옴" — secall 은 같은 쿼리 정확 매칭. secall 이 tunaFlow 보다 검색이 강력한 이유 분석.
> 참고: `seCall/crates/secall-core/src/search/` (hybrid.rs, bm25.rs, vector.rs, tokenizer.rs, query_expand.rs)
> 관련: `src-tauri/src/agents/embedder.rs`, `src-tauri/src/commands/vector_search/`, `src-tauri/src/commands/document_index.rs`, SQLite FTS5

---

## 1. 문제

### 1.1 관찰된 증상
- tunaFlow 에서 "플랜" 으로 검색 → 문서(`docs/plans/*.md`) 매칭 실패.
- secall 에서 같은 쿼리 → 정확히 매칭.

### 1.2 설계 갭 (secall vs tunaFlow)
| 기능 | secall | tunaFlow | 영향 |
|---|---|---|---|
| Hybrid 검색 (BM25 + Vector RRF) | ✅ 통합 | ❌ FTS5 와 bge-m3 vector 분리 | 한쪽 누락된 결과 반영 X |
| 한국어 형태소 분석 | ✅ **Kiwi (기본) + Lindera (fallback)** | ❌ SQLite FTS5 기본 tokenizer (공백 기반) | "플랜" 같은 한국어 명사/용언이 제대로 토큰화 안 됨 |
| Query expansion | ✅ `claude-p haiku` 로 동의어/영↔한 확장 + 7일 캐시 | ❌ 없음 | "플랜" → 그대로 FTS 검색 → 영문 `plan` 문서 미매칭 |
| Graph filter (topic/file/issue) | ✅ session allowlist 사전 필터 | ❌ | 검색 범위 축소 수단 없음 |
| Session 다양성 | ✅ `diversify_by_session(max_per=2)` | ❌ | 한 대화가 결과 독식 |

---

## 2. Kiwi vs Lindera — 왜 2개 다?

### 2.1 Kiwi (`kiwi-rs` crate, Bab2Min/Kiwi 래퍼)
**장점**:
- 한국어 NLP 커뮤니티 표준. 미지어·신조어·도메인 용어 처리에 Lindera 대비 정확.
- 속도 빠름 (C++ 백엔드).
- 최신 언어 모델 채택.

**단점**:
- **Windows / aarch64 Linux 미지원** (C 포팅 이슈).
- 최초 사용 시 **모델 ~50MB 다운로드** (`~/.cache/kiwi/`) — UX 함정.
- `Sync` 불가 → `Mutex` 로 직렬화 필요 (비동기 성능 저하 약간).

### 2.2 Lindera (`lindera` crate + `ko-dic`)
**장점**:
- **임베디드 사전** (바이너리에 포함) → 런타임 다운로드 없음.
- **전 플랫폼 지원** (Windows/aarch64 Linux 포함).
- `Send + Sync` — thread-safe 기본.

**단점**:
- mecab-ko 계열. 최신 도메인 용어/신조어 커버리지가 Kiwi 대비 약함.
- 사전 크기 제약 (ko-dic 공식만 사용).

### 2.3 secall 의 선택: **런타임 선택 + 폴백 체인**
```rust
create_tokenizer("kiwi") → Kiwi 시도
  ├─ 성공: Kiwi 사용 (macOS, x64 Linux)
  └─ 실패: Lindera fallback (Windows, aarch64 Linux, 모델 다운 실패)
create_tokenizer(기타)  → Lindera 기본
```
Fallback 안에 또 하나: `tokenize()` 가 빈 결과 → `tokenize_fallback()` (whitespace + 소문자화).

같은 태그 필터로 일관성 유지:
```
keep: NNG (일반명사), NNP (고유명사), NNB (의존명사),
      VV (동사), VA (형용사), SL (외국어)
discard: 조사·어미·부사·특수문자
length: 1글자 이하 제외
```

### 2.4 tunaFlow 에 적용할 때
- **macOS 베타**: Kiwi 기본
- **Linux/Windows 확장 시점**: Lindera fallback 동작 보장 필요
- **모델 다운로드 UX**: 최초 사용 시 스피너 + 진행률 표시. 백그라운드 prefetch 도 고려.

---

## 3. 우선순위 (ROI 순)

### P0 — Query expansion (가장 쉽고 즉효)
- secall 의 `query_expand.rs` 그대로 이식 가능. `claude -p haiku` subprocess.
- 쿼리 확장 결과 **7일 DB 캐시** → 동일 쿼리 비용 0.
- 캐시 미스 시에도 `claude` 바이너리 없으면 원본 그대로 통과 (safe fallback).
- 예상 효과: "플랜" → `plan 계획 roadmap plan-proposal...` 확장 → FTS 매칭 즉시 개선.
- 구현: 단일 함수 + 캐시 테이블(migration v44+) + search 엔트리포인트에 wrapping.

### P1 — Hybrid RRF 검색
- 현재 `conversation_search (FTS5)` + `vector_search (bge-m3)` 두 경로 이미 존재.
- `reciprocal_rank_fusion` 함수 추가 (secall hybrid.rs §1~68 참고): 두 결과 rank 기반 점수 합성 + 0~1 정규화.
- UI 에서 **단일 검색창이 두 경로 통합 호출** + 결과 합성.
- 구현: 새 모듈 `src-tauri/src/commands/hybrid_search.rs` + 프론트엔드 SearchBox 수정.

### P2 — 한국어 tokenizer (Kiwi + Lindera)
- 가장 체감 크지만 **FTS 인덱스 재구축** 필요 (현재 인덱스 전부 reindex).
- SQLite FTS5 `external tokenizer` 방식으로 연결 (rusqlite 확장).
- 크로스플랫폼 조건부 컴파일 (`cfg(not(windows), not(aarch64))`).
- 구현: tokenizer 모듈 + FTS5 재인덱싱 스크립트 + 설정 UI (kiwi/lindera/auto).

### P3 — Session 다양성
- `diversify_by_session(max_per=2)` — 결과 후처리 필터.
- 구현 단순: 이미 정렬된 결과에 counter 로 cap.

### P4 — Graph filter (장기)
- Topic/File/Issue 노드 + edges 데이터 모델 필요. 현재 tunaFlow 에는 없음.
- `graph:` tool-request 와 code-review-graph 가 있지만 session-level 이 아닌 코드-level.
- 중장기.

---

## 4. 구현 스케치

### 4.1 Phase A — Query expansion (1~2일)
```
src-tauri/src/commands/search/
├── mod.rs
├── query_expand.rs       — secall 의 expand_query 이식
└── (기존 vector_search, conversation_search 유지)
```
DB migration v44: `query_cache (query TEXT PK, expanded TEXT, expires_at INTEGER)`.

### 4.2 Phase B — Hybrid RRF (2~3일)
```
src-tauri/src/commands/search/
├── hybrid.rs             — RRF (k=60) + 정규화 + 다양성
└── ranking.rs            — 공통 SearchResult 구조체
```
`/api/v1/search` 단일 엔트리포인트 (FTS+Vector 내부 합성).

### 4.3 Phase C — Korean tokenizer (3~5일)
```
src-tauri/src/search/tokenizer/
├── mod.rs                — Tokenizer trait + create_tokenizer
├── kiwi.rs               — cfg(not(windows), not(aarch64))
├── lindera.rs            — 전 플랫폼
└── fallback.rs           — whitespace
```
FTS5 외부 tokenizer 등록 + 기존 인덱스 재구축 (`rebuild_fts5.rs` command).

Cargo.toml:
```toml
[target.'cfg(not(any(target_os = "windows", all(target_os = "linux", target_arch = "aarch64"))))'.dependencies]
kiwi-rs = "0.x"
lindera = { version = "0.x", features = ["ko-dic-embed"] }
```

---

## 5. 위험 / 완화

| 위험 | 대응 |
|---|---|
| Kiwi 최초 모델 다운로드로 UX 블록 | 앱 설치 후 첫 실행 시 백그라운드 prefetch + 진행 표시 |
| FTS 재인덱싱 비용 (현재 대규모 conversation corpus) | 증분 재인덱싱 + "재구축 중" 상태 표시 + 기존 검색 fallback |
| query_expand `claude` 호출 실패 / 없음 | 원본 쿼리로 safe fallback (secall 방식 그대로) |
| 크로스플랫폼 (Windows 배포 시) Kiwi 미지원 | Lindera fallback 자동 동작 확인 + 테스트 매트릭스 |
| 모델 버전 drift (ko-dic / Kiwi) | 의존성 pin + CI 에서 토크나이즈 회귀 테스트 |
| bge-m3 embedding 과 tokenizer 의 불일치 (BM25 는 형태소, Vector 는 subword) | 의도된 차이 — RRF 로 두 signal 모두 활용. 문제 아님 |

---

## 6. Scope 경계 — 하지 않을 것

- **rawq 는 이미 별도 검색 엔진** (코드 검색 전용). 본 idea 는 **대화/문서 검색** 에만 해당.
- **영문 stemmer** 추가는 후순위 — 한국어 문제가 우선 체감.
- Graph filter P4 는 별도 plan 으로 승격 시점 판단.

---

## 7. 측정 지표

Phase 별 before/after:

| 지표 | 측정 |
|---|---|
| "플랜" 쿼리 FTS 히트 수 | 현재 0 → 목표 10+ (docs/plans/*.md 기준) |
| Vector + FTS 통합 쿼리 latency (p50) | ≤ 300ms |
| Hybrid 결과 상위 10 의 relevance (사용자 평가) | >= Haiku judge 7/10 |
| Query cache 히트율 | 목표 50%+ (재현 가능한 쿼리 많음) |

Phase 6 regression eval 에 retrieval quality 항목 추가하면 자동 측정 가능.

---

## 8. 관련 문서

- `docs/ideas/bgeM3QuantizationAndAcceleratorIdea.md` — 임베딩 모델 최적화 (vector 검색 속도)
- `docs/plans/harnessVerificationGapPlan.md` — 검색 직접 관련 없으나 harness 품질 지표에 연동 가능
- `docs/ideas/projectDocumentRagIdea.md` — document RAG 전체 설계
- `seCall/crates/secall-core/src/search/` — 참고 구현
- `seCall/CLAUDE.md` — secall 아키텍처 문서
