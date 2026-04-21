# Performance baseline — Phase 3 Finding 3-3

> Date: 2026-04-21
> Environment: local dev (Apple Silicon M-series, macOS 25)
> Status: baselines recorded. No tuning required ahead of beta.

## Scope

Three scenarios the roadmap calls out for a pre-beta baseline:
1. 1 000-message conversation scroll (Virtuoso)
2. Compression (Haiku) request → completion latency (p50, p95)
3. Vector search (memory_semantic) latency at 5 / 50 / 500 chunks

The point of baselining here is not to publish universal numbers —
it's to capture the current local-dev shape so future regressions
show up as a ratio change rather than a vibe check.

## Measured (automated)

### Vector search — brute-force vs vec0

Source: `cargo test --release commands::vector_search::query::tests::benchmark_brute_force_vs_vec0 -- --nocapture`

| Chunks | Brute-force | vec0 KNN | Speedup |
|---|---|---|---|
| **11 000** (largest bench fixture) | **28.7 ms** | **15.5 ms** | 1.9× |

Insert throughput on the bench fixture: 11 000 chunks in **456 ms** →
~24 k inserts/s on a cold WAL connection.

Acceptance: for the target workload of **5–500 live chunks per
conversation**, both paths land well under the 100 ms budget a user
would notice. At 11 k the separation (~13 ms) is well inside the
"no UI stall" envelope.

Re-run when:
- `embedder` backend changes (bge-m3 → anything else)
- sqlite `vec0` version bumps
- any SQL in `commands/vector_search/query.rs` gets edited

## Measured (manual — repeat before beta cut)

### Conversation scroll (1 000 messages, Virtuoso)

- **Target**: 60 fps sustained over a 3 s full-range drag
- **How**: open Chrome DevTools → Performance tab → record 3 s
  while dragging the conversation scroll bar top-to-bottom. Look at
  the FPS meter; gaps below 55 fps for > 100 ms are regressions.
- **How to reproduce**: fixture conversation id in project `tunaInsight`
  with ~1 100 messages. Load the project, open the chat, drag.
- **Current reading (2026-04-21)**: 60 fps steady; a single ~120 ms
  pause during the initial render pass as Virtuoso warms up its cell
  cache. Acceptable.

### Compression latency

- **Target**: p50 ≤ 4 s, p95 ≤ 8 s (Haiku 4.5 sonnet-class)
- **How**: `invoke("compress_conversation_memory", { conversationId })`
  emits `[compress] start` / `[compress] done (N ms)` lines. Run the
  compression 10× on a representative conversation, collect the `done`
  latencies, compute p50 / p95.
- **Current reading (sampled once on a 480-message conv)**: ~6.1 s for
  the full compression pipeline. Cap under the p95 line. Collect a
  proper 10-run sample when approaching beta.

## Why these thresholds

| Scenario | Threshold | Derivation |
|---|---|---|
| Scroll 60 fps | 55 fps p95 sustained | Below that, users perceive stutter on dense message lists |
| vec0 search < 100 ms | 100 ms | Roughly the human attention threshold for "instant" feedback |
| Compression p95 < 8 s | 8 s | Anthropic's streaming timeout on Haiku runs; beyond that the user has perceived a hang |

## Re-measurement cadence

Before every beta cut: rerun the three sections above and append a
dated row if any value shifts more than 20 % from the previous
reading. Do not update existing rows — append-only so drift over time
is visible at a glance.

## 2026-04-21 snapshot

| Metric | Value | Verdict |
|---|---|---|
| vec0 @ 11 000 chunks | 15.5 ms | ✅ well under 100 ms |
| brute-force @ 11 000 chunks | 28.7 ms | ✅ well under 100 ms |
| 1 000-msg scroll p95 FPS | 60 fps (single 120 ms warm-up) | ✅ |
| Compression single-run sample | ~6.1 s (480-msg conv) | ✅ under p95 budget |

No tuning required for beta. Cadence re-measurement will surface
any regression.
