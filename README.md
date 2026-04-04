[![Language: Korean](https://img.shields.io/badge/Language-Korean-2563eb)](./README.md)
[![English](https://img.shields.io/badge/English-9ca3af)](./README.en.md)

# tunaFlow

Tauri 2 + React + TypeScript + Rust + SQLite로 만든 멀티에이전트 오케스트레이션 클라이언트(AOC).

> Of the agent, By the agent, For the agent

사용자 편의만을 위한 채팅 앱이 아니라, 에이전트가 더 적은 마찰로 더 좋은 컨텍스트를 가지고 덜 낭비하며 작업하게 만드는 것을 우선하는 도구입니다. 사용자가 도메인 지식과 방향을 결정하고, 에이전트가 그 결정을 최적의 조건에서 실행합니다.

---

## 주요 기능

### 멀티엔진 에이전트

| 엔진 | 연동 방식 | 스트리밍 |
|------|----------|---------|
| Claude (Anthropic) | CLI + SDK (branch 자동 전환) | ✅ stream-json |
| Codex (OpenAI) | CLI + SDK | ✅ JSONL synthetic |
| Gemini (Google) | CLI + SDK | ✅ stream-json |
| OpenCode | CLI | one-shot |
| OpenAI Compatible (Ollama/LM Studio/vLLM) | HTTP SSE | ✅ SSE streaming |

- 프로젝트 단위로 에이전트를 실행하며, 모든 실행은 background thread/tokio task에서 동작
- Agent Profiles: engine/model/persona/default-skill을 프로필로 묶어 빠르게 전환
- Tool Steps 가시화: Claude/Codex/Gemini의 중간 작업 (thinking, tool_use, file_change)을 실시간 표시

### Roundtable (RT) 토론

여러 에이전트가 하나의 주제에 대해 토론하는 기능:

- **Sequential** — 에이전트가 순서대로 발언, 이전 발언을 참고
- **Deliberative** — 모든 에이전트가 동시에 응답, completion-order로 수집
- Blind verifier, role-based output cap (proposer/reviewer/synthesizer 등)
- 참가자별 identity 주입 — 이름/엔진/역할이 프롬프트에 명시
- 모든 RT는 Branch의 확장 모드로 동작 (드로어에서 열림)

### Branch & Adopt

대화 중간 지점에서 분기하여 독립 실험 후 요약을 채택:

- 메인 대화의 임의 메시지에서 Branch 생성
- Branch 안에서 독립적인 대화/RT 실행
- Adopt: Branch 결과를 요약하여 부모 대화에 삽입
- 모든 Branch는 오른쪽 드로어(슬라이더)로 열림

### 오케스트레이션 워크플로우 파이프라인

Plan 기반 자동화 파이프라인:

```
Chat → Plan 승격 → Approval(승인/검토/보류) → Implementation Branch
→ Developer 자동 호출 → Review RT(2-agent) → Verdict → Done/Rework 루프
```

- `<!-- tunaflow:plan-proposal -->` 등 마커 기반 자동 감지
- PlanProposalCard, ApprovalGate, ImplPlanCard, ReviewVerdictCard UI
- Doom Loop 감지: review 3회 실패 시 자동 에스컬레이션
- zod 스키마 검증 (5개 워크플로우 스키마, graceful degradation)

### ContextPack — 4-engine 공통 프롬프트 조립

매 요청마다 동일한 구조의 normalized prompt를 조립하여 모든 엔진에 전달:

- **Identity**: profile/engine/model/persona 3층 분리 + 한국어 응답 규칙
- **Context modes**: Lite / Standard / Full / Auto (대화 길이 기반 자동 선택)
- **포함 섹션**: identity, project, persona, recent context (author attribution), compressed memory, plan, findings, artifacts, skills, rawq 코드 검색, cross-session context, chops (context-hub)
- **Budget control**: section별 압축 목표, total cap 조정 가능 (Settings)
- **Compressed conversation memory**: 12+ 메시지 시 주제별(topic) JSON 요약, provenance/model 기록
- **Multi-agent context**: participants meta + budget-based dynamic window + per-agent last-message guarantee

### 장기기억 & 벡터 검색

- **주제별 메모리**: JSON 배열 토픽 분할 저장 (1-5개 토픽/대화)
- **자동 세션 발견**: FTS5 + Vector 하이브리드로 관련 대화 자동 연결
- **Vector DB**: rawq embed CLI 활용 (snowflake-arctic-embed-s 384차원), conversation_chunks 테이블, brute-force cosine 검색

### rawq — 코드 검색 엔진

- Sidecar binary로 앱 시작 시 daemon 자동 실행
- 임베딩 모델 상주, `.gitignore` 존중 인덱싱
- SearchOptions: rerank, token-budget, text-weight, rrf-weight
- 개념 쿼리 vs 코드 쿼리 자동 감지 → 가중치 조정

### UI/UX

- **Linear-inspired 레이아웃**: 사이드바 + 5-tab CenterPanel (Chat/Plan/Artifacts/Review/Test)
- **react-virtuoso**: 대량 메시지 가상 스크롤
- **cmdk**: Cmd+K 커맨드 팔레트 (탭/대화/프로젝트 전환, 새 대화, 설정)
- **RuntimeStatusBar**: trace + context mode + memory + rawq 상태 + cost
- **Settings**: Agents / Personas / Runtime 섹션 분리
- **Skills**: vendor별 스킬 snapshot 로딩 (`~/.tunaflow/skills/`), 워크플로우 phase별 자동 주입

---

## 아키텍처

```
┌──────────────────────────────────────────────────────────────┐
│ Frontend (React 18 + Zustand 5 + Tailwind CSS 4)            │
│ ├─ Sidebar — Project selector / Chats / Artifacts / Skills  │
│ ├─ CenterPanel — Chat / Plan / Artifacts / Review / Test    │
│ ├─ Drawers — Branch / RT (오른쪽 슬라이더)                    │
│ ├─ Settings — Agents / Personas / Runtime                   │
│ └─ RuntimeStatusBar + TraceModal + CommandPalette           │
├──────────────────────────────────────────────────────────────┤
│ Tauri 2 Host (Rust + Tokio async)                           │
│ ├─ Commands — CRUD + background agent execution             │
│ ├─ Agents — claude, codex, gemini, opencode, ollama + SDKs  │
│ ├─ Context — ContextPack, compression, vector search        │
│ ├─ Workflow — Plan/Approval/Review/Verdict pipeline         │
│ └─ DB — SQLite WAL, dual read/write, v25 schema            │
├──────────────────────────────────────────────────────────────┤
│ CLI Agents / Sidecars                                       │
│ ├─ claude (Anthropic) — CLI + SDK                           │
│ ├─ codex (OpenAI) — CLI + SDK                               │
│ ├─ gemini (Google) — CLI + SDK                              │
│ ├─ opencode — CLI                                           │
│ ├─ ollama/LM Studio/vLLM — OpenAI-compatible HTTP           │
│ ├─ rawq — code retrieval + embedding sidecar                │
│ └─ context-hub — knowledge search sidecar                   │
└──────────────────────────────────────────────────────────────┘
```

---

## 기술 스택

| 계층 | 기술 |
|------|------|
| Desktop | Tauri 2 |
| Frontend | React 18, TypeScript, Zustand 5, Tailwind CSS 4 |
| Backend | Rust, Tokio (async), rusqlite (bundled SQLite) |
| Virtual scroll | react-virtuoso |
| Command palette | cmdk |
| Toast | sonner |
| Markdown | react-markdown, remark-gfm, react-syntax-highlighter (Prism + oneDark) |
| Schema validation | zod |
| Icons | Lucide React |
| Testing | Vitest + jsdom (frontend), Cargo test (Rust) |

---

## 사전 준비

- [Node.js](https://nodejs.org/) 20+
- [Rust](https://www.rust-lang.org/tools/install) stable
- [Tauri CLI](https://v2.tauri.app/start/prerequisites/)
- 아래 에이전트 CLI 중 최소 1개 이상:
  - `claude` — [Claude Code CLI](https://docs.anthropic.com/en/docs/claude-code)
  - `codex` — [OpenAI Codex CLI](https://github.com/openai/codex)
  - `gemini` — [Gemini CLI](https://github.com/google-gemini/gemini-cli)
- (선택) rawq sidecar — `./scripts/build-rawq.sh`로 빌드
- (선택) Ollama — 로컬 LLM 실행 시

---

## 시작하기

```bash
# 의존성 설치
npm install

# 개발 실행
npm run tauri dev

# 프로덕션 빌드
npm run tauri build
```

---

## 빌드 검증

```bash
npx tsc --noEmit              # TypeScript check
npx vite build                # Frontend build
cd src-tauri && cargo check   # Rust check

# Tests
npx vitest run                # Frontend tests (96개)
cd src-tauri && cargo test --lib  # Rust unit tests (84개)
```

현재 Rust 84개 + Frontend 96개 = **180개 테스트** 통과.

---

## DB 스키마 (v25)

| 테이블 | 용도 |
|--------|------|
| `projects` | 프로젝트 (path, type, soft-delete) |
| `conversations` | 대화 (chat/roundtable mode, rt_config JSON) |
| `messages` | 메시지 (role, content, engine, model, persona) |
| `messages_fts` | FTS5 전문 검색 (트리거 동기화) |
| `branches` | 대화 분기 (chat/roundtable mode, parent chain) |
| `plans` | 워크플로우 플랜 (phase, architect/developer/reviewer engines) |
| `plan_subtasks` | 플랜 하위 작업 |
| `plan_events` | 플랜 이벤트 타임라인 |
| `artifacts` | 산출물 (type, status, subtask 연결) |
| `memos` | 메모 (message 연결, tags) |
| `trace_log` | ContextPack 트레이스 (mode, sections, length, truncation) |
| `agent_jobs` | 에이전트 작업 레지스트리 |
| `conversation_memory` | 주제별 압축 메모리 (topic, provenance, model) |
| `session_links` | 자동 세션 발견 링크 (score, method) |
| `conversation_chunks` | 벡터 임베딩 (BLOB, 384차원) |

24개 인덱스 + FTS5 가상 테이블.

---

## 프로젝트 구조

```
tunaFlow/
├── src-tauri/                # Rust backend
│   ├── src/
│   │   ├── lib.rs            # Tauri app builder + command registration
│   │   ├── agents/           # CLI/SDK adapters (claude, codex, gemini, opencode, ollama, rawq)
│   │   ├── commands/         # Tauri commands + helpers
│   │   │   ├── agents.rs           # 5-engine background stream commands
│   │   │   ├── agents_helpers/     # ContextPack, identity, send_common
│   │   │   ├── roundtable.rs       # RT orchestration
│   │   │   ├── roundtable_helpers/ # RT executor, prompt, persist
│   │   │   ├── conversation_memory.rs  # 주제별 압축 메모리
│   │   │   ├── session_discovery.rs    # FTS5+Vector 세션 발견
│   │   │   ├── vector_search.rs        # 벡터 임베딩/검색
│   │   │   └── ...
│   │   ├── db/               # SQLite schema, migrations (v1-v25), models
│   │   ├── errors.rs         # AppError enum
│   │   └── guardrail.rs      # Context budget limits
│   ├── binaries/             # rawq sidecar (gitignored)
│   └── Cargo.toml
├── src/                      # React frontend
│   ├── components/tunaflow/
│   │   ├── chat/             # Markdown rendering, FileViewer
│   │   ├── context-panel/    # Plans, Review, Test, Trace, Skills, Artifacts, Evaluation
│   │   ├── settings/         # Agents, Personas, Runtime sections
│   │   ├── input/            # EngineSelector, ModelSelector, RoundtableControls
│   │   ├── message/          # MessageMeta, MessageActions, ProgressSurface
│   │   ├── sidebar/          # Chats, TreeRow, Artifacts, Files, Scratchpad
│   │   ├── CenterPanel.tsx   # 5-tab center (Chat/Plan/Artifacts/Review/Test)
│   │   └── RuntimeStatusBar.tsx
│   ├── stores/slices/        # Zustand slices (project, conversation, thread, runtime, asset, engineModel)
│   ├── lib/                  # utils, constants, schemas, parsers, engineConfig
│   └── tests/                # vitest tests
├── docs/
│   ├── plans/                # 구현 계획 (~100개, index.md 참조)
│   ├── prompts/              # 실행 프롬프트
│   ├── reference/            # SSOT 문서
│   ├── ideas/                # 아이디어/브레인스토밍
│   └── how-to/               # 운영 가이드
├── scripts/                  # build-rawq.sh, publish-skills.sh
├── CLAUDE.md                 # Claude Code handoff document
└── package.json
```

---

## 개발 이력

| 세션 | 핵심 성과 |
|------|----------|
| 1 | Linear UI 리팩토링, 4-engine parity, 드로어/Branch/RT 통합, Skills, Agent Profile/Persona |
| 2 | ContextPack 전체 파이프라인, identity, compressed memory, 108 tests |
| 3 | Claude parity fix, auto mode bias 수정, agents.rs 1168→260줄 |
| 4 | Multi-agent context 3-layer, project scaffold, deps Phase 1-4, rawq fs watcher |
| 5 | 오케스트레이션 워크플로우 파이프라인 Phase A-E 전체 완료 |
| 6 | zod 스키마, Ollama 엔진, Tool Steps 가시화, silent error 표면화 |
| 7 | 장기기억 4단계, Vector DB, virtuoso, cmdk, tokio async, 실사용 검증 50+ 버그 수정 |
| 8-9 | 이벤트 격리, RT 전면 수정, 스트리밍 race condition 해결, Virtuoso re-render, duration/token 표시 |
| 10 | 스킬 4-layer + 레지스트리, CRG 통합, 마커 기반 도구 호출, 워크플로우 에이전트 고도화, DB v25 |

---

## 문서

| 문서 | 용도 |
|------|------|
| [CLAUDE.md](./CLAUDE.md) | Claude Code용 상세 handoff (아키텍처, 스키마, 컨벤션) |
| [Data Model](./docs/reference/dataModelRevised.md) | 도메인 모델 SSOT |
| [Implementation Status](./docs/reference/implementationStatus.md) | 기능별 구현 현황 |
| [Plans Index](./docs/plans/index.md) | 구현 계획 인덱스 (~100개) |
| [Known Issues](./docs/reference/knownIssues_2026-04-03.md) | 미해결 이슈 |

---

## 라이선스

Private project.
