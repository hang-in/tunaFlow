# 임베딩 지연 최적화 — embed_text() 3회 순차 호출 문제

> Status: resolved (세션 9에서 해결)
> Created: 2026-04-04
> Updated: 2026-04-04 (코더 Opus 검증 — 캐싱 불필요 확인)
> 발견: Gemini로 "아무 동물이나 3마리 얘기해줘" → 61토큰 응답에 60.4초 소요
> 해결: `is_daemon_ready()` 가드 + 짧은 쿼리 스킵으로 크리티컬 패스 지연 제거

---

## ⚠️ 분석 정정

이 문서의 원래 분석("3회 순차 embed 호출")은 **부정확**했습니다.

**실제 호출 구조** (코더 Opus 검증):
- 크리티컬 패스: `context_loading.rs:259`에서 **1회만** 호출
- 백그라운드: `session_discovery.rs:109`에서 1회 (agent 완료 1.5초 후, 사용자 체감 없음)

**이미 적용된 해결책**:
- `is_daemon_ready()` 가드 — daemon 미실행 시 embed 자체를 스킵 (콜드 스타트 60초 → 0초)
- `prompt.chars().count() >= 15` 가드 — 짧은 질문에서 embed 스킵

**캐싱은 불필요** — 크리티컬 패스에서 1회뿐이므로 캐싱할 대상이 없음.

---

## 1. 문제

### 증상

간단한 한국어 질문(10자)에 대해 61토큰 응답이 60.4초 걸림. Gemini API 자체는 0.1-0.5초면 응답하는 수준.

### 원인

`load_context_data()`에서 같은 프롬프트를 **3번 순차적으로 임베딩**:

```
호출 1: context_loading.rs:259 — Vector search용
        rawq::embed_text(prompt, true)               ~10-20초

호출 2: context_loading.rs:268 — Vector boost (RRF merge)용
        rawq::embed_text(prompt, true)               ~10-20초

호출 3: session_discovery.rs:109 — Session discovery용
        rawq::embed_text(combined, true)              ~10-20초

합계: ~30-60초 (rawq daemon 상태에 따라)
```

### 왜 이렇게 느린가

- `embed_text()`는 rawq 바이너리를 subprocess로 실행 (`rawq.rs:569`)
- 타임아웃: **10초** 하드코딩
- rawq daemon이 꺼져있으면 매 호출마다 ONNX 모델 로딩 (5-7초)
- 3번 순차 실행 → 최악 30초, 콜드 스타트 포함 시 60초

### 시간 분해 (실측 기반 추정)

```
[0초]     prepare_engine_run() 시작
[0-2초]   DB 쿼리 12개 (load_context_data Phase A) — 빠름
[2-12초]  embed_text #1: vector search — daemon 시작 + 임베딩
[12-22초] embed_text #2: vector boost — 같은 프롬프트 재임베딩
[22-32초] embed_text #3: session discovery — 또 재임베딩
[32-55초] assemble_prompt + Gemini CLI 시작
[55-60초] Gemini API 응답 수신
[60.4초]  완료
```

---

## 2. 해결 방안

### 2.1 [즉시] 임베딩 결과 캐싱

같은 `load_context_data()` 호출 내에서 동일 프롬프트의 임베딩을 재사용:

```rust
// context_loading.rs — load_context_data() 내부

// 한 번만 임베딩
let query_embedding: Option<Vec<f32>> = crate::agents::rawq::embed_text(prompt, true).ok();

// 호출 1: vector search — 캐시된 임베딩 사용
if let Some(ref emb) = query_embedding {
    let vec_results = vector_search::search_similar(conn, emb, pk, conversation_id, 5);
    // ... RRF merge
}

// 호출 2: session discovery — 같은 임베딩 전달
let effective_cross_ids = if cross_session_ids.is_empty() {
    session_discovery::discover_with_embedding(conn, conversation_id, pk, query_embedding.as_deref())
} else {
    cross_session_ids.to_vec()
};
```

**효과**: 3회 → 1회. **60초 → ~20초**.
**변경**: `context_loading.rs` (~20줄), `session_discovery.rs` 시그니처 변경 (~10줄).

### 2.2 [다음] rawq daemon 헬스체크

embed 호출 전에 daemon이 ready인지 확인. 안 되어있으면 미리 시작하고 대기:

```rust
// rawq.rs

pub fn ensure_daemon_ready() -> bool {
    // 1. daemon status 확인 (rawq daemon status)
    // 2. 안 돌고 있으면 시작 + 최대 5초 대기
    // 3. ready면 true, 아니면 false (embed 스킵)
}

// context_loading.rs — embed 전에 체크
if !rawq::ensure_daemon_ready() {
    // daemon 없으면 vector search 전체 스킵
    // FTS5만으로 동작 (graceful degradation)
}
```

**효과**: 콜드 스타트 제거. **20초 → 2-3초**.
**변경**: `rawq.rs` (~30줄), `context_loading.rs` (~5줄).

### 2.3 [다음] 단순 쿼리 벡터 검색 스킵

10자 미만 또는 명백히 단순한 질문에서 벡터 검색을 건너뛰기:

```rust
// context_loading.rs

let needs_vector = prompt.chars().count() >= 20
    && !is_simple_greeting(prompt);  // "안녕", "고마워" 등 제외

if needs_vector {
    // embed + vector search
} else {
    // FTS5만 사용
}
```

**효과**: 단순 질문에서 임베딩 자체를 호출하지 않음. **즉시 응답**.
**변경**: `context_loading.rs` (~10줄).

---

## 3. 우선순위

| 수정 | 효과 | 난이도 | 적용 순서 |
|------|------|--------|----------|
| **임베딩 캐싱** | 60초 → 20초 | 낮음 | **1번** |
| **daemon 헬스체크** | 20초 → 2-3초 | 중간 | **2번** |
| **단순 쿼리 스킵** | 단순 질문 즉시 응답 | 낮음 | **3번** |

1번만 적용해도 **3배 개선**. 1+2번이면 **20-30배 개선**.

---

## 4. 현재 코드 위치

| 파일 | 줄 | 내용 |
|------|---|------|
| `context_loading.rs:259` | embed_text 호출 #1 (vector search) |
| `context_loading.rs:268` | embed_text 호출 #2 (vector boost) |
| `session_discovery.rs:109` | embed_text 호출 #3 (session discovery) |
| `rawq.rs:569-576` | embed_text 타임아웃 10초 하드코딩 |
| `rawq.rs:175-205` | ensure_daemon() — async, 완료 대기 없음 |

---

## 5. 검증 방법

수정 전후 비교:

```bash
# 수정 전: Gemini로 간단한 질문
# trace_log에서 duration_ms 확인
# 예상: 50,000-60,000ms

# 수정 후 (캐싱만):
# 예상: 15,000-25,000ms

# 수정 후 (캐싱 + 헬스체크):
# 예상: 2,000-5,000ms

# 수정 후 (캐싱 + 헬스체크 + 단순 쿼리 스킵):
# 예상: 500-2,000ms (Gemini API 응답 시간만)
```

---

## 참고

- rawq 임베딩: `src-tauri/src/agents/rawq.rs` (embed_text, ensure_daemon)
- Context 로딩: `src-tauri/src/commands/agents_helpers/send_common/context_loading.rs`
- Session discovery: `src-tauri/src/commands/session_discovery.rs`
- Vector search: `src-tauri/src/commands/vector_search.rs`
- 관련: `docs/ideas/knowledgeLayerArchitectureIdea.md` (KnowledgeLayer 통합 시 캐싱 자연스럽게 해결)
