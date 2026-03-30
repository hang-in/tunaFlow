[![Korean](https://img.shields.io/badge/한국어-9ca3af)](./README.md)
[![Language: English](https://img.shields.io/badge/Language-English-2563eb)](./README.en.md)

# tunaFlow

Multi-agent orchestration IDE built with Tauri 2 + React + TypeScript + Rust + SQLite.

## Features

- **Multi-Agent Chat** — Claude, Codex (OpenAI), Gemini (Google), OpenCode agents with real-time streaming
- **Roundtable Discussion** — Multiple agents debate topics in Sequential or Deliberative (parallel) mode
- **Branch & Adopt** — Fork conversations at any point, experiment independently, merge summaries back
- **ContextPack** — Automatic system prompt assembly with mode/budget control, trace visibility, compressed memory, rawq code search, and explicit knowledge handoff
- **Agent Profiles & Personas** — Profile-based engine/model/persona/default-skill selection with runtime identity framing
- **Artifacts & Evaluation** — Promote responses into reusable artifacts, compare agents under `Test > Evaluation`, and inspect provenance
- **Plan & Track** — Create plans with subtasks, link artifacts, track progress per conversation/branch
- **Search & Git Awareness** — Project-scoped message search, git branch/dirty visibility, guarded branch create/checkout
- **Background Execution** — All agent runs execute in background threads; UI stays responsive
- **Durable Job Registry** — Tracks running/completed/failed jobs and recovers from interrupted runs on restart

## Architecture

```text
┌──────────────────────────────────────────────────────────────┐
│ Frontend (React 18 + Zustand 5 + Tailwind CSS 4)            │
│ ├─ Sidebar — Workspace selector / Chats / Files             │
│ ├─ CenterPanel — Chat / Plan / Artifacts / Review / Test    │
│ ├─ Settings — Agents / Personas / Skills / Runtime          │
│ └─ RuntimeStatusBar + overlay drawers/modals                │
├──────────────────────────────────────────────────────────────┤
│ Tauri 2 Host (Rust)                                         │
│ ├─ Commands — CRUD + background agent execution             │
│ ├─ Agents — claude, codex, gemini, opencode adapters        │
│ ├─ Context — ContextPack, compression, conversation memory  │
│ └─ DB — SQLite WAL, dual read/write connections             │
├──────────────────────────────────────────────────────────────┤
│ CLI Agents / Sidecars                                       │
│ ├─ claude (Anthropic) — stream-json                         │
│ ├─ codex (OpenAI) — JSONL one-shot                          │
│ ├─ gemini (Google) — stream-json                            │
│ ├─ opencode — one-shot                                      │
│ ├─ rawq — code retrieval sidecar                            │
│ └─ context-hub — knowledge search/get sidecar               │
└──────────────────────────────────────────────────────────────┘
```

## Tech Stack

| Layer | Technology |
|---|---|
| Desktop | Tauri 2 |
| Frontend | React 18, TypeScript, Zustand 5, Tailwind CSS 4 |
| Backend | Rust, rusqlite (SQLite bundled) |
| Markdown | react-markdown, remark-gfm, react-syntax-highlighter |
| Icons | Lucide React |
| Testing | Vitest (frontend), Cargo test (Rust) |

## Prerequisites

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) (stable)
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)
- At least one agent CLI installed:
  - `claude` — [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code)
  - `codex` — [OpenAI Codex CLI](https://github.com/openai/codex)
  - `gemini` — [Gemini CLI](https://github.com/google-gemini/gemini-cli)

## Getting Started

```bash
# Install dependencies
npm install

# Development
npm run tauri dev

# Production build
npm run tauri build
```

## Build Verification

```bash
npx tsc --noEmit             # TypeScript check
npx vite build               # Frontend build
cd src-tauri && cargo check  # Rust check

# Tests
npx vitest run               # Frontend smoke/integration tests
cd src-tauri && cargo test --lib  # Rust unit tests
```

Currently Rust 45 + Frontend 55 = **100 tests** passing.

## Project Structure

```text
tunaFlow/
├── src-tauri/               # Rust backend
│   ├── src/agents/          # CLI agent adapters
│   ├── src/commands/        # Tauri command handlers
│   ├── src/db/              # SQLite schema + migrations
│   └── Cargo.toml
├── src/                     # React frontend
│   ├── components/tunaflow/ # UI components
│   ├── stores/slices/       # Zustand store slices
│   ├── lib/                 # Utilities + constants
│   └── tests/               # Vitest tests
├── docs/
│   ├── plans/               # Implementation plans
│   └── reference/           # SSOT documents
├── CLAUDE.md                # Claude Code handoff document
└── package.json
```

## Documentation

- [CLAUDE.md](./CLAUDE.md) — Detailed handoff document for Claude Code
- [Data Model](./docs/reference/dataModelRevised.md) — Domain model SSOT
- [Implementation Status](./docs/reference/implementationStatus.md) — Feature status tracker
- [Plans Index](./docs/plans/index.md) — All implementation plans

## License

Private project.
