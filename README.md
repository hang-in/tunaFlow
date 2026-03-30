[![Language: Korean](https://img.shields.io/badge/Language-Korean-2563eb)](./README.md)
[![English](https://img.shields.io/badge/English-9ca3af)](./README.en.md)

# tunaFlow

Tauri 2 + React + TypeScript + Rust + SQLite로 만든 멀티에이전트 오케스트레이션 IDE.

## 주요 기능

- **멀티에이전트 채팅** — Claude, Codex (OpenAI), Gemini (Google), OpenCode 에이전트를 실시간 스트리밍으로 실행
- **Roundtable 토론** — 여러 에이전트가 Sequential 또는 Deliberative(병렬) 모드로 주제를 토론
- **Branch & Adopt** — 대화를 원하는 시점에서 분기하고, 별도로 실험한 뒤 요약을 채택
- **ContextPack** — 모드/예산 제어, trace 가시화, compressed memory, rawq 코드 검색, 명시적 knowledge handoff를 포함한 시스템 프롬프트 조립
- **Agent Profiles & Personas** — profile 기반 engine/model/persona/default-skill 선택과 runtime identity framing
- **Artifacts & Evaluation** — 응답을 재사용 가능한 artifact로 승격하고, `Test > Evaluation`에서 에이전트 결과를 비교
- **Plan & Track** — plan/subtask 생성, artifact 연결, conversation/branch 단위 진행 관리
- **검색 & Git 인지 기능** — 프로젝트 범위 메시지 검색, git branch/dirty 상태 표시, guarded branch create/checkout
- **백그라운드 실행** — 모든 에이전트 실행은 background thread에서 돌아가며 UI는 계속 반응
- **Durable Job Registry** — 실행 중/완료/실패 작업을 추적하고 재시작 후 복구

## 아키텍처

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

## 기술 스택

| 계층 | 기술 |
|---|---|
| Desktop | Tauri 2 |
| Frontend | React 18, TypeScript, Zustand 5, Tailwind CSS 4 |
| Backend | Rust, rusqlite (bundled SQLite) |
| Markdown | react-markdown, remark-gfm, react-syntax-highlighter |
| Icons | Lucide React |
| Testing | Vitest (frontend), Cargo test (Rust) |

## 사전 준비

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) stable
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)
- 아래 에이전트 CLI 중 최소 1개 이상
  - `claude` — [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code)
  - `codex` — [OpenAI Codex CLI](https://github.com/openai/codex)
  - `gemini` — [Gemini CLI](https://github.com/google-gemini/gemini-cli)

## 시작하기

```bash
# 의존성 설치
npm install

# 개발 실행
npm run tauri dev

# 프로덕션 빌드
npm run tauri build
```

## 빌드 검증

```bash
npx tsc --noEmit             # TypeScript check
npx vite build               # Frontend build
cd src-tauri && cargo check  # Rust check

# Tests
npx vitest run               # Frontend smoke/integration tests
cd src-tauri && cargo test --lib  # Rust unit tests
```

현재 Rust 45개 + Frontend 55개 = **100개 테스트** 통과.

## 프로젝트 구조

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

## 문서

- [CLAUDE.md](./CLAUDE.md) — Claude Code용 상세 handoff 문서
- [Data Model](./docs/reference/dataModelRevised.md) — 도메인 모델 SSOT
- [Implementation Status](./docs/reference/implementationStatus.md) — 기능 상태 추적
- [Plans Index](./docs/plans/index.md) — 구현 계획 인덱스

## 라이선스

Private project.
