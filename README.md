<div align="center">

# tunaFlow

**One surface for Claude Code, Codex, Gemini, and OpenCode — plan, branch, and review across engines.**

[![CI](https://github.com/hang-in/tunaFlow/actions/workflows/ci.yml/badge.svg)](https://github.com/hang-in/tunaFlow/actions/workflows/ci.yml)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![React 18](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue)](./LICENSE)
[![Status](https://img.shields.io/badge/Status-Beta-f59e0b)](./docs/plans/publicReadinessChecklistPlan.md)

[![🇺🇸 English](https://img.shields.io/badge/🇺🇸-English-2563eb)](./README.md)
[![🇰🇷 한국어](https://img.shields.io/badge/🇰🇷-한국어-9ca3af)](./README.ko.md)

> **Of the agent, By the agent, For the agent**

Stop shuttling between tmux panes. tunaFlow is a desktop client that runs multiple CLI coding agents (Claude Code, Codex, Gemini, OpenCode) side by side under a unified Plan → Dev → Review workflow.

</div>

![tunaFlow screenshot](./docs/assets/screenshot-main.png)

---

## Who is this for?

- Users of Claude Code, Codex, or Gemini CLI who need **structured workflows** beyond simple chat.
- Those who want to delegate execution to agents while **retaining direction and judgment**.
- Small teams or individuals looking to integrate AI agents into their daily development workflow.

### Why it exists

tunaFlow started from a concrete pain: running Claude Code, Codex, and Gemini CLI side by side meant constant copy-pasting between tmux panes, iTerm tabs, or terminal multiplexers like cmux. Even when the individual engines were great, the workflow was manual stitching. tunaFlow bundles that stitching into a single surface so the user's attention stays on intent, not on terminal pane management.

---

## Design Features

### Engine Parity — No prompt rewriting when switching engines
The four engines (Claude, Codex, Gemini, Ollama) share a single assembly function, `build_normalized_prompt_with_budget()`. Since identity, recent context, long-term memory, skills, and tool results are assembled into a consistent `ContextPack` regardless of the engine, switching engines is a one-line toggle, not a prompt rewrite.

### Blind Cross-verification — Catching Plan flaws before implementation
The `Plan` is drafted by the `Architect` (Claude Opus) and verified by an independent `Reviewer` (Codex, blind) using `invariant_checks` and a 4D rubric (`plan_coverage`, `code_quality`, `test_coverage`, `convention`). Converging design-phase BLOCKERs reduces the cost of major implementation reworks.

### Branch-adopt model — Preventing chat tree explosion
Experiment with the same topic by branching it to multiple agents (**Branch**). If a result is satisfactory, **adopt** it—injecting only the summary into the main conversation. Side-branch transcripts do not pollute the main context, maintaining a clean flow of conclusions. `Roundtable` (RT) is an extension of this Branch model.

### CLI-first — Maximizing existing subscriptions
The primary execution paths are via Claude Code, Codex, and Gemini **CLI**. The SDK (API billing) is used only as a fallback. This is designed so users with existing subscriptions can utilize all features without additional token costs.

### Quality over token thrift — tunaFlow is not a token-saving app
Output quality comes first. Identity documents, worldview files, and analysis summaries are allowed to be rich (AGENTS.md-level, 1,500–3,000 tokens) when that richness improves agent output. The wasteful axis we do avoid is **redundancy** — re-injecting context already held in the claude session buffer, stale compression leaking into current requests, or the same information doubled across sections. Here, "lean" means "no redundancy", not "compressed to the minimum".

---

## Key Features

### Orchestration Workflow

An `Architect` → `Developer` → `Reviewer` 3-role system.
Once a `Plan` is designed, the `Developer` implements it, and the `Reviewer` performs cross-verification.
If a failure occurs, the system analyzes findings and automatically proposes a rev.N+1 `Plan`.

### Quick / Deep Review

- **Quick**: Fast verification by a single `Reviewer`.
- **Deep**: Cross-verification via `Roundtable` with multiple engines + automated test injection. Evaluated based on a 4D rubric (`plan_coverage`, `code_quality`, `test_coverage`, `convention`) + `invariant_checks`. The `Reviewer` is assigned to a different vendor (blind) from the `Architect`.

### Interactive Session

Maintains a **persistent session** with CLI agents without the need for one-off `-p` flags. Enables full tool usage (file modification, command execution, etc.). tunaFlow avoids redundant context injection as long as the session is active (via Claude `--sdk-url` WebSocket path + PTY legacy fallback).

### Roundtable (RT)

Agents from multiple engines discuss a single topic. Supports `Sequential` or `Deliberative` (simultaneous) modes. All `RT` sessions are extensions of the `Branch` model.

### ContextPack

A common prompt assembly engine for all 4 engines. Features automatic `Lite` / `Standard` / `Full` tiering. Includes `rawq` code search, long-term memory, failure learning, and role documentation in the context.

### Insight

Analyzes data pre-extracted by `rawq` and `code-review-graph`. Covers 6 categories: Stability, Testing, Architecture, Performance, Security, and Tech Debt. Supports automatic `Quick Wins` fixes.

### Meta Agent Onboarding

Automatically detects available agents during initial project setup and recommends an agent configuration suitable for the project stack.

---

## Supported Engines

| Engine | Integration Method |
|------|----------|
| Claude (Anthropic) | CLI subprocess + WebSocket `sdk-session` (Persistent Session) |
| Codex (OpenAI) | CLI subprocess + `app-server` (Stateful thread) |
| Gemini (Google) | CLI subprocess |
| Ollama / LM Studio / vLLM | HTTP SSE (OpenAI-compatible) |

---

## Installation & Execution

### Prerequisites

- macOS (Currently macOS only)
- Node.js 20+, Rust stable
- At least one agent CLI:

```bash
npm install -g @anthropic-ai/claude-code   # Claude
npm install -g @openai/codex               # Codex
npm install -g @google/gemini-cli          # Gemini
```

### Development

```bash
git clone https://github.com/hang-in/tunaFlow.git
cd tunaFlow
npm install
npm run tauri dev
```

### Build

```bash
./scripts/build.sh
```

### Beta Installation (macOS)

```bash
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
```

> As this uses an ad-hoc signature, a Gatekeeper warning may appear.
> Clear it with: `xattr -cr /Applications/tunaFlow.app`

---

## Tech Stack

Tauri 2 + React 18 + TypeScript + Zustand 5 + Tailwind CSS 4 + Rust + SQLite (WAL, v44)

Code Search: `rawq` sidecar (bge-m3 embedding) · `code-review-graph` · `context-hub`
External Integration: HTTP API + WebSocket · MCP Server (`tunaflow-mcp`)

---

## Documentation

| Document | Content |
|------|------|
| [CLAUDE.md](./CLAUDE.md) | Architecture, conventions, handoff |
| [Architecture Detail](./docs/reference/architecture-detail.md) | RT flow, Store structure, DB schema |
| [Implementation Status](./docs/reference/implementationStatus.md) | Feature implementation status |
| [Beta Release Plan](./docs/plans/betaReleaseReadinessPlan.md) | Deployment readiness checklist |
| [Dev History](./docs/reference/devHistory.md) | Project lineage + development history |
| [Session History](./docs/reference/sessionHistory.md) | Detailed session history (for tracking design decisions) |

---

## Known Constraints (Beta)

### Will be fixed (P0 / P1)

- **PTY Terminal — Work in Progress** — The in-app terminal panel is temporarily unavailable in the Beta bundle and is being rewired. Use an external terminal (iTerm2 / Terminal.app / Warp) alongside tunaFlow until a follow-up release restores it.
- **JSONL Completion Detection Failure (P1)** — Occasional issues where PTY session responses are not reflected in the UI (transitioning to `sdk-session` WebSocket path).
- **Windows / Linux builds** — Not yet supported; packaging pipeline in progress.

### By design / Beta stage

- **Ad-hoc Signature** — No Apple Developer ID signing in Beta. Requires Gatekeeper bypass (`xattr -cr /Applications/tunaFlow.app`).
- **No RT Intermediate Streaming** — `Roundtable` results are displayed only after each round completes (structural — requires significant rewiring to change).
- **Initial Indexing Delay** — Large projects may take several minutes for the first run (CPU spikes mitigated via ONNX thread limits, semaphores, and incremental indexing).

Detailed list: [CLAUDE.md §5](./CLAUDE.md)

---

## Help / Shortcuts

Key shortcuts, feature summaries, and troubleshooting tips are available in the `Settings > Help` panel within the app.

---

## Built with tunaFlow

Projects developed using tunaFlow's multi-agent orchestration workflow:

- **[secall](https://github.com/hang-in/secall)** — Hybrid search "second brain" for AI conversations. A CJK-adapted take on Andrej Karpathy's LLM wiki concept.

---

## References & Acknowledgments

tunaFlow borrows ideas and code from several open-source projects. They are not bundled in this repository, but their work shaped tunaFlow's design. Thanks to the following maintainers:

- **[rawq](https://github.com/auyelbekov/rawq)** (MIT) — code-search sidecar. tunaFlow ships a locally-patched build as a bundled binary.
- **[codex](https://github.com/openai/codex)** (Apache 2.0) — reference implementation for CLI agent protocol work.
- **[xterm.js](https://xtermjs.org/)** (MIT) — terminal rendering in the PTY panel.
- **[react-markdown](https://github.com/remarkjs/react-markdown)** (MIT) — chat markdown rendering.
- **[D2Coding](https://github.com/naver/d2codingfont)** (OFL 1.1) — bundled monospace font.
- **[Tauri](https://tauri.app/)** (MIT / Apache 2.0) — desktop shell framework.

Full third-party attribution list is in [NOTICE](./NOTICE).

---

## Contact

- Email: d9ng@outlook.com
- Issues: https://github.com/hang-in/tunaFlow/issues
- Security: see [SECURITY.md](./SECURITY.md)

---

*100% AI-authored codebase — Claude Code wrote every line; humans provide architecture and direction.*

---
🇺🇸 English · 🇰🇷 [한국어](./README.ko.md)
