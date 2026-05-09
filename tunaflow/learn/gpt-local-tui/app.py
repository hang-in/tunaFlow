"""gpt-local-tui — a 3-role coding TUI in one file.

Read top-to-bottom.  The point is the *pattern*, not the framework.

Roles: Architect (Codex CLI) decomposes -> Developer (local Ollama) generates ->
Reviewer (Codex CLI, *resuming Architect's thread*) decides pass/retry.
Token-saving thesis: see `docs/how-it-works.md`.
"""
from __future__ import annotations

import asyncio
import json
import re
import subprocess
import sys
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Callable

import ollama
from textual.app import App, ComposeResult
from textual.binding import Binding
from textual.containers import Horizontal, Vertical
from textual.screen import ModalScreen
from textual.widgets import Footer, Header, Label, ListItem, ListView, RichLog, TextArea

# tomllib is stdlib on 3.11+; fall back to the `tomli` PyPI package on 3.10.
try:
    import tomllib  # type: ignore[import-not-found]
except ModuleNotFoundError:  # pragma: no cover - 3.10 fallback only
    import tomli as tomllib  # type: ignore[import-not-found, no-redef]

CONFIG_PATH, EXAMPLE_PATH = Path(__file__).parent / "config.toml", Path(__file__).parent / "config.example.toml"
CODEX_SESSIONS_DIR = Path.home() / ".codex" / "sessions"


# ===== config =================================================================
@dataclass
class Config:
    architect_cmd: str
    architect_timeout: int
    developer_model: str
    developer_host: str
    developer_temperature: float
    developer_num_ctx: int
    developer_timeout: int
    show_timing: bool
    max_retries: int


def load_config() -> Config:
    """Load config.toml.  Bail early with a clear message if it is missing."""
    if not CONFIG_PATH.exists():
        sys.exit(
            f"Missing {CONFIG_PATH.name}.\n"
            f"Copy {EXAMPLE_PATH.name} to {CONFIG_PATH.name} and edit it first."
        )
    raw = tomllib.loads(CONFIG_PATH.read_text(encoding="utf-8"))
    a, d, u = raw.get("architect", {}), raw.get("developer", {}), raw.get("ui", {})
    return Config(
        architect_cmd=a.get("command", "codex"),
        architect_timeout=int(a.get("timeout_seconds", 180)),
        developer_model=d["model"],  # required — fail loud if missing
        developer_host=d.get("host", "http://localhost:11434"),
        developer_temperature=float(d.get("temperature", 0.3)),
        developer_num_ctx=int(d.get("num_ctx", 8192)),
        developer_timeout=int(d.get("timeout_seconds", 300)),
        show_timing=bool(u.get("show_timing", True)),
        max_retries=int(u.get("max_retries", 2)),
    )


# ===== prompts ================================================================
# Short on purpose.  Architect forbids itself from writing code (cost-saving).
# Developer is told to be dumb-but-productive.  Reviewer emits JSON so the
# workflow can branch on it.
ARCHITECT_PROMPT = """ROLE: Architect
You are the Architect in a 3-role workflow.  A local 27B model does the coding;
you decompose and verify.

USER REQUEST:
{user_input}

YOUR JOB:
1. Break the request into clear instructions for a Developer.
2. Define objective verification criteria.
3. Output ONLY valid JSON, no prose around it:
{{
  "developer_instructions": "Specific, self-contained task for the Developer.",
  "verification_criteria": ["Criterion 1 (objective and checkable)", "Criterion 2"]
}}
Do NOT write the code yourself.  Be precise about what you want produced.
"""

DEVELOPER_PROMPT = """ROLE: Developer
Your Architect has given you specific instructions.  Follow them exactly.

INSTRUCTIONS:
{developer_instructions}

OUTPUT: Complete, working code (or content) — no snippets, no placeholders.
Code only.  No explanations unless explicitly asked.
"""

REVIEWER_PROMPT = """ROLE: Reviewer (same thread as Architect — your earlier criteria are in context)
The Developer has produced this output:
```
{developer_output}
```
Verify against the criteria you defined earlier.  Output ONLY valid JSON:
{{
  "verdict": "pass" | "retry" | "fail",
  "reasons": ["Specific reason 1", "..."],
  "retry_instructions": "If verdict is retry, concrete guidance.  Otherwise null."
}}
"""


# ===== Codex CLI subprocess wrapper ==========================================
# Reality from `codex --help` (codex-cli 0.128.0):
#   - non-interactive: `codex exec [PROMPT] [--json] [-o FILE]`
#   - resume:          `codex exec resume <SESSION_ID> [PROMPT]`
#   - `--json` emits one JSON event per line; ends with `turn.completed`
#   - first event:     `{"type":"thread.started","thread_id":"<uuid>"}`
#   - final answer:    `{"type":"item.completed","item":{"type":"agent_message","text":"..."}}`
# That `thread_id` is the resume token we keep — Codex stores history on disk for us.
@dataclass
class CodexResult:
    text: str
    thread_id: str
    raw: list[dict]  # full JSONL event list, useful for debugging


def call_codex(prompt: str, cfg: Config, *, session_id: str | None = None) -> CodexResult:
    """Invoke `codex exec` (or `codex exec resume`) and parse the JSONL stream."""
    args: list[str] = [cfg.architect_cmd, "exec", "--json", "--skip-git-repo-check"]
    if session_id:
        # `codex exec resume <SESSION_ID> -` reads prompt from stdin.
        args += ["resume", session_id, "-"]
    else:
        # No session: prompt also via stdin (avoids quoting hell on long prompts).
        args += ["-"]
    proc = subprocess.run(
        args, input=prompt, capture_output=True, text=True, timeout=cfg.architect_timeout
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"codex exec failed (exit {proc.returncode}):\n{proc.stderr or proc.stdout}"
        )
    events: list[dict] = []
    final_text = ""
    thread_id = session_id or ""
    for line in proc.stdout.splitlines():
        line = line.strip()
        if not line:
            continue
        try:
            evt = json.loads(line)
        except json.JSONDecodeError:
            continue  # codex may print non-JSON warnings; skip them
        events.append(evt)
        etype = evt.get("type")
        if etype == "thread.started":
            thread_id = evt.get("thread_id", thread_id)
        elif etype == "item.completed":
            item = evt.get("item", {})
            if item.get("type") == "agent_message":
                final_text = item.get("text", "")
    if not final_text:
        raise RuntimeError("codex exec produced no agent_message.  Raw stdout:\n" + proc.stdout[:2000])
    return CodexResult(text=final_text, thread_id=thread_id, raw=events)


# ===== Ollama wrapper ========================================================
# The Developer is the dumb, time-expensive part of the pipeline.
# We use the blocking client and let the caller wrap it in `asyncio.to_thread`.
def call_ollama(prompt: str, cfg: Config) -> str:
    client = ollama.Client(host=cfg.developer_host, timeout=cfg.developer_timeout)
    resp = client.chat(
        model=cfg.developer_model,
        messages=[{"role": "user", "content": prompt}],
        options={"temperature": cfg.developer_temperature, "num_ctx": cfg.developer_num_ctx},
    )
    return resp["message"]["content"]


# ===== JSON parsing helper ===================================================
# Models sometimes wrap JSON in ``` fences or add a stray sentence.
# Plain json.loads first, then a fenced-block fallback, then balanced-brace fallback.
_FENCE_RE = re.compile(r"```(?:json)?\s*(\{.*?\})\s*```", re.DOTALL)


def parse_json_blob(text: str) -> dict[str, Any]:
    text = text.strip()
    try:
        return json.loads(text)
    except json.JSONDecodeError:
        pass
    m = _FENCE_RE.search(text)
    if m:
        return json.loads(m.group(1))
    start, end = text.find("{"), text.rfind("}")
    if start >= 0 and end > start:
        return json.loads(text[start : end + 1])
    raise ValueError(f"No JSON object found in: {text[:200]}...")


# ===== Codex session listing (the resume feature) ============================
# Codex stores sessions at ~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<uuid>.jsonl
# First JSONL line is `session_meta` with payload.id and payload.timestamp.
# This is the simplest "externalized state" — no DB, no vector store; the
# conversation history IS the resume token.
@dataclass
class SessionInfo:
    session_id: str
    started_at: str
    first_user_prompt: str
    path: Path


def _extract_first_text(evt: dict) -> str:
    payload = evt.get("payload") or evt.get("item") or {}
    if isinstance(payload, dict):
        for key in ("text", "content", "input"):
            v = payload.get(key)
            if isinstance(v, str) and v.strip():
                return v
    return ""


def list_recent_sessions(limit: int = 10) -> list[SessionInfo]:
    if not CODEX_SESSIONS_DIR.exists():
        return []
    files = sorted(
        CODEX_SESSIONS_DIR.rglob("rollout-*.jsonl"),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )[:limit]
    out: list[SessionInfo] = []
    for path in files:
        try:
            session_id = ""
            started_at = ""
            first_prompt = "(no prompt yet)"
            with path.open("r", encoding="utf-8") as fh:
                for line in fh:
                    try:
                        evt = json.loads(line)
                    except json.JSONDecodeError:
                        continue
                    if evt.get("type") == "session_meta":
                        payload = evt.get("payload", {})
                        session_id = payload.get("id", "")
                        started_at = payload.get("timestamp", "")
                    elif first_prompt == "(no prompt yet)":
                        text = _extract_first_text(evt)
                        if text:
                            first_prompt = text[:80].replace("\n", " ")
                            break
            if session_id:
                out.append(SessionInfo(session_id, started_at, first_prompt, path))
        except OSError:
            continue
    return out


# ===== workflow ==============================================================
# Three steps + a retry loop driven by the Reviewer's verdict.
# UI updates are pushed via callbacks so the same logic can run headless.
@dataclass
class WorkflowEvent:
    role: str  # "Architect" | "Developer" | "Reviewer" | "System"
    body: str
    duration_ms: int = 0
    is_error: bool = False


@dataclass
class WorkflowState:
    session_id: str | None = None
    retries_left: int = 2
    history: list[WorkflowEvent] = field(default_factory=list)


async def run_workflow(
    user_input: str, cfg: Config, state: WorkflowState, emit: Callable[[WorkflowEvent], None]
) -> None:
    """End-to-end 3-role flow.  `emit(event)` updates the UI (or stdout in tests)."""
    state.retries_left = cfg.max_retries

    # 1. Architect — decompose
    arch_t0 = time.monotonic()
    try:
        arch = await asyncio.to_thread(
            call_codex, ARCHITECT_PROMPT.format(user_input=user_input), cfg, session_id=state.session_id
        )
    except Exception as exc:
        emit(WorkflowEvent("Architect", f"FAILED: {exc}", is_error=True))
        return
    arch_ms = int((time.monotonic() - arch_t0) * 1000)
    state.session_id = arch.thread_id
    try:
        plan = parse_json_blob(arch.text)
        instructions, criteria = plan["developer_instructions"], plan.get("verification_criteria", [])
    except (ValueError, KeyError) as exc:
        emit(WorkflowEvent("Architect", f"JSON parse error: {exc}\nRaw:\n{arch.text}",
                           duration_ms=arch_ms, is_error=True))
        return
    emit(WorkflowEvent("Architect",
                       "instructions:\n" + instructions + "\n\ncriteria:\n- " + "\n- ".join(criteria),
                       duration_ms=arch_ms))

    # 2. Developer + 3. Reviewer (retry loop)
    dev_input = instructions
    while True:
        dev_t0 = time.monotonic()
        try:
            dev_output = await asyncio.to_thread(
                call_ollama, DEVELOPER_PROMPT.format(developer_instructions=dev_input), cfg
            )
        except Exception as exc:
            emit(WorkflowEvent("Developer", f"FAILED: {exc}", is_error=True))
            return
        dev_ms = int((time.monotonic() - dev_t0) * 1000)
        emit(WorkflowEvent("Developer", dev_output, duration_ms=dev_ms))

        rev_t0 = time.monotonic()
        try:
            rev = await asyncio.to_thread(
                call_codex, REVIEWER_PROMPT.format(developer_output=dev_output), cfg,
                session_id=state.session_id,  # resume! Architect criteria are in context.
            )
        except Exception as exc:
            emit(WorkflowEvent("Reviewer", f"FAILED: {exc}", is_error=True))
            return
        rev_ms = int((time.monotonic() - rev_t0) * 1000)
        try:
            verdict_obj = parse_json_blob(rev.text)
        except ValueError as exc:
            emit(WorkflowEvent("Reviewer", f"JSON parse error: {exc}\nRaw:\n{rev.text}",
                               duration_ms=rev_ms, is_error=True))
            return
        verdict = verdict_obj.get("verdict", "fail")
        reasons = verdict_obj.get("reasons", [])
        retry_instr = verdict_obj.get("retry_instructions") or ""
        emit(WorkflowEvent(
            "Reviewer", f"verdict: {verdict}\nreasons:\n- " + "\n- ".join(reasons),
            duration_ms=rev_ms,
        ))

        if verdict == "pass":
            return
        if verdict == "fail" or state.retries_left <= 0:
            emit(WorkflowEvent("System", "Workflow stopped.  User intervention needed.", is_error=True))
            return
        state.retries_left -= 1
        emit(WorkflowEvent("System",
                           f"Retrying ({cfg.max_retries - state.retries_left}/{cfg.max_retries})..."))
        # Retry guidance is the only changing input; everything else is the same workflow.
        dev_input = instructions + "\n\nADDITIONAL GUIDANCE FROM REVIEWER:\n" + retry_instr


# ===== TUI ===================================================================
# Two RichLogs (left + right) collect role output; a TextArea takes user input;
# Ctrl+Enter submits.  We do not stream tokens — `codex exec` finishes a turn
# before printing the agent_message, and the lesson is the pattern, not streaming.
CSS = """
Screen { layout: vertical; }
#panes { height: 1fr; }
#left, #right { width: 1fr; border: solid $primary; padding: 0 1; }
#left { border-title-align: left; border-title-color: $accent; }
#right { border-title-align: left; border-title-color: $success; }
#input-row { height: 7; border-top: solid $secondary; padding: 0 1; }
TextArea { height: 5; }
"""


class ResumeScreen(ModalScreen[str | None]):
    """Pop-up listing recent Codex sessions; returns the chosen session id."""

    BINDINGS = [Binding("escape", "dismiss", "Cancel")]

    def __init__(self, sessions: list[SessionInfo]) -> None:
        super().__init__()
        self.sessions = sessions

    def compose(self) -> ComposeResult:
        yield Label("Pick a Codex session to resume (Esc to cancel)", id="resume-label")
        items = [
            ListItem(Label(f"{s.started_at}  {s.session_id[:8]}  {s.first_user_prompt}"), id=f"sess-{i}")
            for i, s in enumerate(self.sessions)
        ]
        yield ListView(*items, id="resume-list")

    def on_list_view_selected(self, event: ListView.Selected) -> None:
        idx = int(event.item.id.split("-")[1])
        self.dismiss(self.sessions[idx].session_id)

    def action_dismiss(self) -> None:
        self.dismiss(None)


class GptLocalApp(App):
    CSS = CSS
    BINDINGS = [
        Binding("ctrl+enter", "submit", "Send"),
        Binding("ctrl+r", "open_resume", "Resume..."),
        Binding("ctrl+n", "new_session", "New session"),
        Binding("ctrl+q", "quit", "Quit"),
    ]

    def __init__(self, cfg: Config) -> None:
        super().__init__()
        self.cfg = cfg
        self.state = WorkflowState(retries_left=cfg.max_retries)
        self.title = "gpt-local-tui"
        self.sub_title = "session: (new)"

    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal(id="panes"):
            self.left = RichLog(id="left", wrap=True, markup=False, highlight=False)
            self.left.border_title = "Architect / Reviewer (Codex)"
            self.right = RichLog(id="right", wrap=True, markup=False, highlight=False)
            self.right.border_title = f"Developer ({self.cfg.developer_model})"
            yield self.left
            yield self.right
        with Vertical(id="input-row"):
            self.input = TextArea(id="input")
            yield self.input
        yield Footer()

    def action_submit(self) -> None:
        text = self.input.text.strip()
        if not text:
            return
        self.input.clear()
        self.left.write(f"\n>>> {text}\n")
        self.run_worker(self._run_workflow(text), exclusive=True)

    def action_new_session(self) -> None:
        self.state = WorkflowState(retries_left=self.cfg.max_retries)
        self.sub_title = "session: (new)"
        self.left.write("\n[system] new session\n")

    async def action_open_resume(self) -> None:
        sessions = await asyncio.to_thread(list_recent_sessions, 10)
        if not sessions:
            self.left.write("\n[system] no recent Codex sessions found\n")
            return
        chosen = await self.push_screen_wait(ResumeScreen(sessions))
        if chosen:
            self.state.session_id = chosen
            self.sub_title = f"session: {chosen[:8]}"
            self.left.write(f"\n[system] resumed session {chosen[:8]}\n")

    async def _run_workflow(self, user_input: str) -> None:
        def emit(evt: WorkflowEvent) -> None:
            self.call_from_thread(self._render_event, evt)
        await run_workflow(user_input, self.cfg, self.state, emit)
        if self.state.session_id:
            self.sub_title = f"session: {self.state.session_id[:8]}"

    def _render_event(self, evt: WorkflowEvent) -> None:
        sink = self.right if evt.role == "Developer" else self.left
        timing = f"  ({evt.duration_ms} ms)" if self.cfg.show_timing and evt.duration_ms else ""
        prefix = f"\n[{evt.role}{timing}]" + (" ERROR" if evt.is_error else "")
        sink.write(f"{prefix}\n{evt.body}\n")


# ===== entry point ===========================================================
def main() -> int:
    cfg = load_config()
    # Headless smoke-test: `python app.py --check` validates CLIs without launching the TUI.
    if "--check" in sys.argv:
        try:
            r = subprocess.run([cfg.architect_cmd, "--version"], capture_output=True, text=True, timeout=10)
            print("codex:", r.stdout.strip() or r.stderr.strip())
        except FileNotFoundError:
            print("codex: NOT FOUND on PATH (set [architect].command in config.toml)")
            return 2
        try:
            client = ollama.Client(host=cfg.developer_host, timeout=10)
            models = [m["model"] for m in client.list().get("models", [])]
            print(f"ollama models: {', '.join(models) or '(none)'}")
            if cfg.developer_model not in models:
                print(f"WARNING: configured model '{cfg.developer_model}' not in ollama list")
                return 3
        except Exception as exc:
            print(f"ollama: {exc}")
            return 4
        print("config OK")
        return 0
    GptLocalApp(cfg).run()
    return 0


if __name__ == "__main__":
    sys.exit(main())
