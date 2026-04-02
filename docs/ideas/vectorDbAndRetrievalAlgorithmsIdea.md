# Vector DB & 검색 고도화 알고리즘 레퍼런스

> Status: idea
> Created: 2026-04-01
> 목적: tunaFlow Vector DB 구현 시점에 참고할 알고리즘, 논문, 벤치마크 모음
> 관련 계획: `docs/plans/conversationVectorSearchPlan.md`, `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`

---

## 1. 현재 tunaFlow 검색 파이프라인

```
사용자 프롬프트
  → FTS5 키워드 매칭 (messages_fts)
  → 청크 조립 (Q&A pair, anchor, brief)
  → 점수: FTS×0.5 + recency×0.2 + kind_bonus - overlap_penalty
  → Jaccard dedup
  → Top-N trim
  → ContextPack 주입 (MAX_RETRIEVAL_SECTION: 4,000 chars)
```

**한계**: 의미 검색 없음. "배포 방식 변경 결정"을 "도커 대신 다른 방법"으로 검색 불가.

---

## 2. Hybrid Search — FTS + Vector 결합

### 개념

두 가지 검색을 병렬 실행 후 결과를 융합:

```
Query → [Sparse Retriever (FTS5/BM25)] → 키워드 기반 결과
      → [Dense Retriever (Vector)]      → 의미 기반 결과
      → [Fusion Algorithm]              → 통합 랭킹
      → [Optional: Reranker]            → 최종 정밀 순위
```

### Reciprocal Rank Fusion (RRF)

가장 널리 쓰이는 융합 알고리즘. Elasticsearch, Weaviate, Meilisearch 등 프로덕션 시스템에서 검증됨.

```python
# RRF 점수 계산
def rrf_score(doc, rankings, k=60):
    score = 0
    for ranking in rankings:
        rank = ranking.index(doc) + 1  # 1-based
        score += 1.0 / (k + rank)
    return score
```

- `k=60`: 기본 상수 (상위 결과와 하위 결과의 점수 차이를 조절)
- 각 검색 결과의 순위만 사용 → 점수 스케일 차이에 무관
- **tunaFlow 적용**: FTS5 결과 순위 + Vector 결과 순위 → RRF로 병합

### Weighted Linear Combination

```
final_score = α × sparse_score + (1-α) × dense_score
```

- α 조정으로 키워드/의미 검색 비중 조절
- 키워드 정확도가 중요한 코드 검색: α=0.6
- 의미 이해가 중요한 대화 검색: α=0.3

### 참고 자료

- [Hybrid Search Explained — Weaviate](https://weaviate.io/blog/hybrid-search-explained)
- [Hybrid Search for RAG: BM25, SPLADE, and Vector Search Combined](https://blog.premai.io/hybrid-search-for-rag-bm25-splade-and-vector-search-combined/)
- [Optimizing RAG with Hybrid Search & Reranking — Superlinked](https://superlinked.com/vectorhub/articles/optimizing-rag-with-hybrid-search-reranking)

### tunaFlow 구현 시 고려사항

tunaFlow는 이미 FTS5 파이프라인이 동작 중이므로, Vector 검색을 **병렬 추가**하고 RRF로 병합하는 것이 가장 자연스러운 경로:

```rust
// 구현 스케치
let fts_results = fts5_search(query, project_key, limit * 2);
let vec_results = vector_search(query_embedding, project_key, limit * 2);
let fused = reciprocal_rank_fusion(&fts_results, &vec_results, k=60);
let final_results = fused.into_iter().take(limit).collect();
```

---

## 3. Re-ranking — 검색 결과 정밀 재정렬

### 2-stage 파이프라인

```
Stage 1: Fast retrieval (FTS + Vector) → 후보 20-50개
Stage 2: Precision reranking            → 최종 3-5개
```

Stage 1은 빠르지만 정밀도 낮음. Stage 2가 정밀도를 올림.

### Cross-Encoder Reranking

두 텍스트를 동시에 입력하여 관련도 점수를 직접 계산:

```
cross_encoder("query: 배포 방식 변경", "doc: 도커 대신 fly.io로 배포하기로 결정") → 0.92
```

- **장점**: 가장 정확 (MS MARCO MRR@10 > 40)
- **단점**: 느림 (모든 후보에 대해 개별 추론 필요)
- **대표 모델**: `cross-encoder/ms-marco-MiniLM-L-6-v2` (빠름), `bge-reranker-v2-m3` (다국어)

### ColBERT — Late Interaction

```
query tokens  → [encoder] → query embeddings (token-level)
doc tokens    → [encoder] → doc embeddings (token-level)
score = MaxSim(query_embeddings, doc_embeddings)
```

- **장점**: Cross-Encoder 수준 정확도, 문서 임베딩 사전 계산으로 빠름
- **단점**: 저장 공간 증가 (토큰별 벡터)
- **대표 모델**: ColBERT v2, ColQwen2 (다국어)

### LLM-based Reranking

```
prompt: "다음 문서가 질문에 관련이 있습니까? 질문: {query}, 문서: {doc}"
→ LLM이 yes/no 또는 관련도 점수 출력
```

- **RankGPT**: LLM이 후보 목록을 보고 직접 순위를 매김
- **ICR (In-Context Reranking)**: few-shot 예시로 재랭킹 → ColBERT v2 대비 Recall@2 5-6% 향상
- **비용**: LLM 호출 필요 → 후보 수에 비례하여 비용 증가

### 참고 자료

- [Cross-Encoders, ColBERT, and LLM-Based Re-Rankers: A Practical Guide](https://medium.com/@aimichael/cross-encoders-colbert-and-llm-based-re-rankers-a-practical-guide-a23570d88548)
- [How Good are LLM-based Rerankers? An Empirical Analysis](https://arxiv.org/html/2508.16757v1) — EMNLP 2025
- [SciRerankBench: Benchmarking Rerankers](https://arxiv.org/html/2508.08742v1)
- [A Primer on Re-Ranking for Retrieval Systems](https://vizuara.substack.com/p/a-primer-on-re-ranking-for-retrieval)
- [Late Interaction Overview: ColBERT, ColPali, ColQwen — Weaviate](https://weaviate.io/blog/late-interaction-overview)
- [ICLR 2025 — Reranking Conference Paper](https://proceedings.iclr.cc/paper_files/paper/2025/file/b5b1890a7c1a79fe9b32b0f442347089-Paper-Conference.pdf)

### tunaFlow 구현 시 권장 경로

```
Phase 1: RRF만으로 시작 (추가 모델 불필요)
Phase 2: Cross-Encoder 도입 (bge-reranker-v2-m3, 로컬 실행 가능)
Phase 3: 필요 시 LLM reranking 검토 (비용-정밀도 트레이드오프)
```

---

## 4. 임베딩 모델 선택

### 벤치마크: MTEB / MMTEB

[MTEB Leaderboard](https://huggingface.co/spaces/mteb/leaderboard) — 임베딩 모델의 표준 벤치마크.
[MMTEB](https://arxiv.org/abs/2502.13595) — 250+ 언어, 500+ 태스크, 코드 검색 포함. 2025년 확장판.

### tunaFlow 요구사항

| 요구사항 | 이유 |
|---------|------|
| 한국어 + 영어 다국어 | 사용자는 한국어, 코드/식별자는 영어 |
| 코드 검색 | rawq 보완 (대화 속 코드 언급 검색) |
| 로컬 실행 | 외부 API 의존 없이 (rawq daemon 활용 가능) |
| 경량 | 데스크톱 앱이므로 GPU 없이 CPU 추론 |

### 후보 모델 비교

| 모델 | 차원 | 크기 | 한국어 | 코드 | 비고 |
|------|------|------|--------|------|------|
| `snowflake-arctic-embed-s` | 384 | 33M | △ 제한적 | ○ | **현재 rawq 사용 중** |
| `all-MiniLM-L6-v2` | 384 | 22M | △ 제한적 | △ | 경량, 영문 특화 |
| `bge-m3` (BAAI) | 1024 | 568M | ◎ 우수 | ○ | 다국어 최강급, 크기 주의 |
| `multilingual-e5-large` | 1024 | 560M | ◎ 우수 | ○ | MS 제작, 다국어 |
| `multilingual-e5-small` | 384 | 118M | ○ 양호 | △ | e5 경량 버전 |
| `KoSimCSE-roberta` | 768 | 110M | ◎ 한국어 특화 | × | 한국어 전용 |
| `GTE-Qwen2-7B-instruct` | 3584 | 7.6B | ◎ 우수 | ◎ | 최고 성능, GPU 필수 |

### 권장 경로

```
Phase 1: snowflake-arctic-embed-s 그대로 사용 (rawq daemon 재활용)
         → 추가 인프라 비용 0, 한국어 약점은 FTS5가 보완 (hybrid)

Phase 2: multilingual-e5-small (118M) 도입 평가
         → 한국어 + 영어 + 코드 균형, CPU 실행 가능

Phase 3: bge-m3 (568M) 고려 — 한국어 품질이 결정적일 때
         → 크기 대비 성능 최강, 로딩 시간/메모리 검증 필요
```

### 참고 자료

- [MMTEB: Massive Multilingual Text Embedding Benchmark](https://arxiv.org/abs/2502.13595) — 2025
- [MTEB Leaderboard — Hugging Face](https://huggingface.co/spaces/mteb/leaderboard)
- [Comparative Analysis of Qwen-3 and BGE-M3](https://medium.com/@mrAryanKumar/comparative-analysis-of-qwen-3-and-bge-m3-embedding-models-for-multilingual-information-retrieval-72c0e6895413)
- [KoSimCSE — Korean Embedding](https://www.mdpi.com/2076-3417/13/9/5771)

---

## 5. 대화 메모리 청킹 전략

### 현재 tunaFlow 청킹

```
messages → FTS5 hit → pair chunk (user + assistant)
                    → anchor chunk (branch checkpoint)
                    → brief chunk (RT roundtable brief)
```

### 최신 연구 — Event-Based Decomposition Units (EDU)

> 출처: [A Simple Yet Strong Baseline for Long-Term Conversational Memory](https://arxiv.org/html/2511.17208v1) — 2025

기존 "메시지 단위" 또는 "턴 단위" 대신, **이벤트 단위**로 분해:

```
원문: "어제 도커 배포를 논의했는데, 용량 문제로 fly.io를 쓰기로 했어.
       그리고 DB는 Supabase로 결정."

EDU 분해:
1. "도커 배포를 논의함 (날짜: 어제)"
2. "도커 → fly.io로 배포 방식 변경 (이유: 용량 문제)"
3. "DB는 Supabase로 결정"
```

각 EDU는 **자기 완결적** (self-contained) — 검색 시 단독으로 의미를 가짐.

### A-Mem: Agentic Memory (2025)

> 출처: [A-Mem: Agentic Memory for LLM Agents](https://arxiv.org/html/2502.12110v1)

에이전트가 **스스로 메모리를 관리**:
- 새 정보 → 기존 메모리와 유사하면 **병합**
- 모순되면 **업데이트**
- 관련 없으면 **새 메모리 생성**
- 오래되고 안 쓰이면 **삭제**

### Utility-Based Deletion

> 출처: [Supermemory Research](https://supermemory.ai/research/)

메모리 블로트 방지:
- 각 메모리에 **효용도 점수** 부여 (검색 빈도 × recency × relevance)
- 효용도 낮은 메모리 정기 삭제 → 최대 10% 성능 향상
- 검색 히스토리 기반 삭제가 랜덤 삭제보다 효과적

### Reflective Memory (적응적 단위)

> 출처: [Persistent Memory in LLM Agents](https://www.emergentmind.com/topics/persistent-memory-for-llm-agents)

메모리 단위를 고정하지 않고 **적응적으로 결정**:
- 발화 (utterance) → 턴 (turn) → 세션 (session) → 토픽 (topic)
- 검색 결과의 citation 피드백으로 메모리 관련성을 온라인 학습
- 강화학습으로 reranking 정책 개선

### tunaFlow 청킹 개선 방향

```
현재: message pair 단위 (고정)
  ↓
Phase 1: 현재 유지 + recency decay 강화
  ↓
Phase 2: EDU 분해 도입 (결정/이유/제약 단위로 분리)
  ↓
Phase 3: utility-based 삭제 (검색 빈도 추적 → 저활용 청크 아카이브)
```

### 참고 자료

- [A Simple Yet Strong Baseline for Long-Term Conversational Memory](https://arxiv.org/html/2511.17208v1) — 2025
- [A-Mem: Agentic Memory for LLM Agents](https://arxiv.org/html/2502.12110v1) — 2025
- [Supermemory Research — State-of-the-Art Agent Memory](https://supermemory.ai/research/)
- [Chunking Strategies for RAG — Weaviate](https://weaviate.io/blog/chunking-strategies-for-rag)
- [Beyond a Million Tokens: Long-Term Memory in LLMs](https://arxiv.org/html/2510.27246v1) — 2025
- [Human-Like Remembering and Forgetting — ACT-R Memory](https://dl.acm.org/doi/10.1145/3765766.3765803) — 2024
- [Agent Memory Paper List (Survey)](https://github.com/Shichun-Liu/Agent-Memory-Paper-List)

---

## 6. 비용 영향 분석 (현재 미계획)

### 장기기억이 비용에 미치는 실제 영향

```
요청당 비용 = ContextPack budget 이내 (고정 상한)
세션당 비용 = 요청당 비용 × 요청 횟수
                              ↑
                    장기기억 품질이 이것을 결정
```

| 시나리오 | 요청당 input | 완료까지 요청 수 | 세션 총 비용 (Opus) |
|---------|-------------|----------------|-------------------|
| 장기기억 없음 | ~8k tokens | 5-8회 (배경 재설명 + 재시도) | ~$1.20 |
| FTS5만 | ~9k tokens | 3-5회 | ~$0.75 |
| Hybrid (FTS + Vector) | ~10k tokens | 2-3회 | ~$0.50 |
| Hybrid + Reranking | ~10k tokens | 2회 | ~$0.40 |

**장기기억 도입 = 요청 횟수 감소 = 총 비용 절감**

### 추적해야 할 메트릭

| 메트릭 | 측정 방법 | 목적 |
|--------|----------|------|
| 작업당 요청 횟수 | plan 승격 → done까지 메시지 수 | 맥락 품질 평가 |
| 재시도 비율 | 같은 질문 반복 감지 | 메모리 부족 신호 |
| retrieval hit rate | 검색 결과가 실제 사용된 비율 | 검색 정밀도 |
| 요청당 비용 분포 | trace_log의 tokens/cost | budget 효율성 |

### 참고

현재 tunaFlow에 `trace_log` 테이블이 있어 토큰/비용 추적 인프라는 이미 존재. 메트릭 집계 뷰만 추가하면 됨.

---

## 7. 구현 시점 체크리스트

Vector DB 구현을 시작할 때 확인할 항목:

### 알고리즘 선택

- [ ] Hybrid Search fusion: RRF vs Linear Combination 선택 (RRF 권장)
- [ ] Reranker 필요 여부: Cross-Encoder vs LLM-based vs 없음
- [ ] 임베딩 모델: rawq 기존(arctic-s) 재사용 vs 다국어 모델 전환
- [ ] 청킹 전략: message pair 유지 vs EDU 분해

### 벤치마크 검증

- [ ] MMTEB/MTEB에서 후보 모델의 한국어 + 코드 검색 점수 확인
- [ ] tunaFlow 실제 대화 데이터로 recall@5 측정 (FTS5 단독 vs Hybrid)
- [ ] Reranker 추가 시 latency 오버헤드 측정 (< 200ms 목표)

### 인프라 결정

- [ ] 벡터 저장: sqlite-vec vs 별도 DB (LanceDB/Qdrant embedded)
- [ ] 임베딩 실행: rawq daemon 재활용 vs 별도 프로세스
- [ ] 인덱스 업데이트 시점: 메시지 저장 시 즉시 vs 배치 (fs watcher 패턴 재활용)

### 비용 모니터링

- [ ] 요청당 retrieval 토큰 비용 추적
- [ ] 장기기억 도입 전후 세션당 요청 횟수 비교
- [ ] Reranker LLM 호출 비용 vs 검색 품질 향상 트레이드오프

---

## 8. 논문/자료 전체 목록

### Hybrid Search
- [Hybrid Search Explained — Weaviate](https://weaviate.io/blog/hybrid-search-explained)
- [Hybrid Search for RAG: BM25, SPLADE, Vector — PremAI](https://blog.premai.io/hybrid-search-for-rag-bm25-splade-and-vector-search-combined/)
- [Optimizing RAG with Hybrid Search & Reranking — Superlinked](https://superlinked.com/vectorhub/articles/optimizing-rag-with-hybrid-search-reranking)
- [Building Contextual RAG with Hybrid Search — Analytics Vidhya](https://www.analyticsvidhya.com/blog/2024/12/contextual-rag-systems-with-hybrid-search-and-reranking/)

### Re-ranking
- [How Good are LLM-based Rerankers? — EMNLP 2025](https://arxiv.org/html/2508.16757v1)
- [SciRerankBench — Reranker Benchmark 2025](https://arxiv.org/html/2508.08742v1)
- [Late Interaction: ColBERT, ColPali, ColQwen — Weaviate](https://weaviate.io/blog/late-interaction-overview)
- [Cross-Encoders, ColBERT, LLM Re-Rankers — Practical Guide](https://medium.com/@aimichael/cross-encoders-colbert-and-llm-based-re-rankers-a-practical-guide-a23570d88548)
- [ICLR 2025 Reranking Paper](https://proceedings.iclr.cc/paper_files/paper/2025/file/b5b1890a7c1a79fe9b32b0f442347089-Paper-Conference.pdf)

### 임베딩 모델
- [MMTEB: Massive Multilingual Text Embedding Benchmark](https://arxiv.org/abs/2502.13595) — 2025
- [MTEB Leaderboard — Hugging Face](https://huggingface.co/spaces/mteb/leaderboard)
- [Qwen-3 vs BGE-M3 Multilingual Comparison](https://medium.com/@mrAryanKumar/comparative-analysis-of-qwen-3-and-bge-m3-embedding-models-for-multilingual-information-retrieval-72c0e6895413)

### 대화 메모리 / 청킹
- [Long-Term Conversational Memory Baseline](https://arxiv.org/html/2511.17208v1) — 2025
- [A-Mem: Agentic Memory](https://arxiv.org/html/2502.12110v1) — 2025
- [Supermemory Research](https://supermemory.ai/research/)
- [Chunking Strategies for RAG — Weaviate](https://weaviate.io/blog/chunking-strategies-for-rag)
- [Beyond a Million Tokens: Long-Term Memory](https://arxiv.org/html/2510.27246v1) — 2025
- [Human-Like Remembering and Forgetting — ACT-R](https://dl.acm.org/doi/10.1145/3765766.3765803) — 2024
- [Agent Memory Paper List (Survey)](https://github.com/Shichun-Liu/Agent-Memory-Paper-List)

### 비용 최적화
- tunaFlow 내부: `docs/plans/contextPackAlgorithmImprovementsPlan.md`
- tunaFlow 내부: `docs/plans/contextBudgetScalingPlan.md`
