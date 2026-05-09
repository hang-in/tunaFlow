# How it works

The point of `gpt-local-tui` is **not** to be useful. It is to be the smallest
working version of a pattern that, once you understand it, makes you want
something bigger. That bigger thing is [tunaFlow](../../../../README.md).

This document walks the *why* behind each design choice in `app.py`. Read it
with the file open.

## 1. The token-saving thesis

In a coding workflow, three things happen:

1. **Decompose** the user's vague request into specific instructions.
2. **Generate** the actual code (long output).
3. **Verify** the output and decide whether to retry.

Steps 1 and 3 need *judgment*. They are short prompts, short responses, but
they need a model good enough to plan and to spot bugs.

Step 2 is the opposite. It needs *throughput*. The prompt is short, but the
output is long — every token counts against your budget.

If you run all three on the same paid model, step 2 dominates the bill.
The trick this demo encodes is: **run step 2 locally, free.** Local models
are slow, but they don't bill per token. A 27B-class model on consumer
hardware is fine for code generation when an Architect already told it
exactly what to build.

You only pay for short, smart steps. The long, dumb step is on your GPU.

## 2. Why CLI subprocess instead of an API SDK

Codex CLI is invoked with `subprocess.run`. That is deliberate.

Three reasons:

1. **No API key handling**. The user already logged into Codex CLI with
   `codex login` for their Codex Plus subscription. We piggy-back on that.
   If we used the OpenAI SDK, we would need a separate `OPENAI_API_KEY`
   for the same models we are already paying for through the CLI.
2. **Matches the actual workflow**. The author's day-to-day pattern is
   driving Codex CLI from inside Python scripts and tunaFlow. Demo what
   you do, not what reads cleanly in a tutorial.
3. **The subprocess interface IS the contract**. Codex CLI is a stable
   external binary. SDKs change versions; `codex exec --json` will keep
   producing JSONL events even when the underlying model changes. The
   wrapper in `call_codex()` is ~30 lines and outlives any SDK.

The cost is no streaming. `subprocess.run` waits for the process to finish
before we see output. For a learning demo this is fine; for production you
would want to read the JSONL events as they arrive (this is what tunaFlow
does — see its `claude_sdk_session` and `codex` engine wrappers).

## 3. The session-resume trick

This is the conceptual centerpiece. Open `call_codex` and notice the
`session_id` parameter.

When the Architect runs:

```
codex exec --json --skip-git-repo-check - <<< "<architect prompt>"
```

The first JSONL line is:

```json
{"type":"thread.started","thread_id":"019e0be4-95a8-7071-9ef6-0bc46fe5a3c9"}
```

We capture that `thread_id` and store it in `WorkflowState.session_id`.

When the Reviewer runs, we use:

```
codex exec resume 019e0be4-95a8-... --json --skip-git-repo-check - <<< "<reviewer prompt>"
```

Codex loads the prior conversation from `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`
and runs the Reviewer turn *with the Architect's criteria already in context*.

We did not re-send the criteria. We did not stuff them into the prompt.
We did not maintain our own history database. Codex CLI's session storage
**is** our working memory.

This is the simplest possible form of **externalized state across loops**.
Tools like Tattoo and Gemento explore this in much more depth — durable
memory, vector retrieval, plan-tree state, cross-loop adaptation. But the
core idea is here: *the agent's state lives outside the agent's prompt, and
the orchestrator's job is to point at the right state at the right time.*

Once you internalize that, multi-agent orchestration stops feeling magic.
It is just plumbing — knowing who needs what when.

## 4. Prompt design walkthrough

Three prompts in `app.py`, all short. Read them as a system, not in isolation.

### Architect: "Don't write the code"

```
Do NOT write the code yourself. Be precise about what you want produced.
```

This is the cost-control sentence. Without it, the Architect — being good
at coding — will happily produce 200 lines of Rust to demonstrate it
understood the request. Those 200 lines are billed tokens we did not need.
By forbidding it from coding, the Architect spends its capacity on
decomposition, which is where its judgment is actually valuable.

### Developer: "Be dumb"

```
- Provide complete, working code (or content) — no snippets, no placeholders.
- Code only.  No explanations unless explicitly asked.
- If instructions are unclear, do your best interpretation.
```

The Developer is told it has no judgment. It generates. If the instructions
are bad, that is the Architect's fault, and the Reviewer will catch it.

This is intentional. A local 27B model trying to "make better decisions"
than its instructions said is a bad loop — it reasons in circles. Better
to produce, fail review, retry with explicit guidance.

### Reviewer: JSON output is not a stylistic choice

```json
{
  "verdict": "pass" | "retry" | "fail",
  "reasons": ["..."],
  "retry_instructions": "..."
}
```

The Reviewer outputs JSON because the workflow code branches on the
verdict. JSON parsing means there is no ambiguity. The Architect's prompt
also ends in JSON for the same reason — `developer_instructions` and
`verification_criteria` are passed to other roles, so they have to be
extractable.

The lesson: **the prompt's output format is determined by who reads it
next.** A human reads prose; another agent reads JSON; a tool reads
function-call arguments. Decide by consumer.

## 5. Where this falls short (honestly)

This is a learning artifact. It deliberately omits things tunaFlow handles:

- **No streaming.** `subprocess.run` waits for completion. If the Developer
  takes 90 seconds, the right panel shows nothing for 90 seconds. Real
  tools stream JSONL events as they arrive.
- **No multi-agent debate.** One Developer per request. Real RT systems
  let multiple agents argue, vote, or fall back to each other.
- **No tool calling.** The Developer cannot run tests against its own
  output, cannot search the codebase, cannot read files. The Reviewer
  judges from text alone.
- **No persistent state beyond Codex sessions.** Your project conventions,
  past decisions, design docs — none of that is in the loop. The
  Architect knows nothing about your codebase.
- **Two retries, then give up.** Real systems escalate (different model,
  different decomposition, ask the user, etc.).
- **One workflow at a time.** No history view, no save/export, no
  branching off a prior turn.
- **JSON parse failures are not handled gracefully beyond surfacing the
  error.** A real system would re-prompt with stricter instructions, or
  use structured output features (`--output-schema` flag exists on
  `codex exec`, but we did not wire it up — exercise for the reader).
- **No cost tracking.** The whole point is to save tokens, but this demo
  does not show you how many you saved. Adding usage parsing from the
  `turn.completed` event would be ~20 lines.

## 6. Where to go from here

Three directions, in increasing seriousness:

### Stay shallow, learn one more pattern

- Hook the Developer to a code-execution sandbox. Let it run its own
  output and feed errors back to itself before the Reviewer sees it.
- Replace the Developer with two parallel local models (a quorum), and
  have the Reviewer pick the better output.
- Switch the Reviewer to a different paid model (e.g. Claude) to see
  whether cross-model review catches different bug classes.

### Productionize the pattern

That is what [tunaFlow](../../../../README.md) does. Read its
`docs/reference/architecture-detail.md`. The same Architect/Developer/
Reviewer split is there, but with:

- True streaming via Tauri events
- ContextPack: per-request prompt assembly with budgets (Lite/Standard/Full)
- Branch / Roundtable for multi-agent debate
- SQLite-backed durable history
- Skills, identity, memory, retrieval — externalized state at scale
- Five engines (Claude / Codex / Gemini / Ollama / LM Studio) with parity

If you keep adding features to `app.py`, you will reinvent tunaFlow
poorly. That is the lesson `learn/` is supposed to deliver: the moment
you want any of the bullet points above, switch.

### Study the empirical question

[Gemento](https://github.com/dghong/gemento) is the author's research
project on which orchestration patterns actually pay off in practice —
not on benchmarks, but on real coding work. The 3-role split this demo
shows is a starting point; Gemento measures whether it is worth the
extra round-trip vs single-model approaches.

## 7. Reading order, one more time

If you are still reading: open `app.py` and walk it in this order.

1. `load_config()` — boring but required.
2. The three prompt constants. Read them all together.
3. `call_codex()` — the subprocess wrapper. This is the load-bearing
   function in the whole demo.
4. `call_ollama()` — the easy half.
5. `parse_json_blob()` — defensive parsing because models lie about
   pure-JSON output.
6. `list_recent_sessions()` — the session resume picker. This is what
   makes the demo feel like a real tool.
7. `run_workflow()` — the actual loop. Map each step to the prompts
   you already read.
8. The Textual classes — UI plumbing. Skim, don't memorize.

The other 60% of the file is wiring.

## 8. Reality notes (Codex CLI 0.128.0)

A few details that differ from typical "agent CLI" assumptions:

- `codex -p` is **not** non-interactive prompt. `-p` is `--profile`. Use
  `codex exec [PROMPT]` for non-interactive.
- `--json` emits JSONL on stdout. Last meaningful line is
  `{"type":"turn.completed","usage":{...}}`.
- The agent's textual answer comes via
  `{"type":"item.completed","item":{"type":"agent_message","text":"..."}}`.
  Multiple `item.completed` events can occur for tool calls; we keep the
  last `agent_message` for our final text.
- Session id is `thread_id`, not `session_id`. They mean the same thing.
  `codex exec resume <UUID>` accepts that UUID.
- Sessions are stored at `~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl`.
  First line is `session_meta` with `payload.id` and `payload.timestamp`.
- `--skip-git-repo-check` lets us run outside a repo. The TUI is
  intentionally repo-agnostic.

If a future Codex CLI changes any of this, update this section first
and `call_codex` second. Reality wins.
