# rawq + code-review-graph 통합 진화 전략

> Status: idea
> Created: 2026-04-02
> 관련: `knowledgeLayerArchitectureIdea.md` (ContextPack 주입), `workflowGraphEnhancementIdea.md` (워크플로우 활용)

---

## 1. 두 도구의 본질적 차이

| | rawq | code-review-graph |
|---|------|-------------------|
| **질문** | "이것과 관련된 코드는?" | "이것과 연결된 코드는?" |
| **방법** | 검색 (embedding + BM25) | 탐색 (AST 파싱 + 관계 추출) |
| **입력** | 자연어 쿼리 | 심볼/파일 경로 |
| **출력** | ranked snippets + confidence | nodes + edges + relations |
| **강점** | 모호한 질문, 개념 검색 | 정확한 구조, 영향 범위 |
| **약점** | 구조적 관계 모름 | 의미적 유사성 모름 |

**겹치지 않는다.** 서로 다른 축의 정보를 제공한다.

---

## 2. 함께 쓸 때의 시너지

### 패턴 A: rawq 검색 → graph 확장

```
사용자: "에러 핸들링 개선해줘"
  ↓
rawq: "error handling" 검색
  → errors.rs (confidence 0.87)
  → api/response.rs (confidence 0.72)
  → agents/claude.rs (confidence 0.65)
  ↓
graph: errors.rs 기준 1-hop 확장
  → 호출자: agents/*.rs (4파일), commands/*.rs (12파일)
  → 테스트: tests/error_test.rs
  → 의존: thiserror, serde
  ↓
에이전트가 받는 컨텍스트:
  "errors.rs가 핵심이고, 16파일이 의존하며, 테스트는 1개뿐"
```

rawq가 **어디를** 찾고, graph가 **얼마나 영향이 큰지** 알려준다.

### 패턴 B: graph 구조 → rawq 의미 보강

```
워크플로우: Developer가 auth/middleware.rs를 수정
  ↓
graph: middleware.rs의 caller 목록
  → routes.rs:42, guards.rs:18, ws_handler.rs:23
  ↓
rawq: 각 caller에 대해 "auth middleware usage" semantic 검색
  → routes.rs: "JWT 토큰 검증 후 route guard 적용" (문맥 파악)
  → ws_handler.rs: "WebSocket 연결 시 인증 미들웨어 직접 호출" (위험 패턴 발견)
  ↓
Reviewer가 받는 컨텍스트:
  "ws_handler가 middleware를 비표준 방식으로 호출 — 변경 시 주의"
```

graph가 **연결을** 찾고, rawq가 **맥락을** 보강한다.

### 패턴 C: 독립 사용 (겹치지 않는 영역)

```
rawq만:
  - "Docker 배포 관련 설정 어디있어?" → 개념 검색, 구조 무관
  - "이전에 논의한 성능 최적화 방법" → 문서/대화 검색
  - 한국어 쿼리 → embedding 기반 의미 매칭

graph만:
  - "이 함수 삭제하면 뭐가 깨져?" → 의존성 역추적
  - "이 모듈의 테스트 커버리지" → 테스트 매핑
  - "순환 의존성 있어?" → 그래프 분석
```

---

## 3. 통합 전략: 3단계 진화

### Stage 1: rawq 단독 고도화 (지금)

rawq의 미사용 기능을 활성화하고, Knowledge Layer 인터페이스를 만든다.

```
변경:
  rawq.rs       → search_with_options() 확장 (--rerank, --token-budget 등)
  knowledge.rs  → KnowledgeSource trait + KnowledgeLayer
  context_pack.rs → KnowledgeLayer로 대체

결과:
  - rawq가 코드 + 문서를 모두 검색
  - 검색 품질 개선 (rerank, 동적 rrf-weight)
  - ContextPack 예산 배분 자동화
  - 향후 소스 추가를 위한 인터페이스 확립
```

**graph 관련 준비**: `KnowledgeSource` trait과 `ChunkType::Structure` 정의. 실제 구현은 없지만 자리는 마련.

### Stage 2: graph 도입 + rawq 연동 (다음)

code-review-graph를 sidecar로 추가하고, rawq와 연동한다.

```
변경:
  agents/graph.rs     → CLI 래퍼 (rawq.rs와 동일 패턴)
  knowledge.rs        → GraphSource 추가 (KnowledgeSource 구현)
  workflowOrchestration.ts → 워크플로우 프롬프트에 구조 정보 삽입

연동 패턴:
  1. Knowledge Layer 내부:
     rawq 결과 → graph 확장 → 통합 dedup → ContextPack
  
  2. 워크플로우 내부:
     graph 직접 호출 → Developer/Reviewer 프롬프트 보강
```

### Stage 3: 피드백 루프 (장기)

rawq와 graph가 서로를 개선한다.

```
rawq 검색 결과의 활용 패턴 → graph 탐색 범위 조정
  "사용자가 자주 검색하는 모듈" → graph에서 해당 모듈 주변을 미리 인덱싱

graph 구조 정보 → rawq 검색 품질 개선
  "caller가 많은 함수" → rawq 검색에서 가중치 상향 (hub score)
  "테스트가 없는 모듈" → rawq 검색에서 위험 태그
```

---

## 4. 바이너리 관리 전략

### 현재: rawq만

```
src-tauri/binaries/
  └── rawq-aarch64-apple-darwin     (52MB)
```

### Stage 2: rawq + graph

```
src-tauri/binaries/
  ├── rawq-aarch64-apple-darwin     (52MB)
  └── crg-aarch64-apple-darwin      (??MB)  ← code-review-graph
```

두 바이너리 모두:
- sidecar 번들 (tauri.conf.json `externalBin`)
- 동일한 binary resolution 패턴 (env → sidecar → known path → PATH)
- 동일한 bootstrap script 패턴 (scripts/build-crg.sh)

### Daemon 관리

```
rawq daemon:  ONNX 모델 상주 (임베딩 가속)      → 앱 시작 시 자동
graph daemon: AST 인덱스 상주? (필요성 검토 필요) → 아마 불필요 (tree-sitter 파싱이 빠름)
```

rawq는 임베딩 모델 로딩이 30-60초라서 daemon이 필수지만, graph는 tree-sitter 파싱이 빠르므로 on-demand 호출로 충분할 가능성이 높다. 실측 후 판단.

---

## 5. 인덱스 공유 가능성

### rawq 인덱스

```
~/.cache/rawq/indexes/<project-hash>/
  ├── manifest.json        (파일 목록 + 해시)
  ├── tantivy/             (BM25 인덱스)
  └── embeddings/          (벡터 인덱스)
```

- 청크 단위: AST function/class/block
- 변경 감지: SHA-256 per chunk

### graph 인덱스 (예상)

```
~/.cache/crg/indexes/<project-hash>/
  ├── nodes.json           (심볼 목록)
  ├── edges.json           (관계 목록)
  └── file_map.json        (파일 → 심볼 매핑)
```

- 노드 단위: 함수, 클래스, 모듈
- 엣지: calls, imports, tests, implements

### 공유할 수 있는 것

| 자원 | 공유 가능? | 방법 |
|------|----------|------|
| **파일 목록 + 해시** | O | rawq manifest → graph가 변경 파일만 재파싱 |
| **AST 파싱 결과** | 부분적 | rawq의 tree-sitter 청크 → graph가 관계 추출에 재활용 |
| **변경 감지** | O | rawq의 git-aware 감지 → graph re-index 트리거 |
| **FS watcher 이벤트** | O | tunaFlow의 tauri-plugin-fs watch → 양쪽 모두 재인덱싱 |

### 공유하면 안 되는 것

| 자원 | 이유 |
|------|------|
| 인덱스 자체 | 형식이 다름 (tantivy vs 관계 그래프) |
| 청크 경계 | rawq는 의미 단위, graph는 심볼 단위 |

---

## 6. API 설계 원칙

### 원칙 1: rawq와 graph는 독립적으로 동작한다

```rust
// rawq 없어도 graph 동작
let graph_result = graph::expand(path, symbols)?;

// graph 없어도 rawq 동작
let search_result = rawq::search(path, query)?;

// 둘 다 있으면 연동
let combined = knowledge_layer.query(prompt, budget)?;
```

어느 쪽이든 설치 안 되어 있으면 **해당 소스만 건너뜀**. 다른 소스에 영향 없음.

### 원칙 2: 연동은 KnowledgeLayer에서만

```rust
// rawq.rs — rawq만 알면 됨. graph 모름
pub fn search(path: &str, query: &str, opts: SearchOptions) -> Result<Vec<SearchResult>>

// graph.rs — graph만 알면 됨. rawq 모름
pub fn expand(path: &str, symbols: &[String], depth: u32) -> Result<GraphExpansion>

// knowledge.rs — 둘을 조합
impl KnowledgeLayer {
    fn query(&self, prompt: &str, ctx: &QueryContext, budget: usize) -> Vec<KnowledgeChunk> {
        let rawq_results = self.rawq.query(prompt, ctx, rawq_budget)?;
        let graph_results = self.graph.query(prompt, ctx, graph_budget)?;
        // 여기서만 조합/dedup
        merge_and_dedup(rawq_results, graph_results)
    }
}
```

### 원칙 3: 워크플로우에서의 graph 호출은 직접

```typescript
// workflowOrchestration.ts — graph 직접 호출 (Knowledge Layer 거치지 않음)
const coverage = await invoke("graph_coverage_check", { projectPath, changedFiles });
```

ContextPack 주입(Knowledge Layer)과 워크플로우 프롬프트 보강은 **다른 경로**. Knowledge Layer를 거쳐야 할 이유가 없는 워크플로우 전용 graph 호출은 직접.

---

## 7. 판단 기준: "지금 graph가 필요한가?" (2026-04-03 갱신)

### 현재 상태: Stage 1 완료, Stage 2 보류

**Stage 1 ✅ 완료 (세션 7)**:
- rawq search 옵션 전면 활성화 (rerank, text-weight, rrf-weight, token-budget)
- prompt_needs_rawq() 완화 → 코드 키워드 없어도 검색 포함
- 자동 세션 발견 (session_links + FTS5 + vector)
- Vector DB (conversation_chunks + cosine + FTS5 하이브리드)
- 토픽별 메모리 압축

**KnowledgeLayer trait는 보류**: 5개 소스 하드코딩으로 충분. 상세 → `knowledgeLayerArchitectureIdea.md` §6

### graph가 필요하지 않은 상황

- rawq 검색만으로 에이전트가 충분한 컨텍스트를 받는 경우
- 프로젝트가 작아서 (< 50파일) 구조 탐색이 불필요한 경우
- 워크플로우를 아직 실사용하지 않는 경우 (일반 대화만)

### graph가 필요해지는 시점 (구체적 트리거)

1. **"변경 영향 범위 누락" 리뷰 피드백 2회 이상** — Developer가 caller를 놓쳐서 Review fail이 반복
2. **"테스트 커버리지 판단 불가" Reviewer 불만** — 변경 파일의 테스트 유무를 구조적으로 확인할 수 없음
3. **프로젝트 100+ 파일** — rawq 검색만으로는 구조 파악에 한계, 에이전트에게 graph 제공 필요
4. **워크플로우 풀사이클 3회 이상 완료** — Plan→Dev→Review→Done 경험이 쌓여야 graph 가치 판단 가능

**이 트리거가 발생하기 전까지는 rawq 단독으로 충분하다.**

---

## 8. 문서 간 관계

```
rawqGraphEvolutionStrategyIdea.md (이 문서)
  ├── 두 도구의 본질적 차이와 시너지
  ├── 3단계 진화 전략
  └── 바이너리/인덱스/API 설계 원칙

  참조 ↓

knowledgeLayerArchitectureIdea.md
  ├── KnowledgeSource trait 정의
  ├── RawqSource, GraphSource 구현
  └── ContextPack 주입 (일반 대화)

workflowGraphEnhancementIdea.md  
  ├── Plan/Developer/Reviewer 단계별 graph 활용
  ├── 워크플로우 프롬프트 보강
  └── graph_expand, graph_coverage_check commands
```

| 구현 시점 | 참조할 문서 |
|----------|-----------|
| Phase 1 (rawq 고도화) | `knowledgeLayerArchitectureIdea.md` §3-4 |
| Phase 2 (graph 도입) | 이 문서 §3 Stage 2 + `knowledgeLayerArchitectureIdea.md` §4 |
| Phase 2 (워크플로우 보강) | `workflowGraphEnhancementIdea.md` §2-4 |

---

## 참고 자료

- rawq 소스: `_research/_util/rawq/` (9,653줄)
  - 검색 엔진: `src/search/engine.rs` (Hybrid RRF)
  - AST 청킹: `src/index/chunker/code.rs` (tree-sitter 16언어)
- code-review-graph 소스: `_research/_util/code-review-graph/` (92파일)
- tunaFlow rawq 연동: `src-tauri/src/agents/rawq.rs` (472줄)
- tunaFlow ContextPack: `src-tauri/src/commands/agents_helpers/context_pack.rs`
- tunaFlow 워크플로우: `src/lib/workflowOrchestration.ts`
