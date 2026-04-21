# Observability audit — Phase 3 Finding 3-2

> Date: 2026-04-21
> Status: passing — no meaningful gap found in the 7 target flows

## Method

Count INFO/WARN/ERROR-level log statements per target flow across both
Rust (`eprintln!`) and frontend (`console.info|warn|error|debug`). A
flow passes when a reader can localise any failure to it **from the
logs alone** — i.e. the flow has at least one entry / mid / exit log
and at least one branch-specific error log.

## Coverage per flow

| Flow | Primary owner | Entry | Mid | Error | Verdict |
|---|---|---|---|---|---|
| **rawq** | Rust (`agents/rawq.rs`, `commands/project_tools.rs`) | `[rawq] daemon start`, `[rawq] indexing` | `[rawq] index progress` | `[rawq] ... failed` (multiple) | **pass** (16 Rust log sites) |
| **compression** | Rust (`commands/conversation_memory.rs`, helpers) | `[compress] start` | `[compress] skipped (below threshold)` | `[compress] failed` | **pass** (6 Rust sites) |
| **verdict** | Frontend (`lib/workflow/branchSync.ts`, `reviewWorkflow.ts`) | `[verdict-autodetect]` error channel | `[verdict-poll]` debug, `[verdict] already processed` | `console.warn` on parse / processing failures | **pass** (front-end-owned flow; Rust acts as DB writer only) |
| **startup / bootstrap** | Rust (`src/bootstrap/*.rs`) | `[bootstrap/env]`, `[bootstrap/db]`, `[bootstrap/services]`, `[bootstrap/window]` | per-step phase logs | phase-tagged errors (`[bootstrap/db] stale message cleanup failed`) | **pass** (7+ Rust sites, Finding 1-6 hardened this) |
| **meta-trigger** | Frontend (`lib/metaAnalysisTrigger.ts`) | none (fire-and-forget) | `[meta-tier2] analysis` | `[meta-tier2] analysis failed` | **pass** (front-end-owned; 3+ console sites) |
| **PTY** | Rust (`commands/pty.rs`, `pty_jsonl*`) | `[pty] spawn`, `[pty] paste` | `[pty] jsonl detect`, `[pty] tool steps` | `[pty] start timeout`, `[pty] process exited`, various | **pass** (9 Rust sites + FE mirrors in `ptyMessageSender.ts`) |
| **embedder (bge-m3)** | Rust (`agents/embedder.rs`) | `[embedder] bge-m3 global embedder initialized`, `ORT pool created` | `[embedder] indexing batch` | `[embedder] sync init error`, `[embedder] async download/init error` | **pass** (13 Rust sites) |

## Why the "0 Rust log sites" counts are misleading

First-pass grep for `eprintln!.*\[verdict` and `eprintln!.*\[meta`
returned 0, but both flows are **frontend-owned** — Rust is only a DB
writer on that path. Counting FE `console.*` calls tagged with
`[verdict*]` / `[meta*]` / `[meta-trigger]` surfaces 15+ sites across
`branchSync.ts`, `reviewWorkflow.ts`, `metaAnalysisTrigger.ts`, and
`MetaFloatingChat.tsx`. The actual failure-tracing surface is fine.

## Not adopted (deliberate)

- `tracing` crate. Everything runs on `eprintln!` today. The roadmap
  defers structured logging to post-beta; switching now would force
  rewriting every one of the flows above without adding a failure
  diagnosis capability we don't already have.
- Per-flow log level config. All logs are stderr-unfiltered. Users
  who need to suppress noise pipe through `grep -v`. Dev-only issue.

## How this is audited in future

Re-run these greps at the start of every beta milestone:

```bash
# Rust
grep -rnE 'eprintln!.*\[(rawq|compress|bootstrap|pty|embed)' src-tauri/src | wc -l
# Frontend
grep -rnE 'console\.(info|warn|error|debug).*\[(verdict|meta|meta-trigger)' src | wc -l
```

If any target flow drops to zero on the owner-relevant side, add
entry / mid / error logs to restore coverage before shipping.
