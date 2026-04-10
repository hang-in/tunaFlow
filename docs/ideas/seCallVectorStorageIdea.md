# seCall 벡터 스토리지 평가 — sqlite-vec vs ChromaDB vs plain BLOB

> Status: idea
> Created: 2026-04-08
> 대상: seCall (Rust + Tauri 데스크톱 앱)
> 레퍼런스: `_research/_util/mempalace/` (ChromaDB, 96.6% LongMemEval), `_research/_util/qmd/` (sqlite-vec, TypeScript)

---

## 1. 평가 배경

seCall의 벡터 스토리지 선택지:
- **sqlite-vec**: SQLite 확장. ANN 인덱스. 단일 파일 유지.
- **ChromaDB**: Python 벡터 DB. mempalace에서 96.6% 달성.
- **plain BLOB**: tunaFlow 방식. 추가 의존 0. brute-force cosine.

---

## 2. sqlite-vec 실사용 문제 (qmd에서 발견)

qmd 프로젝트(TypeScript + sqlite-vec)에서 **치명적 제한** 발견:

```
sqlite-vec의 vec0 가상 테이블을 일반 테이블과 JOIN하면 무한 행(hang) 발생
```

워크어라운드 (qmd PR #23):
```sql
-- ❌ 이렇게 하면 hang
SELECT d.*, v.distance FROM vectors_vec v
JOIN documents d ON d.hash_seq = v.hash_seq
WHERE v.embedding MATCH ?

-- ✅ 2-step으로 분리
-- Step 1: 벡터 검색 (JOIN 없이)
SELECT hash_seq, distance FROM vectors_vec WHERE embedding MATCH ?

-- Step 2: 결과 ID로 문서 조회
SELECT * FROM documents WHERE hash_seq IN (?, ?, ?)
```

**이건 sqlite-vec v0.1.9의 근본적 제한**. 단일 쿼리로 "벡터 검색 + 메타데이터 조회" 불가.

추가 문제:
- macOS 시스템 SQLite는 `SQLITE_OMIT_LOAD_EXTENSION` → brew SQLite 필요
- `OR REPLACE` 충돌 해결 무시됨
- 플랫폼별 확장 빌드 필요

---

## 3. ChromaDB 실사용 평가 (mempalace)

mempalace가 LongMemEval 96.6% 달성한 이유:

```
✅ 원문 그대로 저장 (요약/압축 없이 verbatim)
✅ 메타데이터 필터링 (wing/room)으로 검색 범위 축소 → +34% 개선
✅ 교환 쌍(user+assistant)을 개별 청크로 세분화
```

**96.6%는 ChromaDB 덕이 아니라 저장 전략 덕**. 같은 전략을 plain BLOB + FTS5에 적용해도 유사한 결과 가능.

ChromaDB 자체의 문제:
- **Python 런타임 의존** → seCall(Rust)에서 REST API 호출 필요 → 별도 프로세스 + HTTP 오버헤드
- **버전 호환성** → mempalace 저자도 "ChromaDB 버전 핀 없음" 우려 (Issue #100)
- **단일 파일 아님** → 별도 디렉토리 구조
- **로컬 퍼스트 위반** → 별도 DB 서버/프로세스 추가

---

## 4. 트레이드오프 비교

| | sqlite-vec | ChromaDB | plain BLOB (tunaFlow 방식) |
|---|---|---|---|
| **Rust 호환** | rusqlite + 확장 로드 | REST API (reqwest) | rusqlite만 (의존 0) |
| **JOIN 가능** | ❌ hang 문제 | N/A (별도 DB) | ✅ 일반 SQL |
| **벡터 인덱스** | ANN (vec0) | HNSW | brute-force |
| **설치 복잡도** | 확장 빌드 + 로드 | Python + 서버 기동 | 없음 |
| **성능 (1K 청크)** | ~1ms | ~5ms (HTTP) | ~5ms |
| **성능 (10K 청크)** | ~1ms | ~5ms | ~50ms (병목 시작) |
| **단일 파일** | ✅ | ❌ | ✅ |
| **성숙도** | v0.1.9 (초기) | v0.5+ (안정) | 검증됨 (tunaFlow 실사용) |
| **검색 품질** | ANN (근사, recall 95-99%) | HNSW (근사) | brute-force (정확, recall 100%) |

---

## 5. 규모별 권장

### 1000 청크 이하 (대부분의 개인 프로젝트)

```
→ plain BLOB brute-force (tunaFlow 방식)
  - 의존성 0, 설치 0, brute-force <5ms
  - sqlite-vec도 ChromaDB도 과도
  - tunaFlow에서 85 청크로 실증 완료
```

### 1000-10000 청크

```
→ sqlite-vec 2-step 패턴 또는 plain BLOB 유지
  - 2-step 워크어라운드 감수하면 ANN의 속도 이점
  - plain BLOB도 50ms 수준이라 아직 실사용 가능
  - ChromaDB는 Python 의존성 때문에 Rust 앱에 부적합
```

### 10000+ 청크

```
→ sqlite-vec (ANN 인덱스 필요한 시점)
  또는
→ qdrant embedded (Rust 네이티브, 의존성 하나)
  - qdrant를 embedded 모드로 사용하면 별도 서버 불필요
  - Rust 네이티브라 seCall과 자연스러운 통합
```

---

## 6. 결론

**ChromaDB로 전환하지 않습니다.** 이유:

1. seCall이 Rust인데 Python 의존성 추가는 아키텍처 역행
2. mempalace의 96.6%는 ChromaDB가 아니라 원문 저장 전략의 성과
3. sqlite-vec의 JOIN 문제가 있지만 2-step으로 우회 가능
4. 로컬 퍼스트 철학에 별도 DB 프로세스 부적합

**권장 로드맵**:

```
Phase 1 (지금): plain BLOB brute-force
  → tunaFlow와 동일 패턴
  → 의존성 0, 즉시 동작
  → recall 100% (정확도 최고)

Phase 2 (5000+ 청크): sqlite-vec 2-step 패턴
  → ANN 인덱스로 속도 개선
  → JOIN 문제는 2-step으로 우회
  → 단일 SQLite 파일 유지

Phase 3 (대안, 10000+): qdrant embedded 검토
  → Rust 네이티브
  → HNSW 인덱스
  → embedded 모드로 별도 서버 불필요
```

**mempalace에서 가져올 것**: ChromaDB가 아니라 **원문 저장 전략** + 메타데이터 필터링 패턴. 이건 어떤 벡터 DB에서든 적용 가능.

---

## 참고

- mempalace: `_research/_util/mempalace/` (Python, ChromaDB, 96.6% LongMemEval)
- qmd: `_research/_util/qmd/` (TypeScript, sqlite-vec, BM25+벡터 하이브리드)
  - sqlite-vec JOIN hang 문제: qmd PR #23
- tunaFlow plain BLOB: `src-tauri/src/commands/vector_search.rs` (309줄, 85 청크 실사용)
- sqlite-vec: https://github.com/asg017/sqlite-vec (v0.1.9)
- qdrant embedded: https://github.com/qdrant/qdrant (Rust 네이티브)
