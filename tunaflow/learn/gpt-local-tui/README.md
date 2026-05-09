# gpt-local-tui

A standalone learning example: a 3-role coding TUI in ~400 lines of Python.
Uses **Codex CLI** (Architect + Reviewer) and **local Ollama** (Developer) to demonstrate
how splitting roles across models saves tokens.

This is **not** a tunaFlow consumer. It does not import tunaFlow. The point is the opposite:
read this code, understand the pattern, then graduate to [tunaFlow](../../../README.md) when
you outgrow a single file.

## Why

Long code generation eats tokens. Decomposition and verification do not.
So run the cheap-but-smart parts on a paid model, and run the expensive-but-dumb part locally.

| Role          | Model                  | Why                                                |
| ------------- | ---------------------- | -------------------------------------------------- |
| **Architect** | Codex CLI (`codex`)    | Decomposes the request into Developer instructions |
| **Developer** | Local 27B via Ollama   | Generates code (token-heavy, runs free)            |
| **Reviewer**  | Codex CLI (same thread)| Verifies output against criteria, decides retry    |

The Reviewer **resumes the Architect's thread**, so the criteria the Architect wrote
are already in context — no re-prompting cost.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ Session: <thread_id>                          gpt-local-tui │
├──────────────────────────┬──────────────────────────────────┤
│ Architect / Reviewer     │ Developer (local 27B)            │
│ (Codex CLI)              │                                  │
│  > decomposing...        │  > generating...                 │
│  > criteria: ...         │  > <code output>                 │
│  > verifying...          │                                  │
│  > verdict: pass         │                                  │
├──────────────────────────┴──────────────────────────────────┤
│ > type your request                                  [Send] │
└─────────────────────────────────────────────────────────────┘
```

## Prerequisites

1. **Codex CLI** (`codex-cli` 0.128+). Verify:

   ```bash
   codex --version              # codex-cli 0.128.x or newer
   codex exec --json "say hi"   # should print JSONL events ending with turn.completed
   ```

   If `codex exec` errors on auth, run `codex login` first (Codex Plus subscription assumed).

2. **Ollama** running locally with at least one chat model pulled. Verify:

   ```bash
   ollama --version
   ollama list                  # must show a chat model (not just embeddings)
   ```

   The default config points at `gemma4:e4b` (the largest local model the author had on
   hand). For a 27B-class model use `qwen2.5:32b`, `gemma2:27b`, `command-r:35b`, etc.
   Whatever you set in `config.toml` must match `ollama list` output.

3. **Python 3.10+** (3.11+ uses stdlib `tomllib`; 3.10 needs `tomli`).

## Setup

```bash
cd tunaflow/learn/gpt-local-tui
python3 -m venv .venv && source .venv/bin/activate
pip install -r requirements.txt

cp config.example.toml config.toml
# edit config.toml — at minimum, set [developer].model to a model from `ollama list`
```

## Run

```bash
python app.py
```

On first launch the header shows `Session: (new)` until you submit a request.
After the Architect call, it switches to the actual Codex thread id.

Type a coding request in the footer input, press **Ctrl+Enter** to submit.
Plain Enter inserts a newline (coding requests can be multi-line).

The left panel streams Architect output, then Reviewer output.
The right panel streams Developer output. Both panels scroll independently.

## How to read this code

The whole app is one file: [`app.py`](./app.py). Comments call out the pattern at each step.
Start there, then read [`docs/how-it-works.md`](./docs/how-it-works.md) for the *why*.

Key functions to follow in order:

- `call_codex()` — subprocess wrapper around `codex exec` / `codex exec resume`
- `call_ollama()` — thin wrapper around the `ollama` Python client
- `run_workflow()` — the 3-role loop
- `list_recent_sessions()` — picks up where Codex left off (the resume trick)

## What this is NOT

This is a **learning artifact**, not a production tool.

- Single Developer (no multi-agent debate, no quorum, no parallel branches)
- No tool-calling — Developer can't run tests against its own output
- Subprocess capture, not streaming — output appears in chunks
- Two retries max, then the user has to step in
- One workflow at a time, no history view, no save/export

If any of those limits start to bite, you have outgrown the learning example.
That is the moment to switch to [tunaFlow](../../../README.md), which productionizes the
same pattern with proper streaming, multi-agent RT, branch/adopt, ContextPack, and
persistent state.

## License

AGPL-3.0, matching the parent tunaFlow repository.
