# Knowledge Layer — rawq 고도화 + 향후 확장을 위한 통합 구조

> Status: idea
> Created: 2026-04-02
> 관련: `workflowGraphEnhancementIdea.md` (워크플로우에서 graph 활용), `rawqGraphEvolutionStrategyIdea.md` (rawq+graph 통합 전략)

---

## 1. 문제: 현재 ContextPack의 지식 소스가 하드코딩되어 있다

```rust
// 현재 context_pack.rs — 각 소스가 개별 함수로 하드코딩
let rawq_section = build_rawq_section(prompt, project_path, ...)?;      // 코드 검색
let retrieval_chunks = retrieve_relevant_chunks(...)?;                   // FTS5 대화 검색
let cross_session = build_cross_session_section(...)?;                   // 수동 세션 연결
let compressed_memory = load_compressed_memory(...)?;                    // 대화 요약
```

문제:
- 새 지식 소스를 추가하려면 `context_pack.rs`를 직접 수정해야 함
- 소스 간 중복 제거가 소스별로 따로 (`rawq`는 confidence, `retrieval`은 Jaccard)
- 예산 배분이 소스별 고정 (`rawq` 4k, `retrieval` 4k, `cross-session` 4k)
- code-review-graph를 추가하면 또 하나의 하드코딩 섹션이 생김

---

## 2. 목표 구조: Knowledge Layer

```
ContextPack 조립 (send_common.rs)
  │
  ├── Identity / Plan / Skills / Platform   ← 고정 섹션 (변경 없음)
  │
  └── Knowledge Layer                       ← 새로운 추상화
       │
       │  query(prompt, budget) → Vec<KnowledgeChunk>
       │
       ├── rawq        (코드 + 문서 검색)      ← Phase 1: 고도화
       ├── fts5        (대화 키워드 검색)       ← 기존 유지
       ├── memory      (압축 기억)             ← 기존 유지
       ├── cross-session (관련 세션)           ← Phase 1: 자동화
       ├── graph       (코드 구조 탐색)        ← Phase 2: code-review-graph
       └── (향후)      (Vector DB, embeddings)  ← Phase 3
```

### 핵심 인터페이스

```rust
/// 모든 지식 소스가 구현하는 trait
pub trait KnowledgeSource {
    /// 이 소스의 이름 (ContextPack 섹션 라벨)
    fn name(&self) -> &str;

    /// 이 소스가 현재 쿼리에 관련 있는지 빠르게 판단
    fn is_relevant(&self, prompt: &str, context: &QueryContext) -> bool;

    /// 검색 실행. budget_chars 내에서 결과 반환
    fn query(
        &self,
        prompt: &str,
        context: &QueryContext,
        budget_chars: usize,
    ) -> Result<Vec<KnowledgeChunk>, AppError>;

    /// 이 소스의 기본 우선순위 (높을수록 먼저 예산 할당)
    fn priority(&self) -> f32;
}

/// 통일된 결과 단위
pub struct KnowledgeChunk {
    pub source: String,           // "rawq", "fts5", "memory", "graph"
    pub content: String,          // 실제 텍스트
    pub confidence: f32,          // 0.0-1.0 정규화
    pub chunk_type: ChunkType,    // Code, Conversation, Document, Structure
    pub metadata: ChunkMetadata,  // 파일/라인/대화ID 등
}

pub enum ChunkType {
    Code,           // rawq 코드 결과
    Document,       // rawq 문서 결과 (docs/, *.md)
    Conversation,   // fts5, cross-session, memory
    Structure,      // code-review-graph (caller, dependency, test)
}

pub struct ChunkMetadata {
    pub file_path: Option<String>,
    pub line_range: Option<(usize, usize)>,
    pub conversation_id: Option<String>,
    pub scope: Option<String>,       // 함수명, 클래스명
    pub relation: Option<String>,    // "calls", "imported_by", "tested_by" (graph용)
    pub recency: Option<f64>,        // 시간 기반 감쇠값
}
```

### 퓨전 + 예산 배분

```rust
pub struct KnowledgeLayer {
    sources: Vec<Box<dyn KnowledgeSource>>,
}

impl KnowledgeLayer {
    pub fn query(
        &self,
        prompt: &str,
        context: &QueryContext,
        total_budget: usize,
    ) -> Vec<KnowledgeChunk> {

        // 1. 관련 소스 필터링
        let relevant: Vec<_> = self.sources.iter()
            .filter(|s| s.is_relevant(prompt, context))
            .collect();

        // 2. 우선순위 기반 예산 배분
        let budgets = allocate_budgets(&relevant, total_budget);

        // 3. 각 소스에서 결과 수집
        let mut all_chunks: Vec<KnowledgeChunk> = Vec::new();
        for (source, budget) in relevant.iter().zip(budgets) {
            let chunks = source.query(prompt, context, budget)?;
            all_chunks.extend(chunks);
        }

        // 4. 소스 간 중복 제거 (통합 Jaccard + semantic overlap)
        dedup_cross_source(&mut all_chunks);

        // 5. confidence 기반 최종 정렬 + budget trim
        all_chunks.sort_by(|a, b| b.confidence.partial_cmp(&a.confidence).unwrap());
        trim_to_budget(&mut all_chunks, total_budget);

        all_chunks
    }
}
```

---

## 3. Phase 1: rawq 고도화 (장기기억)

### 3.1 rawq를 KnowledgeSource로 감싸기

```rust
pub struct RawqSource {
    project_path: String,
}

impl KnowledgeSource for RawqSource {
    fn name(&self) -> &str { "rawq" }

    fn is_relevant(&self, prompt: &str, _ctx: &QueryContext) -> bool {
        // 현재: 코드 키워드 감지만
        // 변경: 항상 true (코드+문서 통합 검색)
        // 단, 인덱스 없으면 false
        rawq::is_indexed(&self.project_path).unwrap_or(false)
    }

    fn query(&self, prompt: &str, _ctx: &QueryContext, budget: usize) -> Result<Vec<KnowledgeChunk>> {
        let results = rawq::search_with_options(
            &self.project_path,
            prompt,
            SearchOptions {
                limit: 10,
                threshold: 0.3,
                rerank: true,              // ← 활성화
                token_budget: Some(budget), // ← 예산 전달
                text_weight: 0.7,          // ← 문서 가중치 상향
                // rrf_weight는 쿼리 분석에 따라 동적 결정
                rrf_weight: if is_conceptual_query(prompt) { 0.8 } else { 0.5 },
            },
        )?;

        Ok(results.into_iter().map(|r| KnowledgeChunk {
            source: "rawq".into(),
            content: r.snippet,
            confidence: r.confidence,
            chunk_type: if is_code_file(&r.path) { ChunkType::Code } else { ChunkType::Document },
            metadata: ChunkMetadata {
                file_path: Some(r.path),
                line_range: Some((r.start_line, r.end_line)),
                scope: r.scope,
                ..Default::default()
            },
        }).collect())
    }

    fn priority(&self) -> f32 { 0.8 }
}
```

### 3.2 rawq 미사용 기능 활성화

| 옵션 | 현재 | 변경 | 효과 |
|------|------|------|------|
| `--rerank` | 미사용 | 항상 활성화 | 키워드 오버랩 2-pass로 정밀도 개선 |
| `--token-budget` | 미사용 | KnowledgeLayer budget 전달 | rawq가 예산 내에서 자체 트렁케이션 |
| `--text-weight` | 미사용 | 0.7 (문서 가중치 상향) | docs/, plan 문서도 검색 대상 |
| `--rrf-weight` | 미사용 | 쿼리별 동적 (0.5-0.8) | 개념 쿼리 → semantic 비중 ↑ |
| `-s` | 미사용 | 개념 쿼리 감지 시 | "아키텍처 설계 방향" 같은 쿼리 |

### 3.3 인덱싱 범위 확장

현재 rawq는 `.gitignore` 기준으로 프로젝트 전체를 인덱싱하므로, `docs/` 아래 문서도 이미 인덱싱 대상이다.

변경 사항:
- `prompt_needs_rawq()` 키워드 게이트 **제거** → KnowledgeLayer가 is_relevant 판단
- `docs/plans/*.md`, `docs/ideas/*.md` 등 프로젝트 지식 문서도 검색 결과에 포함
- ChunkType으로 Code/Document 구분 → ContextPack에서 섹션 라벨 분리

### 3.4 FTS5를 KnowledgeSource로 감싸기

```rust
pub struct Fts5Source;

impl KnowledgeSource for Fts5Source {
    fn name(&self) -> &str { "conversation" }

    fn is_relevant(&self, _prompt: &str, _ctx: &QueryContext) -> bool {
        true  // 대화 검색은 항상 시도
    }

    fn query(&self, prompt: &str, ctx: &QueryContext, budget: usize) -> Result<Vec<KnowledgeChunk>> {
        // 기존 retrieve_relevant_chunks_with_overlap() 로직을 그대로 사용
        let chunks = retrieve_relevant_chunks(ctx.conn, ctx.conversation_id, prompt, budget)?;
        Ok(chunks.into_iter().map(|c| KnowledgeChunk {
            source: "conversation".into(),
            content: c.content,
            confidence: c.score,
            chunk_type: ChunkType::Conversation,
            metadata: ChunkMetadata {
                conversation_id: Some(c.conversation_id),
                recency: Some(c.recency_score),
                ..Default::default()
            },
        }).collect())
    }

    fn priority(&self) -> f32 { 1.0 }  // 대화 맥락이 가장 중요
}
```

### 3.5 Cross-Session 자동 발견

현재: 사용자가 수동으로 `crossSessionIds` 선택
변경: rawq + FTS5 결과에서 다른 대화 ID가 등장하면 자동 포함

```rust
pub struct CrossSessionSource;

impl KnowledgeSource for CrossSessionSource {
    fn is_relevant(&self, _prompt: &str, ctx: &QueryContext) -> bool {
        ctx.project_conversation_count > 1  // 대화가 2개 이상일 때만
    }

    fn query(&self, prompt: &str, ctx: &QueryContext, budget: usize) -> Result<Vec<KnowledgeChunk>> {
        // 1. FTS5로 다른 대화에서 관련 메시지 검색
        let cross_results = fts5_search_other_conversations(
            ctx.conn, ctx.conversation_id, ctx.project_key, prompt, 5
        )?;

        // 2. 사용자가 명시한 crossSessionIds도 포함
        let manual = load_manual_cross_sessions(ctx.conn, &ctx.cross_session_ids)?;

        // 3. 합산 + 중복 제거
        let mut all = cross_results;
        all.extend(manual);
        dedup_by_conversation(&mut all);

        Ok(all.into_iter().map(|r| KnowledgeChunk {
            source: "cross-session".into(),
            chunk_type: ChunkType::Conversation,
            ..
        }).collect())
    }

    fn priority(&self) -> f32 { 0.6 }
}
```

### 3.6 Compressed Memory를 KnowledgeSource로

```rust
pub struct MemorySource;

impl KnowledgeSource for MemorySource {
    fn name(&self) -> &str { "memory" }
    fn is_relevant(&self, _: &str, ctx: &QueryContext) -> bool {
        ctx.message_count >= 12  // 압축 기억이 존재할 조건
    }
    fn query(&self, _: &str, ctx: &QueryContext, budget: usize) -> Result<Vec<KnowledgeChunk>> {
        let memory = load_compressed_memory(ctx.conn, ctx.conversation_id)?;
        Ok(match memory {
            Some(m) => vec![KnowledgeChunk {
                source: "memory".into(),
                content: truncate(&m.summary, budget),
                confidence: 1.0,  // 항상 최고 우선
                chunk_type: ChunkType::Conversation,
                ..
            }],
            None => vec![],
        })
    }
    fn priority(&self) -> f32 { 1.2 }  // 가장 높은 우선순위
}
```

---

## 4. Phase 2: code-review-graph 추가 (워크플로우)

Phase 1의 KnowledgeLayer가 준비되면, code-review-graph는 **새 KnowledgeSource 하나 추가**로 끝난다.

```rust
pub struct GraphSource {
    project_path: String,
}

impl KnowledgeSource for GraphSource {
    fn name(&self) -> &str { "structure" }

    fn is_relevant(&self, prompt: &str, ctx: &QueryContext) -> bool {
        // 구조 탐색이 필요한 쿼리인지 판단
        // "이 함수를 호출하는 곳", "의존성", "영향 범위", "테스트" 등
        has_structure_signal(prompt)
            && ctx.plan_phase.is_some()  // 워크플로우 중일 때만
    }

    fn query(&self, prompt: &str, ctx: &QueryContext, budget: usize) -> Result<Vec<KnowledgeChunk>> {
        // 1. rawq 결과에서 파일/심볼 추출
        let symbols = extract_symbols_from_context(ctx);

        // 2. code-review-graph로 1-hop 확장
        let related = graph::expand_one_hop(&self.project_path, &symbols)?;
        // → callers, importers, test files

        Ok(related.into_iter().map(|r| KnowledgeChunk {
            source: "structure".into(),
            content: r.snippet,
            confidence: r.relevance,
            chunk_type: ChunkType::Structure,
            metadata: ChunkMetadata {
                file_path: Some(r.path),
                relation: Some(r.relation_type),  // "calls", "imported_by", "tested_by"
                ..
            },
        }).collect())
    }

    fn priority(&self) -> f32 { 0.5 }  // rawq/fts5보다 낮지만 존재
}
```

**추가 작업**: `KnowledgeLayer.sources`에 `GraphSource` push. 끝.

```rust
// Phase 2에서 추가되는 코드 (전체)
let mut layer = KnowledgeLayer::new();
layer.add(RawqSource::new(path));         // Phase 1
layer.add(Fts5Source);                     // Phase 1
layer.add(MemorySource);                   // Phase 1
layer.add(CrossSessionSource);             // Phase 1
layer.add(GraphSource::new(path));         // ← Phase 2: 이 한 줄
```

---

## 5. 왜 이 구조가 좋은가

### rawq와 code-review-graph의 관계가 명확해진다

```
rawq:  "이 쿼리와 관련된 코드/문서를 찾아라"     → 검색 (what)
graph: "이 코드와 연결된 구조를 펼쳐라"          → 탐색 (how it connects)
fts5:  "이 주제에 대해 과거에 뭘 논의했나"       → 기억 (what was said)
memory: "이 대화에서 지금까지 무슨 일이 있었나"   → 요약 (what happened)
```

역할이 겹치지 않는다. 각 소스가 다른 축의 지식을 제공.

### 소스 추가가 코드 1줄

```rust
layer.add(NewSource::new(...));  // 향후 Vector DB, Ollama embeddings 등
```

`context_pack.rs`를 매번 수정하는 대신, KnowledgeSource trait만 구현하면 된다.

### 예산 배분이 자동화된다

현재는 `rawq` 4k, `retrieval` 4k 같은 고정값. KnowledgeLayer는 priority 기반으로 동적 배분:

```
total_budget = 12,000 chars (context mode에 따라)

memory   (1.2) → 2,400  ← 대화 요약은 항상 충분히
fts5     (1.0) → 2,000
rawq     (0.8) → 1,600
cross    (0.6) → 1,200
(여유분 재배분 → 결과가 적은 소스에서 많은 소스로)
```

### 소스 간 중복 제거가 한 곳에서

현재 rawq dedup과 fts5 dedup이 따로인데, `dedup_cross_source()`에서 **모든 소스의 결과를 한번에 비교**.

---

## 6. 구현 계획

### Phase 1: rawq 고도화 + KnowledgeLayer 기초 (지금)

```
1-1. KnowledgeSource trait + KnowledgeChunk 정의
     → send_common.rs 또는 별도 knowledge.rs

1-2. 기존 4개 소스를 KnowledgeSource로 래핑
     → RawqSource, Fts5Source, MemorySource, CrossSessionSource
     → 기존 로직 그대로, 인터페이스만 통일

1-3. KnowledgeLayer 퓨전 + 예산 배분
     → context_pack.rs의 개별 섹션 빌드를 KnowledgeLayer.query()로 대체

1-4. rawq 미사용 옵션 활성화
     → --rerank, --token-budget, --text-weight, --rrf-weight
     → rawq.rs의 search() 함수 확장

1-5. Cross-session 자동 발견
     → FTS5로 다른 대화 검색 (수동 선택과 병행)

검증: 기존 테스트 통과 + rawq 검색 품질 비교
```

### Phase 2: code-review-graph 추가 (나중)

```
2-1. code-review-graph 바이너리 sidecar 통합
     → rawq와 동일 패턴 (agents/graph.rs)

2-2. GraphSource 구현
     → KnowledgeSource trait 구현

2-3. KnowledgeLayer에 등록
     → 1줄 추가

검증: Developer/Reviewer 워크플로우에서 구조 정보 포함 확인
```

### Phase 3: 향후 확장 (선택)

```
3-1. Vector DB Source (Ollama embeddings 기반)
3-2. SDK Embeddings Source (유료, 고품질)
3-3. 임베딩 모델 교체 (한국어 지원)
```

---

## 7. 변경 범위 예측

### Phase 1 (rawq 고도화)

| 파일 | 변경 |
|------|------|
| `send_common.rs` | KnowledgeLayer 호출로 대체 (~50줄 변경) |
| `context_pack.rs` | 개별 빌드 함수 → KnowledgeSource 래핑 (~200줄 리팩토링) |
| `rawq.rs` | `search_with_options()` 확장 (~30줄 추가) |
| `context_queries.rs` | FTS5 cross-conversation 검색 추가 (~50줄) |
| 새 파일: `knowledge.rs` | trait + layer + dedup (~150줄) |

**총 예상**: ~300줄 신규 + ~200줄 리팩토링. 기존 API 변경 없음.

### Phase 2 (graph 추가)

| 파일 | 변경 |
|------|------|
| 새 파일: `agents/graph.rs` | CLI 래퍼 (~100줄) |
| `knowledge.rs` | GraphSource 추가 (~50줄) |
| `lib.rs` | source 등록 1줄 |

**총 예상**: ~150줄 신규. Phase 1 코드 변경 없음.

---

## 참고 자료

- rawq 소스: `_research/_util/rawq/` (9,653줄 Rust)
- rawq 검색 엔진: `rawq/src/search/engine.rs` (1,092줄, RRF 퓨전)
- code-review-graph: `_research/_util/code-review-graph/` (92파일)
- 현재 ContextPack: `src-tauri/src/commands/agents_helpers/context_pack.rs`
- 현재 FTS5: `src-tauri/src/commands/context_queries.rs`
- 현재 rawq 연동: `src-tauri/src/agents/rawq.rs` (472줄)
- 관련 아이디어: `docs/ideas/vectorDbAndRetrievalAlgorithmsIdea.md`
- Multi-agent context: `docs/reference/multiAgentContextStrategy.md`
