---
title: Developer 핸드오프 — tunaflow/learn/gpt-local-tui/ standalone learning example
plan: (this file is the spec — single SSOT)
created_at: 2026-05-09
---

# Handoff: `tunaflow/learn/gpt-local-tui/`

**For**: Claude Code (tunaFlow architect session)
**From**: Claude Desktop (planning session, 2026-05-09)
**Status**: Implementation spec, ready to build

---

## 0. Context

This is a **standalone learning example** that lives inside the tunaFlow repo at `tunaflow/learn/gpt-local-tui/`. It is **not** a tunaFlow consumer — it does **not** import any tunaFlow code. The point is to show, in ~400 lines of single-file Python, the same Architect/Developer/Reviewer pattern that tunaFlow productionizes. After reading this code, a user should naturally understand why tunaFlow exists.

The folder name `learn/` (not `examples/`) is intentional. `examples/` would imply "example usage of tunaFlow." `learn/` correctly signals "build your own minimal version, then graduate to tunaFlow when you need the real thing."

## 1. What this demo does

A TUI application where the user types a coding request, and three roles handle it:

| Role | Model | Job |
|---|---|---|
| **Architect** | Codex CLI (`codex -p` mode) | Breaks down the request, defines what Developer should produce, writes verification criteria |
| **Developer** | Local 27B (via Ollama) | Generates the actual code/content (token-heavy work) |
| **Reviewer** | Codex CLI (same session as Architect) | Verifies Developer's output against criteria, decides pass/retry |

**The token-saving thesis**: the expensive part (long code generation) runs locally for free. The smart part (decomposition + verification) uses paid LLM but only with short prompts. Same workflow Codex Plus subscription supports easily.

## 2. Folder structure

```
tunaflow/learn/gpt-local-tui/
├── README.md                  # User-facing setup + run guide
├── app.py                     # Main TUI application (~400 lines target)
├── requirements.txt           # textual, ollama (python client)
├── config.example.toml        # User copies to config.toml
└── docs/
    └── how-it-works.md        # Walkthrough of the architecture decisions
```

**Single file (`app.py`) for the entire application is intentional.** Splitting into modules makes the learning artifact harder to read. One file, well-commented, with clear function-level separation.

## 3. Dependencies

```txt
# requirements.txt
textual>=0.85.0       # TUI framework
ollama>=0.4.0         # local LLM client
tomli>=2.0.0          # config loading (Python <3.11 compat); use tomllib if 3.11+
```

**No OpenAI SDK, no anthropic SDK.** Codex is invoked via subprocess. This is intentional — it shows users that orchestration over CLI tools is a valid pattern, and matches what the user (dghong) actually does in practice.

**No tunaFlow import.** The whole point is that this works standalone.

## 4. External tool requirements

User must have, before running:

1. **Codex CLI** installed and logged in (Codex Plus subscription assumed).
   - Verify with: `codex --version` should work
   - Verify auth: `codex -p "say hi"` returns a response
2. **Ollama** running locally with a 27B-class model pulled.
   - Default config assumes: `gemma3:27b` (since Gemma 4 27B may not be on Ollama at write time; verify before defaulting)
   - Alternative: `qwen2.5:32b` (~20GB), `gemma2:27b` (~16GB)
   - Verify: `ollama list` shows the model
3. **Python 3.11+** (for tomllib; if you must support 3.10, use `tomli`)

README must list these as prerequisites with verification commands.

## 5. Configuration

```toml
# config.example.toml

[architect]
# Codex CLI command. Adjust if user's binary path differs.
command = "codex"
# -p mode flag for non-interactive prompt
prompt_flag = "-p"
# How session resume works (verify exact flag with `codex --help`)
resume_flag = "--resume"

[developer]
# Ollama model name — must match `ollama list` output
model = "gemma3:27b"
# Ollama host (default local)
host = "http://localhost:11434"
# Generation parameters
temperature = 0.3
num_ctx = 8192

[ui]
# Theme: "dark" or "light"
theme = "dark"
# Show timing info per role
show_timing = true
```

User copies `config.example.toml` to `config.toml` and edits. App refuses to start if `config.toml` missing (with clear message pointing to the example).

## 6. Codex CLI subprocess interface

**This is the most critical implementation detail.** Verify the exact flags by running `codex --help` before coding. Spec below is the *intended behavior*; adjust to actual CLI flags.

### Architect call (first turn)

```python
# Conceptual — adapt to actual codex CLI signature
result = subprocess.run(
    ["codex", "-p", architect_prompt],
    capture_output=True, text=True, timeout=120
)
# Capture session_id from stdout (format depends on codex CLI output)
session_id = parse_session_id(result.stdout)
architect_response = result.stdout  # or parsed JSON if codex outputs JSON
```

### Reviewer call (resume same session)

```python
result = subprocess.run(
    ["codex", "--resume", session_id, "-p", reviewer_prompt],
    capture_output=True, text=True, timeout=120
)
```

**Key behaviors needed**:
- Capture `session_id` from first call (search for it in stdout, or use whatever mechanism codex CLI provides)
- Pass session_id to subsequent calls so Architect's decomposition context is available to Reviewer
- Handle timeout gracefully (show error in TUI, allow retry)
- If codex CLI doesn't output session_id reliably, fall back to file-based session tracking (codex stores sessions on disk; find the most recent one)

**If codex CLI's actual interface differs significantly from this**: document the actual interface in `docs/how-it-works.md` and adapt. The principle is what matters: **same session for Architect+Reviewer, different role prompts**.

## 7. Prompt templates

Three prompts hard-coded in `app.py`. Show them clearly in code comments — these are part of what users learn.

### Architect prompt

```
ROLE: Architect

You are the Architect in a 3-role workflow. A local 27B model will do the actual work; you decompose and verify.

USER REQUEST:
{user_input}

YOUR JOB:
1. Break this request into clear, specific instructions for a Developer.
2. Define verification criteria — what should the output look like to be acceptable?
3. Output ONLY in this JSON format:

{
  "developer_instructions": "Specific, self-contained task description for the Developer.",
  "verification_criteria": [
    "Criterion 1 (objective, checkable)",
    "Criterion 2",
    "..."
  ]
}

Do not write the code yourself. The Developer will. Be precise about what you want.
```

### Developer prompt (sent to local Ollama)

```
ROLE: Developer

You are a coding assistant. Your Architect has given you specific instructions. Follow them exactly.

INSTRUCTIONS:
{developer_instructions}

OUTPUT REQUIREMENTS:
- Provide complete, working code (or content) — not snippets.
- No explanations unless asked. Code only.
- If the instructions are unclear, do your best interpretation.
```

### Reviewer prompt (resumes Architect session)

```
ROLE: Reviewer (same session as Architect — you remember what you decomposed)

The Developer has produced the following output:

```
{developer_output}
```

YOUR JOB:
Verify the output against the criteria you defined earlier.

Output ONLY in this JSON format:

{
  "verdict": "pass" | "retry" | "fail",
  "reasons": [
    "Specific reason 1",
    "..."
  ],
  "retry_instructions": "If verdict is retry, specific guidance for next attempt. Otherwise null."
}
```

**Why these prompts matter for learning**: they show readers that orchestration is mostly about *"who knows what at which step"*, not magic. Comments in code should call attention to:
- Architect doesn't write code (saves tokens)
- Developer doesn't make decisions (it's the cheapest model, just produces)
- Reviewer leverages session resume (Architect's context is free to access)

## 8. UI layout (Textual)

```
┌─────────────────────────────────────────────────────────────┐
│ Session: <session_id> [New] [Resume...]      gpt-local-tui  │ ← header
├──────────────────────────┬──────────────────────────────────┤
│                          │                                  │
│  Architect / Reviewer    │   Developer (local 27B)          │
│  (Codex CLI)             │                                  │
│                          │                                  │
│  > Decomposing...        │   > Generating...                │
│  > Instructions: ...     │   > [code output streaming]      │
│  > Verifying...          │                                  │
│  > Verdict: pass         │                                  │
│                          │                                  │
├──────────────────────────┴──────────────────────────────────┤
│ > Type your request here...                          [Send] │ ← input
└─────────────────────────────────────────────────────────────┘
```

**Key UI behaviors**:
- Left and right panels scroll independently
- Each role's output appears in real-time (stream from subprocess where possible)
- Header shows current session ID; clicking "Resume..." opens a list of recent codex sessions
- Footer input has Ctrl+Enter to submit (Enter alone for newline — coding requests can be multi-line)
- Timing info (if `show_timing = true`): each role's wall time displayed at end of its block

**Don't over-engineer**:
- No syntax highlighting in MVP (Textual supports it but adds complexity)
- No file save/export in MVP (user can copy-paste from terminal)
- No multi-tab sessions (one workflow at a time)

## 9. Workflow logic (the loop)

```python
async def run_workflow(user_input: str, session_id: str | None = None):
    # 1. Architect: decompose
    arch_response, session_id = await call_codex(
        prompt=ARCHITECT_PROMPT.format(user_input=user_input),
        session_id=session_id  # None = new session
    )
    instructions, criteria = parse_architect_json(arch_response)
    update_left_panel("Architect: " + format_instructions(instructions, criteria))

    # 2. Developer: generate
    dev_output = await call_ollama(
        prompt=DEVELOPER_PROMPT.format(developer_instructions=instructions)
    )
    update_right_panel("Developer:\n" + dev_output)

    # 3. Reviewer: verify (same codex session)
    rev_response, _ = await call_codex(
        prompt=REVIEWER_PROMPT.format(developer_output=dev_output),
        session_id=session_id
    )
    verdict = parse_reviewer_json(rev_response)
    update_left_panel("Reviewer: " + format_verdict(verdict))

    # 4. Decide next step
    if verdict.verdict == "pass":
        return  # done
    elif verdict.verdict == "retry":
        # Re-run Developer with retry instructions, keep same session
        # Limit retries to 2 to avoid infinite loops
        ...
    else:  # fail
        update_left_panel("Workflow failed. User intervention needed.")
```

**Retry policy**: max 2 retries. If still failing, show user the verdict and let them decide (edit prompt and resubmit, or accept partial result).

## 10. Resume feature (the Tattoo equivalent)

**This is the conceptual highlight of the demo**. Comments must call this out clearly.

When user clicks "Resume...", show a list of recent codex sessions (last 5–10). User picks one. New requests in that resumed session inherit all prior decomposition context — Architect "remembers" what it was thinking about.

```python
def list_recent_sessions() -> list[Session]:
    # Codex CLI typically stores sessions in ~/.codex/sessions/ or similar
    # Verify actual path and parse session metadata
    # Return list of (session_id, first_user_prompt, timestamp) for UI display
    ...
```

**Why this matters for the learning**: this is the simplest possible demonstration of *externalized state*. The user doesn't have to implement a database or vector store — Codex CLI's own session storage IS the working memory. Comments should explicitly point out: *"this is the simplest form of what tools like Gemento explore in depth — externalized working memory across loops."*

## 11. Error handling (minimum bar)

| Failure mode | Behavior |
|---|---|
| Codex CLI not installed | App refuses to start, prints install hint |
| Ollama not running | App starts but shows clear error on first request, with hint to run `ollama serve` |
| Model not pulled | Clear error: `Model 'X' not found. Run: ollama pull X` |
| Codex auth expired | Show CLI's own error message in left panel; user re-auths and retries |
| Subprocess timeout (>120s) | Show timeout error in panel; allow retry |
| JSON parse failure (Architect/Reviewer) | Show raw response + parse error; user can debug their prompt |

**No silent failures, no retries-on-network-error masquerading as success.** Every error is visible to the user.

## 12. README content (target ~150 lines)

Sections (in order):

1. **What this is** — 3 sentences. "Standalone learning example, GPT + local LLM, token-saving via role split."
2. **Why** — 1 paragraph. The token-saving thesis.
3. **Architecture** — ASCII diagram of the 3-role flow. Reuse Section 1's table.
4. **Prerequisites** — Codex CLI, Ollama, Python. With verification commands.
5. **Setup** — `pip install -r requirements.txt`, copy config, edit model name.
6. **Run** — `python app.py`. What to expect on first launch.
7. **How to read this code** — Pointer to `docs/how-it-works.md`. List of key functions: `call_codex`, `call_ollama`, `run_workflow`.
8. **What this is NOT** — Explicit: "This is a learning artifact, not a production tool. For real work see tunaFlow." Link to tunaFlow main README.
9. **License** — match tunaFlow main repo (AGPL-3.0).

**Tone**: matter-of-fact, no marketing language, no "powerful" or "revolutionary." Treat the reader as someone who can already code.

## 13. `docs/how-it-works.md` content (target ~250 lines)

This is where the *learning* lives. Sections:

1. **The token-saving thesis** — Why split roles by model.
2. **Why CLI subprocess instead of API SDK** — Practical reasons (no API key needed for Codex Plus users, matches actual workflow).
3. **The session-resume trick** — Why Architect and Reviewer share a session. How this is the simplest form of externalized state.
4. **Prompt design walkthrough** — Why Architect's prompt forbids code-writing, why Developer's prompt is dumb-on-purpose, why Reviewer's prompt outputs JSON.
5. **Where this falls short** — Honest list:
   - Single Developer (no multi-agent debate like tunaFlow's RT mode)
   - No persistent state beyond Codex session (no Tattoo)
   - No tool-calling (Developer can't run tests)
   - No streaming (subprocess captures all output before showing)
   - Synchronous-ish (one workflow at a time)
6. **Where to go from here** — Pointers:
   - tunaFlow for production-grade orchestration
   - Gemento for empirical study of which patterns actually help
   - LangGraph / CrewAI for framework-based approaches

**This document is the reason `learn/` exists**. The code is illustrative; the docs are the actual teaching.

## 14. Implementation order (suggested)

1. README skeleton (so user knows what they're building toward)
2. Config loading + validation
3. Codex CLI subprocess wrapper with session ID extraction (verify with manual tests first!)
4. Ollama client wrapper
5. Workflow logic (no UI, just print to stdout) — verify the 3-role pipeline works end-to-end
6. Textual UI with stub data
7. Wire UI to workflow
8. Resume feature
9. Error handling polish
10. `docs/how-it-works.md`

**Step 3 is the highest-risk step.** Do it manually first with `subprocess.run(["codex", "-p", "test"])` and inspect the output. Build the wrapper around the actual behavior, not the spec'd behavior. If codex CLI behaves differently than this document assumes, **update the document and proceed** rather than forcing the spec.

## 15. Validation checklist (before merging)

- [ ] Fresh clone → `pip install` → copy config → edit model name → `python app.py` works without errors
- [ ] User can submit a coding request and see all 3 roles execute
- [ ] Codex session ID visible in header
- [ ] Resume button lists recent sessions, selecting one preserves context for follow-up requests
- [ ] Ollama not running → clear error, app doesn't crash
- [ ] Codex CLI not installed → clear error before app starts
- [ ] Workflow with intentionally-bad request → Reviewer catches it, retry logic engages
- [ ] `docs/how-it-works.md` reads coherently as a standalone learning piece

## 16. Out of scope for this iteration

Don't build:
- File save/export
- Workflow history/replay
- Multiple Developer models running in parallel
- Tool calling (file ops, test runs) inside workflow
- Web UI
- Anything that imports from tunaFlow's main package
- Anything that requires extending tunaFlow itself

If any of the above feels needed, it's a sign that user has graduated past `learn/` and should be using tunaFlow proper.

## 17. Notes for the implementing agent

- This spec was written by Claude Desktop (planning) for Claude Code (implementing). Treat it as a starting point, not a contract. If reality diverges (especially around Codex CLI's actual interface), reality wins.
- Keep `app.py` under 500 lines if possible. Past that, it's no longer a single-file demo.
- Comments in code should be teaching comments, not just function descriptions. A reader scrolling through `app.py` should learn the pattern, not just see what each function does.
- When in doubt, simpler. The point is to be the *minimum example that works*, not the most feature-complete.

---

**End of handoff. Ready for Claude Code to begin implementation.**
