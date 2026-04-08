# ContextPack Tiering — Push 경량화 + 선택적 Pull + 벡터 맥락 공유

> Status: idea
> Created: 2026-04-08
> 근거: RT 토큰 효율 분석 → $20 플랜 사용자 접근성 → 전체 ContextPack 구조 재검토

---

## 제품 동기: $20 플랜 사용자의 AOC 접근성

tunaFlow는 "다중 에이전트 오케스트레이션 클라이언트(AOC)". 핵심 가치인 멀티에이전트를 쓰려면 현재 토큰 비용이 너무 높음.

### 현실 계산

```
Claude Pro $20: Sonnet 기준 일 45-50회 대화 (rate limit)
ChatGPT Plus $20: 유사한 제한

RT 1회 (3명 × 2라운드):
  6요청 × ~5-7k 입력 토큰 = ~40-55k 토큰
  → 일일 한도의 15-20% 소진

$20 유저가 RT 5-6번 ≈ 하루 한도 소진
→ 핵심 기능(멀티에이전트)을 쓸수록 단일 대화 여유 없어짐
→ "Opus를 원없이 쓸 수 있는 max100+ 유저"만을 위한 도구가 됨
```

### 목표

| | 현재 | 목표 |
|---|---|---|
| RT 1회 비용 | 일일 한도 15-20% | **3-4%** |
| $20 유저 RT | 하루 5-6회 한계 | **하루 20+회** |
| AOC 가치 | 고액 플랜 전용 도구 | **보편적 오케스트레이션** |

---

## 핵심 문제 3가지

### 1. 노이즈 (Codex 리뷰 반영)

rawq + skills + memory + cross-session + artifacts가 한 번에 들어오면 에이전트의 신호 대 잡음비(SNR)가 낮아짐. **토큰 비용보다 프롬프트 품질 문제가 더 큼.** 목표는 "덜 넣기"이기도 하지만, 더 정확히는 "기본 prompt를 더 선명하게 만들기".

### 2. 맥락 공유 = 복제

현재 멀티에이전트 맥락 공유: A의 발언 → ContextPack에 텍스트로 직렬화 → B 입력에 통째로 삽입. **"공유"가 아니라 "복제"**. 3명이면 같은 정보가 3벌 + ContextPack 고정비용 N배 증폭.

### 3. 고정비용

매 요청마다 ~12-18k자(~5-7k 토큰). 10번 중 7번은 skills/rawq/memory를 안 쓰는데 매번 포함. RT에서는 6요청 × 고정비용 = ~30k 토큰이 context만으로 소진.

---

## 현재 상태 — 이미 순수 Push가 아님

- `prompt_needs_rawq()` — 코드 키워드 감지 시만 rawq 포함
- context mode (lite/standard/full) — 모드별 섹션 포함 차등
- tool-request 마커 4종 (docs/rawq/graph/plans) — 에이전트 자발적 Pull
- context-hub — **이미 Push → Pull 전환 완료** (자동 injection 비활성화, tool-request 기반)
- 스킬 키워드 매칭 — 관련 스킬 섹션만 선별 주입
- RtContextCache — auto/lite 2단계 캐싱

---

## 제안 A: 3-Tier 하이브리드 ContextPack

### Tier 0 — 항상 포함 (Always Push)

에이전트가 "나는 누구, 여기는 어디, 지금 뭘 하는지" 파악하는 최소 정보.

- identity 최소 (profile + engine + persona)
- project 기본 정보 (이름, 경로, 스택)
- 현재 작업 한 줄 요약
- **~1.5k자 목표**

### Tier 1 — 상황별 휴리스틱 Push (Conditional Push)

**현재 작업의 진실원** — Pull로 내리면 작업 전제가 무너지는 것들.

| 조건 | 포함 섹션 |
|------|----------|
| active plan 존재 | plan context (subtasks, phase) |
| phase=review | review findings, test output |
| phase=implement | impl brief, plan hints |
| 승인된 plan/verdict | 해당 artifact (**Tier 2로 내리면 안 됨**) |
| branch 대화 | branch inheritance (부모 대화 요약) |
| 코드 토픽 감지 | rawq 결과 |

- **~2-4k자 (상황에 따라 가변)**

### Tier 2 — Pull (tool-request 기반 on-demand)

필요 시 에이전트가 마커로 요청. 기존 tool-request 인프라 활용.

| 소스 | tool-request 타입 | 인프라 상태 |
|------|------------------|-----------|
| context-hub docs | `tool-request:docs:query` | ✅ 이미 동작 |
| rawq (추가 검색) | `tool-request:rawq:query` | ✅ 이미 동작 |
| code-review-graph | `tool-request:graph:pattern` | ✅ 이미 동작 |
| completed plans | `tool-request:plans:completed` | ✅ 이미 동작 |
| compressed memory | `tool-request:memory:topic` | 🆕 신규 필요 |
| cross-session | `tool-request:sessions:query` | 🆕 신규 필요 |
| skills | `tool-request:skills:keyword` | 🆕 신규 필요 |
| 완료된 artifacts | `tool-request:artifacts:id` | 🆕 신규 필요 |
| failure lessons | `tool-request:lessons:pattern` | 🆕 신규 필요 |

---

## 제안 B: 벡터 기반 맥락 공유 (구조화 요약 대체)

### 구조화 요약의 문제

```
원문: "이 함수는 O(n²)인데 캐시 히트율이 95%라서 실제로는 빠릅니다.
       다만 cold start 시 500ms 스파이크가 있고 prewarming으로 해결 가능합니다."
요약: "성능 이슈 없음, prewarming 권장"
→ 다음 에이전트가 "왜 prewarming?"을 물으면 맥락이 없음
→ 요약자가 뭘 버릴지 판단을 잘못하면 핵심 논거가 사라짐
```

요약은 "뭐가 중요한지" 미리 알아야 하는데, 그건 다음 에이전트가 뭘 물어볼지에 달려 있음. 미래 예측 기반 버리기는 본질적으로 위험.

### 세 번째 길: 벡터 검색 기반 선택적 전달

```
전문 복제:  A 발언 전체 → B에 전달              (비쌈)
구조화 요약: A 발언 → 3줄 요약 → B에 전달        (정보 손실)
벡터 검색:  A 발언 → chunk → embed → 저장
            B의 topic 기반 top-K chunk 검색       (효율 + 정확)
```

**저장은 전문, 전달은 선택적. 버리는 게 아니라 "지금 안 꺼낼 뿐".**

### 기존 인프라로 구현 가능

| 인프라 | 현재 상태 | RT 맥락 공유에 활용 |
|-------|----------|------------------|
| conversation_chunks 테이블 | ✅ DB v22 | RT transcript chunk 저장용 |
| rawq embed CLI | ✅ daemon 상주 (~630ms/embed) | chunk 임베딩 생성 |
| brute-force cosine | ✅ 동작 중 | chunk 검색 |
| FTS5+Vector 하이브리드 | ✅ RRF 병합 | 키워드+의미 복합 검색 |

### RT에 적용하면

```
라운드 1:
  Alice 발언 → chunk화 → rawq embed → conversation_chunks INSERT
  Bob 발언 → chunk화 → rawq embed → conversation_chunks INSERT

라운드 2:
  Alice 실행 시:
    topic + Alice의 역할로 top-3 chunk 검색 (Bob 발언 중 관련 부분만)
    ~500-800 토큰으로 핵심 맥락 전달 (전문 ~3-5k 대비 80% 절감)

비용:
  현재: 3명 × 2라운드 transcript 전문 복제 = ~30k 토큰
  벡터: 3명 × 2라운드 top-3 chunk = ~5k 토큰 (83% 절감)
```

---

## 제안 C: sqlite-vec 도입

### 현재 벡터 검색 성능

```
임베딩 생성: rawq embed ~630ms (daemon 모드, 지배적 병목)
검색 (brute-force): ~2ms (2000 chunks)
전체: ~650ms/쿼리
```

brute-force 검색 자체(~2ms)는 빠르지만, 데이터가 10,000+ chunks로 커지면 O(n) 선형 증가. 모든 chunk를 메모리에 로드하는 구조도 비효율적.

### sqlite-vec 도입 근거

1. **HNSW 인덱스**: O(n) → O(log n). chunks 10,000+에서 차이 체감
2. **메모리 효율**: 전체 로드 대신 인덱스 기반 검색
3. **네이티브 통합**: SQLite 확장 → 별도 서버 불필요, Rust에서 직접 사용
4. **쿼리 통합**: SQL WHERE + 벡터 검색 한 쿼리로 가능
5. **미래 대비**: RT 벡터 맥락 공유, Tier 2 Pull 검색 등 벡터 사용처 확대 시 필수

### 현재 vs sqlite-vec

```
현재:
  SELECT * FROM conversation_chunks WHERE project_key = ? AND embedding IS NOT NULL
  → Rust 메모리에서 전수 cosine 계산 → 정렬 → top-K

sqlite-vec:
  SELECT * FROM conversation_chunks
  WHERE project_key = ?
  AND vec_distance(embedding, ?) < threshold
  ORDER BY vec_distance(embedding, ?)
  LIMIT K
  → SQLite 엔진에서 HNSW 검색 → top-K 직접 반환
```

### 도입 범위

- `Cargo.toml`: sqlite-vec 의존성 추가
- `migrations.rs`: vec0 가상 테이블 생성 (기존 conversation_chunks와 연동)
- `vector_search.rs`: `search_similar()` → sqlite-vec 쿼리로 전환
- `context_loading.rs`: 하이브리드 병합 쿼리 간소화

### ChromaDB 제외 근거

tunaFlow는 단일 사용자 데스크톱 앱. ChromaDB는:
- Python 서버 or embedded → Tauri(Rust)와 아키텍처 불일치
- rawq daemon이 이미 임베딩 모델 상주 → 모델 중복
- 수만 건이 아니라 수천 건 수준 → sqlite-vec로 충분
- 별도 프로세스 관리 → 운영 복잡도 불필요 증가

---

## 순수 Pull이 안 되는 이유

tunaFlow는 CLI subprocess 기반. tool-request 1회마다:
- 마커 파싱 → follow-up 프로세스 실행 → 입력 누적
- SDK의 같은 run 안에서 tool call 해결하는 구조가 아님

```
Pull 1회 비용:
  마커 출력 (~200 토큰) + 검색 결과 주입 (~1-2k 토큰) + 프로세스 재시작
  = ~2-3k 토큰 + 5-10초 지연
```

검색 2회 이상이면 Push보다 비쌈. **자주 필요한 것은 Push, 가끔 필요한 것만 Pull.**

---

## 절감 예상 (A+B+C 통합)

```
메인 채팅 (단일 요청):
  현재: ~12-18k자 (~5-7k 토큰)
  Tier 0+1: ~3-6k자 (~1.5-2.5k 토큰)
  절감: 50-70%

RT (3명 × 2라운드):
  현재: 6 × ~15k자 = ~90k자 (~30k 토큰)
  Tier 0+1 + 벡터 맥락 공유: 6 × ~4k자 = ~24k자 (~8k 토큰)
  절감: ~73%
  → $20 유저 일일 한도 소진: 15-20% → 3-4%

장기기억 (compressed memory + cross-session):
  현재: 매 요청 ~1.4k자 Push
  Tier 2 Pull: 필요 시에만 ~1-2k자
  → 10번 중 7번은 0 토큰
```

---

## 제안 D: Chunk 품질 개선 — Parent Document Retriever + 대화 특화 전략

### 현재 chunking의 문제

```
Alice 발언 (800자):
  "O(n²)이지만 캐시 히트율 95%라 실제론 빠르고, cold start 시
   500ms 스파이크가 있는데 prewarming으로 해결 가능합니다."
      ↓ rawq embed 500자 트렁케이션
  "O(n²)이지만 캐시 히트율 95%라 실제론 빠르고, cold start 시..."
      ↓ 384-dim 임베딩
  → "prewarming" 사라짐 → 관련 검색에서 miss
```

자르는 순간 정보가 사라지고, 사라진 정보는 검색할 수 없음.

### 구조화 요약의 한계

요약은 "뭐가 중요한지" 미리 알아야 하는데, 그건 다음 에이전트가 뭘 물어볼지에 달려 있음. 미래 예측 기반 버리기는 본질적으로 위험.

### 세 번째 길: 저장은 전문, 전달은 선택적

```
전문 복제:  A 발언 전체 → B에 전달              (비쌈)
구조화 요약: A 발언 → 3줄 요약 → B에 전달        (정보 손실)
벡터 검색:  A 발언 → child chunk → embed → 저장
            B의 topic으로 관련 chunk 검색
            → root_message_id로 원본 발언 전체 반환  (효율 + 정확)
```

**버리는 게 아니라 "지금 안 꺼낼 뿐". 원본은 항상 messages 테이블에 보존.**

### Parent Document Retriever 패턴

기존 인프라가 이미 지원하는 구조:

```
conversation_chunks:
  root_message_id  TEXT  ← 부모 메시지 참조 (이미 존재!)
  text_preview     TEXT  ← 검색용 child chunk
  embedding        BLOB  ← 임베딩

messages:
  id, content            ← 원본 전체 보존

검색 흐름:
  child chunk 임베딩 매칭 → root_message_id JOIN → messages.content 반환
```

성능 (레퍼런스): 고정 크기 chunking 대비 recall +10-20%, F1 ~15% 향상.

### 대화 특화 chunking 전략

일반 문서와 대화의 차이:

| 특성 | 일반 문서 | 대화 |
|------|----------|------|
| 순서 의존성 | 낮음 | 매우 높음 |
| 대명사/지시어 | 보통 | 매우 빈번 ("그거", "아까 말한") |
| 화자 교대 | 없음 | 핵심 구조 |
| chunk 독립성 | 높음 | 낮음 (맥락 없이 의미 불명) |

#### 적용할 기법 (우선순위순)

**1. 슬라이딩 윈도우 chunk (즉시 적용, 저비용)**
```
현재: 단일 메시지 = 1 chunk
개선: 3-turn 윈도우 = 1 chunk (1-2턴 오버랩)
효과: 대명사/지시어 해결, 맥락 보존. recall +10-15%
구현: create_conversation_chunk() 수정만
```

**2. 화자 메타데이터 prefix (즉시 적용, 저비용)**
```
현재: "매출은 전분기 대비 3% 증가..."
개선: "[Architect · claude] 매출은 전분기 대비 3% 증가..."
효과: 임베딩에 화자/역할 시그널 포함 → 화자별 검색 정밀도 향상
```

**3. Parent retriever 연결 (중기, 중비용)**
```
검색: child chunk hit → root_message_id → 원본 메시지 전체 반환
→ 500자 chunk로 정밀 검색 + 원본 전체로 맥락 전달
→ conversation_memory 토픽이 "더 큰 부모" 역할 가능
```

**4. 토픽 경계 감지 (중기, 중비용)**
```
인접 턴 임베딩 유사도 → 급격 하락점 = 토픽 전환
→ 토픽 단위로 chunk 경계 재설정
→ TextTiling 알고리즘 변형. precision +10-15%
```

### 적용하지 않는 기법과 근거

| 기법 | 제외 이유 |
|------|----------|
| **Contextual Retrieval** (Anthropic) | chunk당 LLM 호출 필요. 대화는 실시간 업데이트 → 비용 폭발 |
| **Late Chunking** (Jina) | rawq embed = 512 토큰 제한 → long-context 모델 아님 |
| **ColBERT/Multi-vector** | 저장 10x 증가 + GPU 필수. 데스크톱 앱에 과도 |
| **RAPTOR** (재귀 요약) | LLM 호출 다수 필요. 실시간 대화 인덱싱 불가 |
| **Agentic Chunking** | 인덱싱 시 LLM 비용. 대화의 자연 경계(turn)가 이미 존재 |

### 레퍼런스

| 기법 | 출처 | 핵심 수치 |
|------|------|----------|
| Parent Document Retriever | LangChain docs, 2024 | recall +10-20%, F1 +15% |
| Contextual Retrieval | Anthropic blog, 2024.09 | retrieval failure -35% (embeddings), -49% (hybrid) |
| Late Chunking | Jina AI, 2024 | NDCG@10 +5-15% (BeIR) |
| Dense X Retrieval (Proposition) | Chen et al., 2024 | recall@5 +12-17% |
| RAPTOR | Sarthi et al., 2024 (Stanford) | NarrativeQA accuracy +20% |
| ColBERT v2 | Santhanam et al., 2022 | MRR@10 0.397 (BM25: 0.187) |
| Semantic Splitting | LlamaIndex SemanticSplitter | recall +8-12% (비공식) |
| 대화 Topic Segmentation | TextTiling 변형 | precision +10-15% (비공식) |

---

## 구현 우선순위

| 순서 | 항목 | 효과 | 난이도 |
|------|------|------|--------|
| 1 | **RT minimal ContextPack** (Tier 0+1만) | RT 70% 절감, 즉시 체감 | 낮 |
| 2 | **Chunk 품질: 슬라이딩 윈도우 + 화자 prefix** | 검색 recall +10-15% | 낮 |
| 3 | **Chunk 품질: Parent retriever 연결** | 원본 반환으로 정보 손실 제거 | 낮 (인프라 있음) |
| 4 | **sqlite-vec 도입** | 검색 O(n)→O(log n), 전수 로드 제거 | 중 |
| 5 | **RT 벡터 맥락 공유** | transcript 복제 83% 절감 | 중 |
| 6 | **Tier 2 tool-request 5종 추가** | 메인 채팅 50% 절감 | 중 (인프라 있음) |
| 7 | **메인 채팅 Tiering 적용** | 전체 ContextPack 경량화 | 높 (휴리스틱 설계) |

---

## RT 토론 검증 결과 (2026-04-08, 세션 16)

3-agent Sequential RT (Claude Sonnet / Codex GPT-5.4 / Gemini 3.1-pro-preview)로 "CLI subprocess 아키텍처 장단점" 토론 실행. RT 중간 스트리밍 정상 동작 확인. 아래는 에이전트 의견 검토.

### 세 에이전트 공통 합의

- CLI subprocess는 tunaFlow의 "구독자를 위한 오케스트레이터" 포지션에 최적화된 선택
- 장기적으로 프로세스/이벤트/취소 제어 계층이 병목이 될 수 있음
- API SDK 전환은 불필요, 현재 구조 위에서 개선하는 방향이 맞음

### 에이전트별 검토

**Claude (Sonnet)** — 정확하지만 일부 outdated
- "RT 중간 스트리밍이 없다"고 지적 → **이미 해결됨** (이 토론 자체가 새 스트리밍으로 동작)
- "CLI stdout 파싱으로 실시간 진행상황이 어렵다" → **이미 동작 중** (`--output-format stream-json` JSONL)
- 구조적 트레이드오프 테이블은 깔끔. 나머지 장단점 분석은 정확

**Codex (GPT-5.4)** — 가장 actionable한 제안
- ✅ "내부 프로토콜을 line-delimited structured event로 엄격하게" → Tiering과 함께 설계 가능
- ✅ "start, delta, tool_request, error, done 공통 이벤트 모델 강제" → 현재 엔진마다 이벤트 형식 상이. 통합 필요
- ✅ "CLI adapter와 API adapter를 같은 추상 인터페이스" → 이미 `RunInput/RunOutput` + `run()`/`stream_run()` 패턴이 전 엔진 공통. trait 통합만 하면 됨
- ✅ "cancellation, timeout, cleanup 1급 개념 승격" → 현재 `CancelRegistry` 존재하나 codex/ollama는 cancel 미지원. 기술 부채

**Gemini (3.1-pro-preview)** — 독창적 관점, 일부 과장
- ✅ "에이전트가 파일시스템에 직접 안착" → 맞지만 RT보다는 메인 채팅/워크플로우의 장점
- ⚠️ "비대화형 hang 위험" → **이미 대응 완료** (`--permission-mode bypassPermissions`, idle timeout 600초)
- ✅ "CLI 내부 상태와 오케스트레이터 컨텍스트 불일치" → 진짜 문제. resume_token으로 부분 대응하나 근본 해결 아님. Tiering과 연결
- ⚠️ "JSON-RPC 브릿지 계층 필수불가결" → 과장. 현재 JSONL이 사실상 구조화 스트림. Codex의 "공통 이벤트 모델"이 더 현실적

### Actionable 항목 (Tiering과 연계)

| # | 항목 | 출처 | 연계 |
|---|------|------|------|
| 1 | **공통 이벤트 모델** (start/delta/tool_request/error/done) | Codex | Tiering Tier 1 휴리스틱의 입력 소스로 활용 |
| 2 | **Engine trait 통합** (CLI/SDK 듀얼 어댑터) | Codex | 현재 `run()`/`stream_run()` 패턴을 trait으로 정형화 |
| 3 | **Cancel 전 엔진 지원** | Codex | codex/ollama cancel 미지원 → graceful stop 구현 |
| 4 | **CLI-오케스트레이터 상태 동기화** | Gemini | resume_token 확장 + ContextPack 상태 추적 |

---

## 미결 사항

- Tier 1 휴리스틱의 정확도: false negative(필요한데 안 넣음) 위험
- Tier 2 도구 목록 안내 자체의 토큰 비용 (~500자)
- mode(lite/standard/full)와 tier의 관계 정리
- runtimeSlice(메인 채팅) tool-request 자동 follow-up 동작 확인 필요 (현재 threadSlice만)
- RT 벡터 맥락 공유 시 임베딩 생성 타이밍 (participant 완료 직후? 라운드 종료 후?)
- sqlite-vec Rust 크레이트 성숙도 확인 (rusqlite 확장 로딩 방식)
- rawq embed 500자 트렁케이션 완화 가능성 (모델 컨텍스트 512 토큰 ≈ 영문 ~2000자, 한글 ~1000자)
- 토픽 경계 감지: rawq embed 유사도 계산 vs 별도 경량 모델
