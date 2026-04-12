# tunaFlow — Claude Code Handoff Document

> 최종 갱신: 2026-04-12 (세션 22 반영)
> SSOT: `docs/reference/dataModelRevised.md` (도메인 모델), `docs/reference/implementationStatus.md` (구현 현황)
> **세션 이력 전체**: `docs/reference/sessionHistory.md` — 새 세션 첫 요청 시 또는 과거 결정 맥락 필요 시 읽을 것

---

## 1. 프로젝트 개요

tunaFlow는 **다중 에이전트 오케스트레이션 클라이언트(AOC)**이다. Tauri 2 + React + TypeScript + Rust + SQLite 기반.

> **"Of the agent, By the agent, For the agent"**
> 도메인 지식을 기반으로 서비스를 구축하는 **인간지능 주도형 개발 어플리케이션**이다.
> 사용자가 도메인 지식과 방향을 결정하고, 에이전트가 그 결정을 최적의 조건에서 실행한다.
> 에이전트가 편해야 결과가 좋아진다는 철학 — ContextPack, identity, memory, retrieval 등 모든 설계는 "에이전트가 불필요한 토큰 낭비 없이, 정확한 맥락으로, 역할 혼동 없이 작업할 수 있는가"를 기준으로 판단한다.

핵심 기능:
- 프로젝트 단위로 Claude/Codex/Gemini/OpenCode 에이전트를 실행
- Roundtable(RT) 토론: 여러 에이전트가 순차(Sequential) 또는 병렬(Deliberative)로 토론
- Branch: 대화 중간에서 분기해 독립 실험 후 adopt(요약 삽입)
- Plan/Artifact/Memo: 작업 계획, 산출물, 메모 관리
- ContextPack: 매 요청마다 normalized prompt를 조립 (4개 엔진 공통)
- rawq: 코드 검색 엔진 (sidecar, daemon 모드)
- Skills: vendor별 스킬 snapshot (`~/.tunaflow/skills/`)

---

## 2. 기술 스택

| 계층 | 기술 |
|---|---|
| Desktop shell | Tauri 2 |
| Frontend | React 18 + TypeScript + Zustand 5 + Tailwind CSS 4 |
| Backend | Rust (tauri commands) |
| DB | SQLite (WAL mode, dual read/write connections) |
| Agent CLI | claude, codex(OpenAI), gemini(Google), opencode |
| Markdown | react-markdown + remark-gfm + react-syntax-highlighter (Prism + oneDark) |
| Icons | Lucide React |
| Code search | rawq (sidecar binary, daemon mode) |

---

## 3. 프로젝트 구조

```
tunaFlow/
├── src-tauri/              # Rust backend
│   ├── src/
│   │   ├── lib.rs          # Tauri app builder + command registration + rawq daemon startup
│   │   ├── agents/         # CLI agent adapters (claude, codex, gemini, opencode, rawq, context_hub, loader)
│   │   ├── commands/       # Tauri commands
│   │   ├── db/             # SQLite schema, migrations(v1-v17), models
│   │   ├── errors.rs       # AppError enum
│   │   └── guardrail.rs    # Context budget limits + truncation
│   ├── binaries/           # rawq sidecar binary (gitignored)
│   └── Cargo.toml
├── src/                    # React frontend
│   ├── components/tunaflow/  # UI 컴포넌트
│   │   ├── chat/           # MarkdownComponents, FileViewer, fileViewerContext
│   │   ├── context-panel/  # PlansPanel, ReviewPanel, TestPanel, TracePanel, SkillsPanel, ArtifactsPanel, EvaluationPanel
│   │   ├── settings/       # AgentsSection, PersonasSection, RuntimeSection (SettingsPanel에서 분리)
│   │   ├── input/          # EngineSelector, ModelSelector, RoundtableControls, useSendActions
│   │   ├── message/        # MessageMeta, MessageActions, ProgressSurface
│   │   ├── sidebar/        # ChatsSection, TreeRow, ArtifactsSidebarPanel, FilesSection
│   │   ├── CenterPanel.tsx # 4-tab center (Chat/Plan/Review/Test)
│   │   ├── RuntimeStatusBar.tsx # 하단 상태바 (trace + rawq)
│   │   └── TraceModal.tsx  # Trace 상세 모달
│   ├── stores/slices/      # Zustand store slices (6개)
│   ├── lib/                # utils, constants, appStore, api/
│   ├── types/index.ts      # 공유 타입
│   └── tests/              # vitest tests
├── scripts/                # build-rawq.sh, build-rawq.ps1, publish-skills.sh
├── docs/
│   ├── plans/              # 실행 계획 문서 40+개 (index.md 참조)
│   ├── prompts/            # 실행 프롬프트 (index.md 참조)
│   ├── reference/          # SSOT 문서
│   └── how-to/             # 운영 가이드
└── package.json
```

---

## 4. 아키텍처 핵심 원칙

### 4.1 Project-centric
모든 데이터는 Project 소속. Store는 선택된 프로젝트의 데이터만 보유.
프로젝트 삭제는 soft-hide (hidden=1) — DB 데이터 보존, 같은 경로 재추가 시 복원.

### 4.1.1 레이아웃 구조 (Linear-inspired)
```
┌──────────┬─────────────────────────────────────┐
│ Sidebar  │ [Chat] [Plan] [Artifacts] [Review]  │ ← 5-tab pills
│ (darkest │ [Test]                              │
│  base)   │    [CHAT] Main — 🔀br              │ ← centered path
│          │ ┌─────────────────────────────────┐ │
│ 📁 Drop  │ │ content (rounded border)        │ │
│ Chats    │ │                                 │ │
│ Artifacts│ └─────────────────────────────────┘ │
│ Skills   ├─────────────────────────────────────┤
│ Files    │ trace+memory │ context mode │ rawq  │ ← full-width footer
│ ⚙ Set   │                                     │
└──────────┴─────────────────────────────────────┘
```
- **Project-first startup**: 프로젝트 미선택 시 ProjectStartup 화면 → 선택 후 메인 진입
- Sidebar: 프로젝트 드롭다운 → 대화 트리 (Chat이 루트) → status dot 인디케이터 → ⚙ Settings
- CenterPanel: 5-tab (Chat/Plan/Artifacts/Review/Test), toolbar zone + content zone + SearchBox
- RuntimeStatusBar: trace(active/skipped + context mode + memory) + rawq 상태 + cost
- Settings: settings/ 폴더에 분리 (AgentsSection, PersonasSection, RuntimeSection)

### 4.2 Background execution
- `start_*` 커맨드: DB 준비 후 즉시 반환, background thread에서 subprocess 실행
- 이벤트: `{engine}:progress`, `{engine}:chunk`, `agent:completed`, `agent:error`
- Frontend: fire-and-forget invoke + event listener 패턴
- DB = SSOT: event를 놓쳐도 `list_messages()`로 복구

### 4.3 Normalized ContextPack (4-engine parity)
- **모든 엔진이 동일한 context를 받음** — `build_normalized_prompt_with_budget()` 공용 함수
- Claude: system prompt 분리 방식, non-Claude: inline prompt에 합침
- 포함 섹션: identity, project, persona, recent context (with author attribution), compressed memory, plan, findings, artifacts, skills, rawq, cross-session, thread inheritance
- rawq는 mode 독립 — `prompt_needs_rawq()` 기준으로 코드 신호가 있으면 항상 포함
- rawq 후처리: confidence 필터(0.4+), dedup(±5줄), confidence 정렬, 300자 snippet
- Compression: section 유형별 압축 목표 (context=800자, cross-session=600자, findings=400자)
- Context mode override + total budget cap: Settings에서 조정 → appStore 영속 → 매 전송 시 전달
- Identity framing: `## Identity` 블록으로 profile/engine/persona 3층 분리 + 한국어 응답 규칙
- Message author attribution: 과거 메시지에 `[assistant:ProfileName (engine)]` 태그 → 작성자 혼동 방지
- Compressed conversation memory: 12+ 메시지 시 오래된 메시지를 구조화 요약 → `conversation_memory` 테이블 → ContextPack `compressed-memory` 섹션

### 4.4 Branch = 대화 분기 공간
- Branch는 git branch와 유사한 역할 — 독립 실험, RT 토론, 지식 정리 공간
- Branch의 메시지는 `branch:{branchId}` shadow conversation에 저장
- **모든 Branch는 오른쪽 드로어(슬라이더)로 열림** — full view 없음
- 드로어 너비: 사이드바 제외 영역의 최대 80%까지 확장 가능

### 4.5 RT = Branch의 협업 모드
- RT는 독립 기능이 아니라 **Branch의 확장 모드**
- `branches.mode: "chat" | "roundtable"` — RT 모드면 여러 에이전트가 토론
- **모든 RT는 채팅의 하위 branch로 생성** — 독립 RT conversation 폐기
- 드로어 안에서 RoundtableView, RT 컨트롤, 참가자 선택이 모두 동작
- 사이드바 구조: Chats 섹션 하위에 RT/Branch를 트리로 표시

### 4.6 rawq = 필수 런타임 의존성
- sidecar binary (`src-tauri/binaries/rawq-{target-triple}`)
- 앱 시작 시 daemon 자동 시작 (임베딩 모델 상주, 30분 idle timeout)
- `.gitignore`를 존중하여 인덱싱 (node_modules, target 등 자동 제외)
- `start_rawq_index` command로 비동기 인덱싱 (UI 블로킹 없음)
- `RawqIndexing` guard로 동일 경로 중복 인덱싱 방지
- timeout 제거 — daemon이 실제 작업 수행, CLI wait_with_output으로 완료 대기

---

## 5. 현재 상태 (세션 22 기준)

- **DB**: v30 / **Rust**: 223 tests / **Frontend**: 174 tests
- **현재 브랜치**: `feature/context-tiering`
- **알려진 이슈** (상세: `docs/reference/knownIssues_2026-04-05.md`)
  - Claude CLI 동시 실행 충돌 (같은 프로젝트 브랜치+메인, P1)
  - RT 중간 스트리밍 미지원 (구조적 변경 필요)
  - window-state: dev 모드 Ctrl+C 종료 시 상태 미저장
- **전체 이력**: `docs/reference/sessionHistory.md`

---

## 6. RT (Roundtable) 실행 흐름

### RT 생성 경로

| 경로 | 설명 |
|---|---|
| 사이드바 [+] | `CreateRoundtableDialog` → 부모 채팅 선택 → RT branch 생성 → 드로어 |
| 메시지 RT 분기 | `CreateRoundtableDialog(checkpointId)` → RT branch 생성 → 드로어 |

- 저장: `branches.mode = "roundtable"` + shadow conversation (`branch:{branchId}`)
- 참가자: `conversations.rt_config` (JSON), 키 = shadow conversation ID
- 열리는 곳: **드로어** (`BranchThreadPanel` → `RoundtableView`)

### 실행 흐름
1. 드로어: `sendThreadRoundtable(prompt, participants, mode)` → `invoke("start_roundtable_run")`
1. 메인 패널 (레거시): `sendRoundtable(prompt, participants, mode)` → `invoke("start_roundtable_run")`
2. Backend: `execute_round()` per participant (Sequential: 직렬, Deliberative: 병렬)
3. Events: `roundtable:participant_status`, `roundtable:progress`, `agent:completed`
4. Frontend: `list_messages()` 리로드 → `RoundtableView` 렌더링

### RT config
- `conversations.rt_config` (JSON) — `{ participants: [...], mode: "sequential"|"deliberative" }`
- RT branch는 shadow conversation ID (`branch:{branchId}`)를 키로 사용
- `get_rt_config` / `save_rt_config` Tauri commands

---

## 7. Frontend Store 구조

`src/stores/chatStore.ts`가 6개 slice를 합성:

| Slice | 핵심 상태 |
|---|---|
| `projectSlice` | `projects`, `selectedProjectKey`, `projectLoading`, `selectProject()` |
| `conversationSlice` | `conversations`, `selectedConversationId`, `messages`, `selectConversation()` |
| `branchSlice` | `branches`, `activeBranchId`, `threadBranchId`, `threadBranchConvId`, `threadMessages`, `openThread()`, `sendThreadMessage()`, `sendThreadRoundtable()`, `sendThreadRoundtableFollowup()` |
| `runtimeSlice` | `runningThreadIds`, `messageQueue`, `sendMessage()`, `sendWithEngine()`, `sendRoundtable()` |
| `assetSlice` | `memos`, `artifacts`, `skills`, `activeSkills` (persist), `crossSessionIds` |
| `engineModelSlice` | `engineModels`, `loadEngineModels()` |

### 주요 실행 패턴
- **메인 패널 전송**: `runtimeSlice.sendWithEngine(engine, prompt)` → `ENGINE_CONFIGS[engine].command` + event listener
- **드로어 전송**: `branchSlice.sendThreadMessage()` → `ENGINE_CONFIGS[engine].command` + event listener (background)
- **드로어 RT 전송**: `branchSlice.sendThreadRoundtable()` → `start_roundtable_run` + event listener (threadMessages)
- **메인 RT 전송**: `runtimeSlice.sendRoundtable()` → `start_roundtable_run` + event listener (messages)
- **입력 라우팅**: `useSendActions({ threadMode })` — RT + threadMode → `sendThreadRoundtable`, RT → `sendRoundtable`, threadMode → `sendThreadMessage`, 일반 → `sendWithEngine(engine)`

---

## 8. DB 스키마 (v30)

| 테이블 | 핵심 필드 |
|---|---|
| `projects` | key(PK), name, path, type, source, hidden |
| `conversations` | id, project_key(FK), label, mode(chat/roundtable), rt_config(JSON), usage_status |
| `messages` | id, conversation_id(FK), role, content, status, engine, model, persona |
| `messages_fts` | FTS5 가상 테이블 (v15, 트리거 기반 동기화) |
| `branches` | id, conversation_id(FK), label, status, checkpoint_id, mode(chat/roundtable), parent_branch_id, git_branch |
| `memos` | id, message_id, content, type, tags |
| `artifacts` | id, conversation_id, type, title, status, subtask_id, plan_id (v28) |
| `failure_lessons` | id, project_key, plan_id, file_path, pattern, finding, resolution (v27) |
| `failure_lessons_fts` | FTS5 가상 테이블 (v27, 트리거 기반 동기화) |
| `plans` | id, conversation_id, title, status, phase, architect_engine, developer_engine, reviewer_engines, implementation_branch_id, review_branch_id, parent_plan_id |
| `plan_subtasks` | id, plan_id(FK), title, status, owner_agent, depends_on(JSON), parallel_group |
| `plan_events` | id, plan_id(FK), event_type, actor, detail, created_at (v18) |
| `trace_log` | id, conversation_id, trace_id, span_id, engine, context_mode, context_sections, context_length, context_truncated, usage_status |
| `agent_jobs` | id, conversation_id, message_id, engine, kind, status, error |
| `conversation_memory` | id, conversation_id(FK), summary, source_count, created_at, updated_at, topic, phase, message_range, provenance, model_used (v21) |
| `session_links` | id, conversation_id(FK), linked_conv_id(FK), score, method, created_at (v21) |
| `conversation_chunks` | id, project_key, conversation_id(FK), kind, root_message_id, text_preview, embedding(BLOB), created_at (v22) |
| `insight_sessions` | id, project_key, status, categories, test_output, summary, created_at, completed_at (v29) |
| `insight_findings` | id, session_id, project_key, category, severity, fix_difficulty, title, description, file_path, line_number, snippet, estimated_files, resolution, plan_id, status, created_at (v29) |
| `insight_reports` | id, session_id, project_key, type, category, content, created_at (v29) |
| `vec_chunks` | 가상 테이블 (sqlite-vec vec0, float[384] cosine distance, v30) |

---

## 9. 주요 이벤트 모델

| 이벤트 | Payload | 발생 시점 |
|---|---|---|
| `claude:progress` | `{ messageId, text }` | thinking/tool_use 진행 |
| `claude:chunk` | `{ messageId, text }` | assistant 텍스트 누적 |
| `gemini:progress/chunk` | 동일 | Gemini streaming |
| `codex:progress/chunk` | 동일 | Codex JSONL synthetic streaming |
| `opencode:progress` | 동일 | 시작 알림 |
| `agent:completed` | `{ messageId, conversationId, engine }` | 실행 완료 |
| `agent:error` | `{ messageId, conversationId, engine, error }` | 실행 실패 |
| `roundtable:participant_status` | `{ conversationId, name, engine, model, round, status }` | participant 시작/완료 |
| `roundtable:progress` | `Message` (full) | participant 응답 완료 |
| `rawq:indexing` | `{ projectPath, message }` | 인덱스 빌드 시작 |
| `rawq:indexed` / `rawq:error` | `RawqStatus` | 인덱스 완료/실패 |

---

## 10. 세션 이력

> 전체 이력: `docs/reference/sessionHistory.md` — 새 세션 첫 요청 시 또는 과거 결정 맥락 필요 시 읽을 것

---

## 11. 다음 우선순위

### P0: 최우선
- **PTY 고도화** — write buffer + ACK + session health check + message queue. 현재 메시지 전달 실패가 실사용 블로커
- **main 머지 준비** — feature/context-tiering 브랜치가 커져서 머지 지연 위험. 실사용 검증 후 머지

### P1: 후순위
- 디자인 시스템 확대 적용 — 사이드바 리팩토링 후속, text-tf-*/prose-* 토큰 점진 교체
- Project-per-window 아키텍처 (`docs/ideas/projectPerWindowIdea.md`) — VS Code 패턴
- 브랜치 label git 스타일 slug화 (띄어쓰기 → 하이픈)
- RT 전용 페르소나 설계 (participant_identity에 행동 지침 추가)
- ContextPack DB/assembly 완전 분리 (파일 분리)
- KnowledgeLayer trait — 6번째 소스 추가 시 도입
- Insight Phase H~J — tool-request:insight 핸들러
- 온보딩 메타에이전트 (`docs/ideas/onboardingMetaAgentIdea.md`)
- CLAUDE.md 경량화 패턴 tunaFlow 적용 — 에이전트 CLAUDE.md에도 sessionHistory 분리 적용

### P2: 후순위
- 디자인 시스템 Phase 2: 라이트 모드 (oklch 통일)
- Gemini SDK 직접 통합 (보조 경로, CLI 기본 유지)
- smoke test 복구
- Trace Phase 2: Git 상태 + OTel 중첩 스팬
- Codex app-server 프로토콜 분석

---

## 12. 빌드 / 실행 / 테스트

```bash
# 개발 실행
npm run tauri dev

# 빌드 검증
npx tsc --noEmit              # TypeScript
npx vite build                # Frontend
cd src-tauri && cargo check   # Rust

# 테스트
npx vitest run                # Frontend (96 tests)
cd src-tauri && cargo test --lib  # Rust unit tests (84 tests)

# rawq sidecar 준비
./scripts/build-rawq.sh       # macOS/Linux
./scripts/build-rawq.ps1      # Windows

# Skills snapshot 발행
./scripts/publish-skills.sh
```

---

## 13. 문서 참조

| 문서 | 용도 |
|---|---|
| `docs/reference/sessionHistory.md` | **세션 이력 전체** — 새 세션 시작 시 또는 과거 결정 맥락 필요 시 읽기 |
| `docs/reference/dataModelRevised.md` | 도메인 모델 SSOT |
| `docs/reference/implementationStatus.md` | 기능별 구현 현황 + Provider 비교 테이블 |
| `docs/plans/index.md` | 40+개 plan 상태 인덱스 |
| `docs/prompts/index.md` | 실행 프롬프트 인덱스 |
| `docs/plans/threadModelRoundtableRedesign.md` | RT/Branch 통합 설계 |
| `docs/plans/engineFeatureParityClassificationPlan.md` | 4-engine parity 분류 (Wave 1+2 완료) |
| `docs/plans/chatUiParityWithTunaChatPlan.md` | tunaChat 수준 UI parity 계획 |
| `docs/reference/chatUiVsTunaChatGapReview_2026-03-29.md` | tunaChat vs tunaFlow UI 비교 |
| `docs/how-to/rawq-setup.md` | rawq 설치/운영 가이드 |
| `docs/how-to/skills-runtime-policy.md` | Skills snapshot 운영 규칙 |

---

## 14. Skill 로딩 규칙

작업 시작 전에 현재 작업 유형에 맞는 skill 1~3개를 `~/.tunaflow/skills/`에서 먼저 읽고 그 규칙에 따라 진행한다.

| 작업 유형 | 추천 스킬 |
|---|---|
| 프론트엔드 구현 | `anthropic-frontend-design`, `microsoft-zustand-store-ts` |
| 프론트엔드 리뷰 | `microsoft-frontend-design-review`, `anthropic-webapp-testing` |
| OpenAI/Codex 연동 | `openai-openai-docs` |
| Claude/Anthropic 연동 | `anthropic-claude-api` |
| MCP/tool 연동 | `anthropic-mcp-builder` |

---

## 15. 작업 안전 규칙

### 실행 경로 검증 우선
- **UI 진입점을 변경하기 전에** 대체 경로가 완전히 동작하는지 반드시 확인한다
- 기존 동작을 제거/교체할 때는 새 동작이 end-to-end로 작동하는 것을 먼저 증명한다
- "나중에 구현"을 전제로 기존 기능을 제거하지 않는다

### 단일 경로 수정 원칙
- 한 번에 여러 실행 경로를 동시에 바꾸지 않는다
- 하나의 경로를 수정 → 검증 → 다음 경로 순서로 진행한다
- 특히 RT/Branch/Thread 같이 여러 모드가 얽힌 기능은 모드별로 분리 수정한다

### 사이드 이펙트 체크
- 컴포넌트를 교체할 때 해당 컴포넌트가 사용하던 **모든 기능 경로**를 나열하고, 새 컴포넌트가 동일하게 커버하는지 확인한다
- Store 상태를 바꿀 때 해당 상태를 읽는 **모든 컴포넌트/훅**을 grep으로 확인한다
- dead code 제거는 기능 검증 완료 후에만 한다

### 과거 사고 사례
- 2026-03-29: RT branch를 드로어로 전환하면서 드로어에 RT 지원이 없는 상태에서 full view 진입점 제거 → RT 기능 전체 사라짐. **대체 경로가 없는데 기존 경로를 제거한 것이 원인.**

---

## 16. 코딩 컨벤션

- **한국어 응답**: 사용자 대면 텍스트는 한국어, 코드/경로/식별자는 원문
- **Zustand selector**: broad `useChatStore()` 금지, 개별 `useChatStore((s) => s.field)` 사용
- **Tauri command**: 인자는 `camelCase` (serde rename), 긴 실행은 `start_*` background 패턴
- **DB migration**: `add_column_if_missing`으로 idempotent, 버전 번호 순차 증가
- **에러 처리**: dev 단계에서 silent fallback 최소화, 명시적 경고/에러 표시
- **테스트**: vitest + jsdom (frontend, 55개), cargo test --lib (Rust unit, 53개)
- **4-engine parity**: 새 기능 추가 시 4개 엔진 모두에서 동작하는지 확인. 모든 엔진이 `build_normalized_prompt_with_budget()` 단일 경로 사용. Multi-agent context 전략: `docs/reference/multiAgentContextStrategy.md`
- **send 함수 패턴**: `runtimeSlice.sendWithEngine(engine)` + `branchSlice.sendThreadMessage()` 모두 `ENGINE_CONFIGS[engine]`로 command/event 매핑. 엔진별 함수 복사 금지. 레거시 동기 `send_with_*` 명령은 완전 제거됨
- **Settings 구조**: `settings/` 폴더에 섹션별 분리 파일. SettingsPanel은 thin shell

---

## 17. 문서 버전관리 규칙

- **Reference는 같은 파일 갱신** — 날짜 파일 복제 금지, `updated_at` 메타 갱신
- **Plan/Prompt는 작업 단위별 새 문서 허용** — 반드시 index.md 업데이트
- **브레인스토밍/비교 문서는 SSOT 아님** — `canonical: false` 명시, 구현 기준 문서와 분리
- **아카이브는 삭제보다 상태 변경** — `status: archived` + `superseded_by` 관계 명시
- 상세: `docs/reference/documentVersioningPolicy_2026-03-30.md`, `docs/reference/documentationNavigationModel_2026-03-30.md`

