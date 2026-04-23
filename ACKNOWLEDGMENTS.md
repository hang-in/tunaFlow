# Acknowledgments

tunaFlow is the result of reading, reviewing, and borrowing from a wide ecosystem of open-source projects, articles, and papers. The main README shows the projects with the most direct influence; this file is the long-form list for anyone who wants to trace the full lineage.

Not every entry here translated into shipped code or documented patterns — some were reviewed and intentionally rejected, and their absence shaped tunaFlow just as much as the ones that stuck. That context lives in `docs/ideas/` (internal) and is not repeated below.

Licensing note: all entries are MIT unless otherwise marked. See each project's LICENSE for authoritative terms.

---

## 1. Bundled sidecars (ship with the app)

| Project | Upstream | License | How tunaFlow uses it |
|---|---|---|---|
| **rawq** | https://github.com/auyelbekov/rawq | MIT | Code-search sidecar. Locally-patched build shipped as bundled binary. |
| **code-review-graph** | https://github.com/tirth8205/code-review-graph | MIT | CRG graph-based code review (Full track bundle; Lite installs at first run). |
| **context-hub** | https://github.com/andrewyng/context-hub | MIT | Curated, versioned context layer. Auto-installed at first launch. |

## 2. Direct design / architecture influence

Projects that tunaFlow's internal design docs (`docs/ideas/`) analyzed in detail and adopted patterns from.

| Project | Upstream | License | Influence |
|---|---|---|---|
| **abtop** | https://github.com/graykode/abtop | MIT | Runtime observability / diagnostics — shaped Trace panel and status bar. |
| **hermes-agent** | https://github.com/NousResearch/hermes-agent | MIT | Memory / toolset / iteration-budget patterns. |
| **larksuite-cli** | https://github.com/larksuite/cli | MIT | CLI action layering / shared-rule / async-contract patterns. |
| **chops** | https://github.com/Shpigford/chops | MIT | ContextPack code-slice injection ideas. |
| **claw-compactor** | https://github.com/open-compress/claw-compactor | MIT | Memory compaction / context compression strategies. |
| **codex** | https://github.com/openai/codex | Apache 2.0 | Reference implementation for CLI agent protocol work. |

## 3. Runtime / menu-bar UI influence

Projects that shaped tunaFlow's status bar, menu-bar metaphors, and token/cost display.

| Project | Upstream | License |
|---|---|---|
| **AgentBar** | https://github.com/scari/AgentBar | MIT |
| **claude-status-bar** | https://github.com/kangraemin/claude-status-bar | MIT |
| **claude-code-stats** | https://github.com/dmelo/claude-code-stats | MIT |
| **duckbar** | https://github.com/rofeels/duckbar | MIT |
| **DINKIssTyle-Markdown-Browser** | https://github.com/DINKIssTyle/DINKIssTyle-Markdown-Browser | MIT |

## 4. Orchestration / multi-agent references

Reviewed for orchestration patterns; influenced the Roundtable (RT) design and the Architect/Developer/Reviewer role split.

| Project | Upstream |
|---|---|
| **agent-skills** (Addy Osmani) | https://github.com/addyosmani/agent-skills |
| **ClawTeam** (HKUDS) | https://github.com/HKUDS/ClawTeam |
| **clawsouls** | https://github.com/clawsouls/clawsouls |
| **agentscope** | https://github.com/agentscope-ai/agentscope |
| **entroly** | https://github.com/juyterman1000/entroly |
| **mex** | https://github.com/open-compress/mex |
| **LightRAG** | https://github.com/HKUDS/LightRAG |
| **opendev** | — |
| **speedy-claude** | — |
| **OpenHarness** | — |
| **optio** | https://github.com/jonwiggins/optio |
| **aiStartKit** (AIProject-Starterkit) | internal reference |
| **hang-in/mempalace** | https://github.com/hang-in/mempalace (author's other project) |

## 5. Articles, papers, and talks

- **[Code Agent Orchestra](https://addyosmani.com/blog/code-agent-orchestra/)** — Addy Osmani. Shaped tunaFlow's multi-agent orchestration philosophy.
- **Stavros Korokithakis's Claude Code workflow posts** — inspired the `Plan → Dev → Review` pipeline.
- Andrej Karpathy's **LLM wiki** concept — shaped the "conversation-as-knowledge-base" direction (applied to `secall`, tunaFlow's sibling project).
- **Mixture-of-Agents (MoA)**, **Multi-Agent Debate (MAD)**, **Self-Refine**, **Agent-as-a-Judge** — surveyed in `docs/ideas/rtAlgorithmEnhancementIdeas.md`; informed the Roundtable verdict rubric.

## 6. Bundled NPM / Cargo dependencies

The full runtime dependency tree is in `package.json` and `src-tauri/Cargo.toml`. Highlights:

- **Tauri** — MIT / Apache 2.0 — https://tauri.app/
- **React, React DOM** — MIT
- **Zustand** — MIT
- **i18next, react-i18next** — MIT
- **react-markdown, remark-gfm, react-syntax-highlighter** — MIT
- **lucide-react** — ISC
- **xterm.js** — MIT
- **ONNX Runtime** (via sqlite-vec / bge-m3 pipeline) — MIT

## 7. Bundled fonts

- **D2Coding** — SIL Open Font License 1.1 — https://github.com/naver/d2codingfont

## 8. A note on attribution scope

This list includes projects whose source was cloned locally for review under `_research/_util/`, even when the final tunaFlow code does not contain a direct patch. The goal is to acknowledge the ecosystem that made this possible — not to claim every entry materially shaped shipped code.

If your project appears here and you'd prefer different attribution language (or would like to be removed), please open an issue or email d9ng@outlook.com.

---

*Last updated: 2026-04-24*
