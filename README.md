<div align="center">

# tunaFlow

**A desktop client that runs Claude Code, Codex, Gemini, and local engines under one Plan → Dev → Review workflow.**

[![CI](https://github.com/hang-in/tunaFlow/actions/workflows/ci.yml/badge.svg)](https://github.com/hang-in/tunaFlow/actions/workflows/ci.yml)
[![Tauri 2](https://img.shields.io/badge/Tauri-2.0-FFC131?logo=tauri&logoColor=white)](https://v2.tauri.app/)
[![React 18](https://img.shields.io/badge/React-18-61DAFB?logo=react&logoColor=white)](https://react.dev/)
[![Rust](https://img.shields.io/badge/Rust-stable-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue)](./LICENSE)
[![Status](https://img.shields.io/badge/Status-Beta-f59e0b)](./docs/plans/publicReadinessChecklistPlan.md)

[![🇺🇸 English](https://img.shields.io/badge/🇺🇸-English-2563eb)](./README.md)
[![🇰🇷 한국어](https://img.shields.io/badge/🇰🇷-한국어-9ca3af)](./README.ko.md)

> **Of the agent, By the agent, For the agent**

</div>

![tunaFlow screenshot](./docs/assets/screenshot-main.png)

> 📺 Workflow demo (6 min)

https://github.com/user-attachments/assets/69cdc5b3-2456-4873-9599-3c2c3e0f6f13

---

## What it is

A desktop app for orchestrating subscription-tier CLI coding agents (Claude Code, Codex, Gemini) plus local engines (Ollama, LM Studio) inside one window. The same `ContextPack` is assembled for all five engines, so switching providers mid-conversation is a one-line toggle, not a prompt rewrite. The Architect drafts a plan; the Developer implements; a *different vendor's* model performs blind cross-review before any code lands.

Status: Beta. macOS arm64 + Windows x64 builds shipping (v0.1.5-beta). 614 Rust tests · 401 frontend tests · SQLite v49.

## What's notable

A few design choices that aren't standard in the surrounding tools:

- **Single assembly function for five engines** — `build_normalized_prompt_with_budget()` produces the same `ContextPack` (identity, recent context, long-term memory, skills, tool results) regardless of which engine consumes it. Engine swap is a toggle, not a fork.
- **Subscription-CLI-first, not API-first** — primary execution paths spawn the official `claude` / `codex` / `gemini` binaries against the user's own subscription. SDK / API is a fallback. No tunaFlow account, no token markup, no billing axis owned by us.
- **Cross-vendor blind review** — Architect (Claude Opus) and Reviewer (Codex by default) sit on different vendors so failure modes don't cancel out. The 4D rubric (`plan_coverage` / `code_quality` / `test_coverage` / `convention`) + `invariant_checks` runs *before* implementation cost is spent.
- **Branch / adopt model** — experiments fork into shadow conversations; only the summary lands back in main. The chat tree stays linear; side branches don't pollute context.
- **100% AI-authored codebase** — every line was written by Claude Code; a single human maintainer sets architecture and direction. An honest engineering data point about modern multi-agent workflows, not a quality guarantee.

## How it sits next to existing tools

Not a replacement for your IDE or your CLI. The niche is between them — a structured surface that uses subscriptions you already pay for, swaps engines without rewriting prompts, and catches design flaws via cross-vendor blind review.

| | tunaFlow | Editor extensions (Cursor / Continue / Cline) | Single-engine CLIs (Claude Code / aider) | Agent frameworks (crewAI / langgraph) |
|---|---|---|---|---|
| Surface | Desktop app | IDE plugin | Terminal | Library |
| Engines run together | 5 | 1–2 | 1 | Unopinionated |
| Billing model | Subscription CLI | Mostly API | Subscription CLI (own only) | API |
| Plan / Dev / Review pipeline | Built-in, cross-vendor | — | — | DIY |
| Branch / Roundtable | Yes | — | — | DIY |

tunaFlow is unlikely to be your only AI tool — it complements an IDE-side assistant rather than replacing it.

## Non-goals

- **Not an IDE.** No inline completion or syntax-aware suggestion surface.
- **Not a SaaS.** No tunaFlow account, hosted backend, or token markup. The maintainer cannot see your conversations.
- **Not an API-billing wrapper.** Account multiplexing, token resale, OAuth pooling, or any pattern that violates the upstream provider's TOS is out of scope.
- **Not a substitute for human review.** The review pass is performed by *other AI agents*. There is no human gate before commit; treat the cross-vendor blind review as a strong filter, not a guarantee.

## Who this is for

- Users running multiple CLI agents who want a structured workflow surface beyond raw chat.
- Developers who delegate execution but keep direction and judgment.
- Small teams or individuals integrating AI agents into daily development.

## Why it exists

tunaFlow started from a concrete pain: running Claude Code, Codex, and Gemini CLI side by side meant constant copy-pasting between tmux panes, iTerm tabs, or terminal multiplexers like cmux. Even when the individual engines were great, the workflow was manual stitching. tunaFlow bundles that stitching into a single surface so the user's attention stays on intent, not on terminal pane management.

---

## Design Features

### Engine Parity — No prompt rewriting when switching engines
The five engines (Claude, Codex, Gemini, Ollama, LM Studio) share a single assembly function, `build_normalized_prompt_with_budget()`. Since identity, recent context, long-term memory, skills, and tool results are assembled into a consistent `ContextPack` regardless of the engine, switching engines is a one-line toggle, not a prompt rewrite.

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

A common prompt assembly engine for all supported engines (Claude / Codex / Gemini / Ollama / LM Studio). Features automatic `Lite` / `Standard` / `Full` tiering. Includes `rawq` code search, long-term memory, failure learning, and role documentation in the context.

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

- **macOS** 12 Monterey+ (Apple Silicon / Intel) or **Windows** 10 21H2+ (x64). Linux deferred.
- **Node.js 20+**
- **Rust stable** — if not already installed, one line via rustup:

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  source "$HOME/.cargo/env"
  ```

  (If `npm run tauri dev` errors with `cargo metadata ... No such file or directory`, this is why — Rust / cargo is required by Tauri.)

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

### Beta Installation

**macOS** (one-line install):

```bash
curl -fsSL https://raw.githubusercontent.com/hang-in/tunaFlow/main/install.sh | bash
```

> Ad-hoc signature only — a Gatekeeper warning may appear. The installer handles `xattr -cr` for you. Manual: `xattr -cr /Applications/tunaFlow.app`.

**Windows**: download the NSIS installer (`tunaFlow_*_x64-setup.exe`) from the [latest release](https://github.com/hang-in/tunaFlow/releases/latest) and run it.

> SmartScreen may flag the binary (no code-signing in Beta) — choose "More info" → "Run anyway". If antivirus quarantines, restore from quarantine or whitelist `%LOCALAPPDATA%\tunaFlow`. WebView2 Runtime auto-installs if missing.

Detailed install / troubleshooting: [INSTALL.md](./INSTALL.md).

---

## Tech Stack

Tauri 2 + React 18 + TypeScript + Zustand 5 + Tailwind CSS 4 + Rust + SQLite (WAL, v49)

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

### 📖 Dev Blog — 10-part technical series (Korean)

Written by Claude Opus while building tunaFlow. A candid, first-person account of the design decisions, tradeoffs, and mistakes behind each feature.

- **[tunaFlow Wiki](https://github.com/hang-in/tunaFlow/wiki)** — full series + side posts

Highlights:
1. Why an orchestration layer (agent process split)
2. Plan → Dev → Review pipeline
3. Branching conversations (adopt model)
4. Roundtable — making agents debate
5. Long-term memory (brute-force vectors, why not sqlite-vec yet)
6. Engine architecture on a Claude Pro ($20) plan
7. rawq + code-review-graph sidecars
8. Auto-picking skills out of 246
9. Breaking the Doom Loop (failure learning)
10. Running tunaFlow against itself — a full-cycle retrospective

---

## Security & Permissions

tunaFlow launches the Claude CLI with the `--dangerously-skip-permissions` flag. This means the CLI skips approval prompts for file access outside the project directory, system commands, and similar operations.

**Your responsibilities**:
- Choose the project directory you hand to the agent carefully — it defines the trust boundary.
- Do not enable untrusted prompts, tools, or MCP servers.
- Review the work the agent performed periodically; don't treat autonomous runs as unattended.

Why this flag, and why no UI approval flow: the `stream-json` protocol does not emit a `permission_request` event, so there is no way for tunaFlow to intercept prompts and surface them in the UI. The CLI writes prompts directly to the terminal, and tunaFlow holds stdin for outbound messages, so prompts go unanswered — which is exactly the [#178](https://github.com/hang-in/tunaFlow/issues/178) infinite-hang path. Until Anthropic ships a first-class permission event (tracked in `postBetaBacklog` B-20), `--dangerously-skip-permissions` is the pragmatic choice.

If this trade-off is unacceptable for your workflow, run Claude Code directly in a terminal instead of via tunaFlow for that task — the permission surface is identical either way, just under your direct interaction.

---

## Known Constraints (Beta)

### Will be fixed (P0 / P1)

- **PTY Terminal — Work in Progress** — The in-app terminal panel is temporarily unavailable in the Beta bundle and is being rewired. Use an external terminal (iTerm2 / Terminal.app / Warp / Windows Terminal) alongside tunaFlow until a follow-up release restores it.
- **JSONL Completion Detection Failure (P1)** — Occasional issues where PTY session responses are not reflected in the UI (transitioning to `sdk-session` WebSocket path).
- **Linux builds** — Deferred. macOS arm64 + Windows x64 are shipping.

### Anthropic billing & Claude session behavior

- **Claude `-p` headless mode**: tunaFlow uses the `claude -p --resume <id>` CLI path (since v0.1.4-beta, after the upstream `--sdk-url` policy change). Pro/Max plan limits — 5-hour rolling window + weekly cap + overage policy — apply the same as in `claude.ai`.
- **Stale resume_token auto-recovery (v0.1.5-beta+)**: idle conversations may carry a `resume_token` that the upstream session store has rolled off. tunaFlow detects the rejection pattern (`out of extra usage`, `404 session not found`, etc.), clears `--resume`, and retries once. From the next send, `ContextPack` is re-attached in full mode + 2-turn anchor. A toast informs the user when this happens.
- **Manual restart**: right-click any conversation → "Restart Claude session" forces a fresh session for the next send (works alongside the auto-recovery above).
- **Where to check usage**: [claude.ai/settings/usage](https://claude.ai/settings/usage) — verify your 5-hour limit, weekly cap, and "extra usage" (overage) toggle.

### By design / Beta stage

- **Ad-hoc Signature** — No Apple Developer ID signing in Beta. Requires Gatekeeper bypass (`xattr -cr /Applications/tunaFlow.app`). Drag-installing the DMG without running `install.sh` leaves a quarantine attribute on the `.app`, which silently blocks the bundled sidecars (rawq) — see [INSTALL.md → "rawq 가 인식 안 될 때"](./INSTALL.md#rawq-가-인식-안-될-때-footer-rawq-sidecar-없음) for the symptom-cause table.
- **rawq is a bundled sidecar, not a PATH binary** — tunaFlow ships a locally-patched rawq build inside the `.app` bundle (`Contents/MacOS/rawq`) and only resolves it from there. Running `cargo install rawq` to put rawq on `$PATH` does **not** affect tunaFlow — the app intentionally ignores the system-wide rawq to avoid version drift. Build path: `./scripts/build.sh` (recommended wrapper, runs `build-rawq.sh` first); running `npm run tauri build` directly will fail with `binaries/rawq-aarch64-apple-darwin doesn't exist` unless you pre-built the sidecar (upstream: https://github.com/auyelbekov/rawq).
- **rawq source is auto-cloned at build time** — first run of `./scripts/build-rawq.sh` (or `build-rawq.ps1`) auto-clones `vendor/rawq/` from `https://github.com/hang-in/rawq` if no local copy is present. `vendor/rawq/` is gitignored. Override with `RAWQ_SRC=<path>` (offline) or `RAWQ_REPO_URL=<fork>` (private fork). See [INSTALL.md](./INSTALL.md#rawq-소스-자동-clone-외부-contributor--처음-빌드).
- **Limited mid-round RT interruption** — participant-level token streaming works in real time, but once a round is in progress, redirecting the discussion mid-round is awkward. Feedback is delivered between rounds.
- **Initial Indexing Delay** — Large projects may take several minutes for the first run (CPU spikes mitigated via ONNX thread limits, semaphores, and incremental indexing). Build artifact directories (`target/`, `node_modules/`, `dist/`, `.venv/`, `__pycache__/` and similar) are excluded from rawq indexing to prevent OOM — full list and the `Rebuild index` button (for users upgrading from before the exclude list) are documented in [rawq-setup.md](./docs/how-to/rawq-setup.md#제외-패턴-exclude-patterns--issue-180-hotfix).

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

tunaFlow borrows ideas and code from several open-source projects. Thanks to the following maintainers:

### Bundled sidecars (shipped with the app)

- **[rawq](https://github.com/auyelbekov/rawq)** (MIT) — code-search sidecar. tunaFlow ships a locally-patched build as a bundled binary.
- **[code-review-graph](https://github.com/tirth8205/code-review-graph)** (MIT) — CRG sidecar (Full track). Graph-based code review analysis.
- **[context-hub](https://github.com/andrewyng/context-hub)** (MIT) — context-sharing sidecar. Prompted to install on first run (consent required; falls back gracefully if declined).

### Design / architecture influences

- **[abtop](https://github.com/graykode/abtop)** (MIT) — runtime observability / diagnostics for AI coding agents. Shaped tunaFlow's Trace panel and status bar.
- **[hermes-agent](https://github.com/NousResearch/hermes-agent)** (MIT) — memory / toolset / iteration-budget patterns.
- **[larksuite-cli](https://github.com/larksuite/cli)** (MIT) — CLI action layering / shared-rule / async-contract patterns.
- **[chops](https://github.com/Shpigford/chops)** (MIT) — ContextPack code-slice injection ideas.
- **[codex](https://github.com/openai/codex)** (Apache 2.0) — reference implementation for CLI agent protocol work.
- **[xterm.js](https://xtermjs.org/)** (MIT) — terminal rendering in the PTY panel.
- **[react-markdown](https://github.com/remarkjs/react-markdown)** (MIT) — chat markdown rendering.
- **[D2Coding](https://github.com/naver/d2codingfont)** (OFL 1.1) — bundled monospace font.
- **[Tauri](https://tauri.app/)** (MIT / Apache 2.0) — desktop shell framework.

See **[ACKNOWLEDGMENTS.md](./ACKNOWLEDGMENTS.md)** for the full list of 25+ referenced projects (articles, papers, and indirect influences). Full third-party attribution list is in [NOTICE](./NOTICE).

### Philosophy / articles

- **[Code Agent Orchestra](https://addyosmani.com/blog/code-agent-orchestra/)** by Addy Osmani — shaped tunaFlow's multi-agent orchestration philosophy.
- Stavros Korokithakis's Claude Code workflow posts — inspired the `Plan → Dev → Review` pipeline.

---

## Contact

- Email: d9ng@outlook.com
- Issues: https://github.com/hang-in/tunaFlow/issues
- Security: see [SECURITY.md](./SECURITY.md)

---

*100% AI-authored codebase — Claude Code wrote every line; humans provide architecture and direction.*

---
🇺🇸 English · 🇰🇷 [한국어](./README.ko.md)
