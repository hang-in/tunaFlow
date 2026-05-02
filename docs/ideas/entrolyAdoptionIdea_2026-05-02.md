---
title: Entroly 도입 — token compression / multi-resolution context / federation 전략
status: idea
created_at: 2026-05-02
canonical: false
priority: P1 (사용자 환경 anchor 2 turns trigger 의 architectural 해결)
external_reference:
  repo: https://github.com/juyterman1000/entroly
  license: Apache-2.0
  compat: tunaFlow Apache-2.0 호환 ✅ (incorporate 가능, attribution 필요)
  current_version: 0.10.0 (entroly-core)
  language: Rust core + WASM bindings + Python wrapper
  features:
    - 70~95% token savings (live API 검증)
    - <10ms latency
    - 834 tests, 100% accuracy retention (n=100 verified)
    - Local-only (zero cloud / zero data exfiltration)
related:
  - docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md  # T11/T12 의 후속 architectural 격상
  - docs/ideas/bkitReferenceAdoptionIdea_2026-04-29.md  # Idea A1 (Priority Preserve) 의 정량 구현
  - docs/ideas/threadlensSessionManagementIdea_2026-04-30.md  # T1 sidebar search 와 axis 다름
  - src-tauri/src/commands/agents_helpers/send_common/persistence.rs  # T11/T12 cli mode drop 분기
  - src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs  # ContextPack assemble
trigger:
  reported_at: 2026-05-02
  reporter: 사용자 (d9ng)
  motivation: "tunaFlow 고도화 기여 가능 영역 확인"
---

# Entroly 도입 idea

## 0. Context

`/Users/d9ng/privateProject/_research/_util/entroly` (Apache-2.0, 0.10.0) — **AI token compression engine** (Rust core + WASM + Python). `conversation_pruner` 모듈이 tunaFlow 의 ContextPack 영역과 정확히 매칭. 2026-04-30 사용자 환경에서 발견된 *anchor 2 turns content paid API trigger* (T9~T12 누적 fix) 의 **architectural 해결** 가능성.

## 1. License + Attribution

Apache-2.0 = tunaFlow 호환. incorporate 시:
- Cargo dependency: `entroly-core = "0.10"` (crates.io 미등록 가능성, GitHub source 또는 vendor)
- file header: `// Pattern adapted from juyterman1000/entroly (Apache-2.0)`
- NOTICE 한 줄 (대규모 통합 시): `This product uses entroly-core by juyterman1000 (Apache-2.0)`

## 2. Entroly 핵심 모듈 분석

`entroly-core/src/` 25 모듈 중 tunaFlow 매칭 가능 영역:

| 모듈 | 명세 | tunaFlow 매칭 |
|---|---|---|
| **`conversation_pruner.rs`** | **Multi-Resolution Causal DAG Pruning** (4 levels L0~L3, MCKP solver, KKT dual bisection O(120N)) | **ContextPack assemble 직접 대체 가능** |
| `bm25.rs` | BM25 keyword scoring | tunaFlow FTS5 + retrieval 보강 |
| `dedup.rs` + `semantic_dedup.rs` | content + semantic deduplication | bkit Idea A2 (fingerprint dedup) 정량 구현 |
| `entropy.rs` | Kolmogorov information density | retrieval ranking 보강 |
| `knapsack.rs` + `knapsack_sds.rs` | submodular knapsack ((1-1/e) approximation) | budget allocation 수학적 보증 |
| `lsh.rs` | Locality Sensitive Hashing | similarity dedup |
| `prism.rs` | **PRISM RL learning loop** (token-negative 보장: learning ≤ 5% × savings) | tunaFlow 의 self-improvement axis (현 unsupported) |
| `cognitive_bus.rs` + `cogops.rs` | cognitive operations DAG | agent reasoning 구조화 |
| `archetype.rs` + `causal.rs` + `channel.rs` | DAG / causal structure | RT / Branch 의 DAG 표현 |
| `depgraph.rs` | dependency graph | rawq + crg 와 결합 |
| `fragment.rs` + `hierarchical.rs` + `skeleton.rs` | multi-resolution representation | code/docs 계층적 indexing |
| `query.rs` + `query_persona.rs` | query engine | retrieval pipeline |
| `resonance.rs` | resonance-based scoring | message importance ranking |
| `sast.rs` | static analysis | code understanding |
| `nkbe.rs` | Normalized K-Best Encoding | encoding optimization |
| `guardrails.rs` | safety guards | tunaFlow guardrail 보강 |
| `cache.rs` | caching layer | ContextPack cache |
| `utilization.rs` | token budget tracking | RuntimeStatusBar context indicator (bkit Idea B2 정량) |

## 3. tunaFlow ContextPack 의 현 상태 vs Entroly 의 패턴

### 3.1 현 tunaFlow ContextPack (T12 적용 후)

```rust
// persistence.rs T11/T12: cli mode + fresh session 시 binary drop
data.compressed_memory = None;
data.plan_section = None;
data.plan_document = None;
data.artifacts_section = None;
data.findings_section = None;
data.retrieval_chunks.clear();
data.cross_session_data.clear();
data.current_messages.clear();   // ← anchor 2 turns 도 통째 drop
data.parent_messages.clear();
```

**한계**: binary drop = *모두 keep 또는 모두 drop*. agent 가 정보 필요하면 *0 인지 → tool-request 호출 → re-fetch* (round-trip 추가).

### 3.2 Entroly conversation_pruner 패턴

```
4 Resolution Levels:
- L0: 전체 verbatim (0% savings, 100% info)
- L1: Structural skeleton (~70% savings, ~85% info)
- L2: One-line semantic digest (~92% savings, ~35% info)
- L3: 64-bit SimHash fingerprint (~99% savings, ~5% info)

Information Value Scoring:
  w(v) = α·forward_value + β·ref_density + γ·recency + δ·kind_shield

Progressive Compression:
  < 70% util  → no compression
  70-80%      → tool results → L1
  80-90%      → + thinking blocks → L2
  90-95%      → + old tool results → L3
  > 95%       → + old assistant messages → L1
```

**우월점**:
- *graceful degradation*: 정보 완전 손실 X — L3 fingerprint 도 retrieval key 역할
- *수학적 보증*: (1-1/e) approximation (knapsack)
- *DAG coherence*: dependency 깨지지 않음
- *kind_shield*: user message 0.95 / system 1.0 / thinking 0.10 → 자동 priority preserve (bkit Idea A1)

## 4. 적용 idea (5 영역, P0~P3)

### Idea E1 — `conversation_pruner` 직접 integration [P1]

**Target**: tunaFlow 의 ContextPack assemble 을 entroly-core::conversation_pruner 호출로 대체.

**Spec**:
- `Cargo.toml`: `entroly-core = { git = "https://github.com/juyterman1000/entroly", branch = "main" }` 또는 vendor
- `prompt_assembly.rs::assemble_prompt` 안에서 entroly_core::conversation_pruner::prune 호출:
  ```rust
  use entroly_core::conversation_pruner::{prune, BlockKind, ResolutionLevel};
  
  let blocks = build_context_blocks(&data);  // ContextPack 의 모든 layer → blocks
  let pruned = prune(blocks, budget_chars, current_utilization)?;
  // pruned 가 [L0, L1, L2, L3] 중 적절한 level 로 압축된 prompt 반환
  ```
- T11/T12 의 binary drop 분기 제거 (entroly 가 *more 정밀하게* 처리)
- ContextPack 의 conversational + structured + retrieval layer 모두 entroly 통과

**효과 기대**:
- paid API trigger 회피 + 정보 보존 (L0~L3 적절 mix)
- prompt_chars 자동 budget 안 fit (knapsack guarantee)
- 사용자 환경 anchor 2 turns trigger 자연 해결 (L1 skeleton 으로 압축)

**변경 영역**:
- `src-tauri/Cargo.toml` (entroly-core dependency)
- `prompt_assembly.rs::assemble_prompt` (refactor)
- `persistence.rs` T11/T12 분기 제거 (entroly 가 대체)
- 약 200~400 LoC (refactor) + 100 LoC (adapter)

**위험**:
- entroly-core 가 PyO3 + libpython link — Tauri 환경에서 link 가능성 검증 필요 (또는 `extension-module` feature off)
- entroly-core API 안정성 (0.10.0, breaking change 가능)
- DAG coherence 유지 — tunaFlow 의 plan/artifact reference 가 entroly 의 dependency model 과 align 필요

### Idea E2 — Entroly proxy 모드 활용 (가장 단순) [P0 가능성]

**Target**: tunaFlow 의 모든 LLM API 호출을 entroly proxy 통과.

**Spec**:
- `entroly proxy --port 9377` 백그라운드 실행 (sidecar 또는 사용자 manual)
- tunaFlow 의 claude/codex/gemini 호출 시 `ANTHROPIC_BASE_URL=http://localhost:9377` 같이 redirect
- entroly 가 자동으로 prompt 압축 + response distillation
- tunaFlow 코드 변경 0 (sidecar 추가만)

**효과 기대**:
- 즉시 70~95% token savings
- 사용자 환경 paid API trigger 자연 회피 (compressed prompt)
- 다른 엔진 (codex / gemini / ollama / lmstudio) 에도 일관 적용

**변경 영역**:
- `src-tauri/binaries/entroly-*` (rawq 와 비슷한 sidecar binary)
- `bootstrap/services.rs` (entroly proxy spawn)
- engine 별 base_url 분기 (env var 또는 config)
- 약 50~100 LoC (sidecar wiring)

**위험**:
- proxy 가 Anthropic API 의 streaming + tool use 모두 정상 통과하는지 검증 필요
- claude `-p` 의 stream-json 응답이 entroly 통과 후 깨지지 않는지
- localhost proxy 가 차단된 사용자 환경 (firewall) → fallback 필요
- entroly daemon 추가 자원 (~10ms latency, RAM 사용량)

**uncertainty**: entroly proxy 가 Anthropic 의 specific 응답 (rate_limit_event, stream-json result event 등) 처리 여부 미확인. 검증 필요.

### Idea E3 — `prism` learning loop 채택 [P3, 별 product 영역]

**Target**: tunaFlow 의 ContextPack 정책을 *self-improving* 으로 (token-negative 보장).

**Spec**:
- entroly_core::prism::Learning 모듈을 tunaFlow 의 trace 데이터와 결합
- 매 send 의 *실제 응답 quality* (사용자 만족 / agent rework / 등) 를 reward signal 로
- ContextPack mode (Lite/Standard/Full) 의 layer 선택을 RL 으로 최적화
- "Day 1 70% savings → Day 30 85% → Day 90 90%" 패턴 (entroly README)

**효과 기대**:
- self-improvement 가 *learning ≤ 5% × savings* 보장
- federation opt-in 시 다른 사용자의 학습도 absorb (anonymous)

**변경 영역**:
- 큰 변경. 별 plan 가치. v0.2.0 영역
- DB schema 추가 (RL state)
- frontend 가시화 (학습 진행도)

**위험**:
- federation 의 privacy / 보안 검증 (anonymous + noise-protected 설계지만 audit 필요)
- "swarm dreaming" pattern — tunaFlow 의 single-user 가정과 architecture 일치 검토

### Idea E4 — Response distillation [P2]

**Target**: LLM 응답의 ~40% filler 제거.

**Spec**:
- entroly 의 response distillation (lite/full/ultra 3 levels)
- claude 응답 stream 을 tunaFlow 가 받을 때 distillation 적용 후 frontend 전달
- code blocks 미터치 (entroly 의 안전 정책)

**효과**:
- 출력 token cost 절감 (사용자 paid API quota 보존)
- UI 응답 length 짧아져 readability ↑

**변경 영역**:
- `claude.rs` 의 `on_chunk` 콜백 안에서 distillation 적용
- 사용자 settings 토글 (lite/full/ultra/off)
- 약 100 LoC + dependency

### Idea E5 — `dedup` + `semantic_dedup` 활용 (bkit A2 정량 구현) [P3]

**Target**: bkit Idea A2 의 SHA-256 fingerprint dedup 을 entroly 의 LSH + semantic_dedup 으로 정교화.

**Spec**:
- ContextPack assemble 시 entroly_core::dedup 으로 chunk-level dedup
- `lsh.rs` 의 SimHash 활용 — 같은 의미의 다른 표현 (paraphrase) 도 dedup
- bkit A2 의 fingerprint 보다 *의미적 정밀도* 높음

**효과**:
- 같은 conversation 의 반복 패턴 inject 회피 (큰 history 보유 사용자)
- bkit Idea A2 의 P3 → P1 격상 + entroly 활용으로 구현

## 5. 통합 시 안전 가드 / 검증 plan

### 5.1 INV (cross-cutting)

| ID | 내용 |
|---|---|
| INV-E-1 | macOS / Windows / Linux 모든 OS 동일 동작 — entroly_core Rust 라 cross-platform 안전 |
| INV-E-2 | Lite mode 호환 — entroly 도입해도 사용자 mode = Lite 선택 시 기존 동작 유지 (혹시 entroly bug 있어도 graceful) |
| INV-E-3 | sdk-url path 미영향 (cli mode 한정 도입) |
| INV-E-4 | 다른 엔진 (codex/gemini/ollama/lmstudio) 영향 0 첫 단계 — claude path 만 |
| INV-E-5 | "100% accuracy retention (n=100)" — entroly 자체 테스트 신뢰. 그러나 tunaFlow 환경에서 100 sample 별도 검증 |

### 5.2 단계적 roll-out

**Phase 1 (P1, v0.1.5-beta 후 첫 cycle)**:
- E2 (Entroly proxy 모드) 검증 — 변경 작고 fast track 가능. 사용자 paid API trigger 즉시 회피
- 실패 시 (proxy 호환성 issue) E1 (direct integration) 로 fallback

**Phase 2 (P2, v0.1.6-beta)**:
- E1 (conversation_pruner direct integration) — T11/T12 binary drop 대체
- E4 (response distillation) — 출력 token 절감

**Phase 3 (P3, v0.2.0+)**:
- E3 (PRISM RL) + E5 (semantic dedup) — self-improving + 정밀 dedup

### 5.3 검증 메트릭

| Metric | 현 (T12 적용) | Entroly 적용 후 목표 |
|---|---|---|
| paid API trigger | 회피 (binary drop) | 회피 (graceful) |
| prompt_chars (avg) | ~15K | ~10K (compressed) 또는 ~25K (Standard 정상화) |
| 응답 quality | history 0 (사용자 fact) | history 보존 (L0~L3 mix) |
| agent tool-request 호출 횟수 | 많음 (history 잃어 fetch) | 적음 (entroly 가 보존) |
| token cost / send | 사용자 측정 X | 70~95% 절감 |

## 6. tunaFlow + Entroly 의 differentiation 강화

| 영역 | tunaFlow | + Entroly |
|---|---|---|
| Multi-agent orchestration | ✅ (claude/codex/gemini/ollama/lmstudio 5종) | 그대로 + 모두 70~95% 절감 |
| ContextPack | binary drop (T11/T12) | multi-resolution graceful |
| RT / Branch | DAG (manual structure) | + entroly causal DAG (자동 구조화) |
| MCP server | ✅ (tunaflow-mcp) | + entroly MCP (#1 on MCP Market) 결합 가능 |
| Local-only | ✅ | ✅ (entroly 도 local-only) |
| Self-improvement | ❌ | + PRISM RL opt-in |

**Strategic value**: tunaFlow 가 *"agent-first AOC + token-efficient"* differentiation. 외부 사용자 cost 부담 ↓ → adoption rate ↑.

## 7. Cross-link to existing ideas

- `bkitReferenceAdoptionIdea_2026-04-29.md` Idea A1 (Priority Preserve) — entroly `kind_shield` 가 정량 구현
- `bkitReferenceAdoptionIdea_2026-04-29.md` Idea A2 (Fingerprint Dedup) — entroly `dedup` + `semantic_dedup` + `lsh` 가 정량 구현. 본 idea 의 E5 가 그 격상
- `claudeTransportFlipHardeningPlan_2026-04-29.md` T11/T12 — binary drop. 본 idea 의 E1 이 그 *architectural 격상*
- `threadlensSessionManagementIdea_2026-04-30.md` — session management UX axis. 본 idea 와 axis 다름 but complementary

## 8. uncertainty / 검증 필요

추측 X — fact 확보 필요한 영역:

| 항목 | 검증 방법 |
|---|---|
| entroly-core 의 Tauri 환경 link 가능성 | `cargo add entroly-core` 후 cargo check |
| entroly proxy 의 Anthropic stream-json 호환 | claude `-p --output-format stream-json` 통과 시 result event / rate_limit_event 정상 forward 여부 sample 호출 검증 |
| L3 SimHash fingerprint 의 retrieval key 동작 | 같은 conversation 에 재인용 시 entroly 가 자동 expand 하는지 |
| federation 의 privacy 보장 수준 | entroly 의 noise-protected 설계 audit |
| accuracy retention 100% (n=100) | tunaFlow 환경에서 50~100 sample 별도 검증 (claude/codex/gemini 응답 비교) |
| `prism.rs` 의 RL state 가 tunaFlow trace 와 호환 | API surface 일치 검증 |

## 9. 다음 step

본 idea 가 plan 으로 격상될 조건:
- v0.1.4-beta release publish 완료 (현 cycle 마무리)
- entroly proxy 또는 direct integration 의 PoC (POC) — 사용자 환경에서 paid API trigger 회피 확정
- 또는 외부 사용자 cost 보고 누적 시점

조건 충족 시 plan 이름 후보:
- `entrolyProxyIntegrationPlan_<date>.md` (E2 단독, P1, fast track)
- `entrolyDirectIntegrationPlan_<date>.md` (E1, P2, refactor)
- `prismRLLearningLoopPlan_<date>.md` (E3, P3, 별 영역)

본 idea 는 SSOT — 적용 결정 시 cross-link.

## 10. 사용자 cycle position

- 현재 (2026-05-02): v0.1.4-beta cycle 마무리 + dev mode notification crash hotfix 완료 (PR #251)
- 본 idea 는 **publish 후 첫 cycle 의 PoC** 영역 — E2 (proxy) 가 가장 빠른 검증 path
- v0.1.5-beta 의 핵심 가치 후보 — 사용자 환경 paid API trigger 의 *체계적 해결* + 70~95% cost savings
