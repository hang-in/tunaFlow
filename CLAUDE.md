# tunaFlow — Claude Code Handoff Document

> 최종 갱신: 2026-03-30 (세션 2 반영)
> SSOT: `docs/reference/dataModelRevised.md` (도메인 모델), `docs/reference/implementationStatus.md` (구현 현황)

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

## 5. 현재 상태와 알려진 이슈 (2026-03-31 세션 3-4)

### ✅ 해결됨 (이전 세션 1-2)
- 드로어 RT 기능, 사이드바 계층 구조, Linear UI 리팩토링, rawq 안정화, 프로젝트 soft-delete, Agent Profile/Persona, Branch/RT 고도화, Artifacts 워크플로, Settings, 문서 IA 거버넌스
- 4-engine context metadata parity, ContextPack visibility, rawq 후처리, Compression, Context budget control UI
- context-hub 연동, Agent identity framing, Message author attribution, Compressed conversation memory
- runtimeSlice 팩토리, SettingsPanel 분할, deprecated isRunning 제거, OpenCode discovery

### ✅ 해결됨 (세션 3: Claude parity + dead code)
- Claude parity fix → unified `build_normalized_prompt_with_budget()` 전환
- Auto mode +1 bias 수정 (persona_fragment → explicit persona check)
- Lite mode retrieval/compressed thresholds 완화
- Compression DB lock 분리 (3-phase)
- Trace surface mode 포맷 호환 (`baseMode()` 헬퍼)
- agents.rs 1168→260줄 (레거시 6개 삭제 + prepare/finalize 공유 추출)
- branchSlice ENGINE_CONFIGS 통합

### ✅ 해결됨 (세션 4: multi-agent context + quality + deps)
- **Multi-agent context 3-layer**: participants meta + budget-based dynamic window + per-agent last-message guarantee. 문서: `docs/reference/multiAgentContextStrategy.md`
- **Retrieval 품질 튜닝**: FTS5 stopwords, scoring rebalance (fts 0.5/recency 0.2), overlap penalty 상향 (0.75), adaptive limit (Lite=3/Std=6/Full=10), content truncation 확대
- **Compressed memory 참여자 보존**: SUMMARY_PROMPT에 `## Participants` 섹션 필수화
- **Gemini Auto model**: discovery에 `auto` 기본 추가, preview 모델 "(용량 미보장)" 라벨
- **fnm/nvm 바이너리 경로**: Gemini + Codex resolve에 fnm/nvm 탐색 추가
- **Persona 리셋**: 엔진 변경 시 프로필 불일치 → persona 자동 초기화
- **스트리밍 UX 정리**: CLI가 thinking을 안 주므로 progress block 제거 → typing indicator만. progress_content는 DB에 lazy-load 방식으로 보존
- **ContextPack DB 분리 준비**: `load_context_data()` + `assemble_prompt()` 2-phase 분리 완료. DB/assembly 완전 분리 프롬프트 준비됨

### ✅ 해결됨 (세션 4 후반: project scaffold + deps)
- **프로젝트 scaffolding**: 프로젝트 생성 시 CLAUDE.md + docs/ 자동 생성. restore 시에도 동작
- **plan-first 규칙**: ContextPack identity block + CLAUDE.md 양쪽에 "승인 전 구현 금지" 규칙
- **Claude --permission-mode acceptEdits**: 편집 자동 승인, 터미널 확인 제거
- **Gemini Auto model**: discovery에 auto 기본, preview 모델 "(용량 미보장)" 라벨
- **fnm/nvm 바이너리 경로**: Gemini + Codex resolve에 fnm/nvm 탐색 추가
- **rawq 빈 인덱스 체크**: 인덱스 없으면 search skip (5초 타임아웃 제거)
- **rawq fs watcher**: tauri-plugin-fs watch + 에이전트 완료 시 re-index
- **스킬 선택적 주입**: 키워드 매칭 섹션만 포함 (8k→3k 절감)
- **RoundtableView 분할**: 468→212줄 + roundtable/ 폴더 3파일
- **스트리밍 UX 정리**: CLI thinking 미제공 → typing indicator만
- **의존성 도입 Phase 1-3**: clipboard-manager, shell, opener, fs (Tauri) + chrono, tokio (Rust) + react-virtuoso, cmdk, sonner (npm)
- **Phase 4-1 clipboard**: navigator.clipboard → native plugin 전환 (7파일)
- **Phase 4-2 sonner**: toast 알림 도입 (scaffold 알림)

### 기타 알려진 이슈
- window-state: dev 모드 Ctrl+C 종료 시 상태 미저장 (X 버튼으로 닫아야 함)
- Rust 57 unit test + Frontend 55 test이나, integration test 부재
- RT에서 `run()` 동기 사용 — progress 가시성 없음
- 긴 multi-agent 대화 (24+ 메시지) 실사용 검증 미완

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

## 8. DB 스키마 (v17)

| 테이블 | 핵심 필드 |
|---|---|
| `projects` | key(PK), name, path, type, source, hidden |
| `conversations` | id, project_key(FK), label, mode(chat/roundtable), rt_config(JSON), usage_status |
| `messages` | id, conversation_id(FK), role, content, status, engine, model, persona |
| `messages_fts` | FTS5 가상 테이블 (v15, 트리거 기반 동기화) |
| `branches` | id, conversation_id(FK), label, status, checkpoint_id, mode(chat/roundtable), parent_branch_id, git_branch |
| `memos` | id, message_id, content, type, tags |
| `artifacts` | id, conversation_id, type, title, status, subtask_id |
| `plans` | id, conversation_id, title, status |
| `plan_subtasks` | id, plan_id(FK), title, status, owner_agent |
| `trace_log` | id, conversation_id, trace_id, span_id, engine, context_mode, context_sections, context_length, context_truncated, usage_status |
| `agent_jobs` | id, conversation_id, message_id, engine, kind, status, error |
| `conversation_memory` | id, conversation_id(FK), summary, source_count, created_at, updated_at (v17) |

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

## 10. 2026-03-28~29 세션 주요 변경사항

### Engine Feature Parity (Wave 1+2 완료)
- `build_normalized_prompt()` — 4개 엔진 동일 context 조립
- rawq injection mode 독립화
- Codex JSONL synthetic streaming (`stream_run` + `codex:chunk`)
- Frontend: activeSkills/crossSessionIds 전 엔진 전달
- Token/cost: frontend N/A 표시 (backend DB 레벨 구분은 후속)
- Resume/continuation: Claude native + non-Claude context replay

### Chat UX (tunaChat parity 일부)
- CodeBlock: 헤더 바 (lang + lines + copy), 15줄 이상 collapse/expand
- FileViewer: inline code 파일 경로 감지 + 모달 preview (AppShell 레벨 공유)
- Message grouping: 연속 동일 발신자 아바타/이름 축소
- MessageActions 아이콘 축소

### 드로어/Branch 통합 (완료)
- 모든 Branch/RT는 드로어로만 열림 (openBranchStream UI에서 제거)
- BranchThreadPanel: RT 모드 → RoundtableView 렌더링, 일반 → MessageItem 렌더링
- sendThreadRoundtable/sendThreadRoundtableFollowup: thread context RT 전용 함수
- openThread: shadow conv를 conversations 배열에 추가 + 부모 conv 자동 로딩
- 사이드바: Chats 하위 트리로 RT/Branch 통합 (RoundtablesSection/BranchesSection 폐기)
- CreateRoundtableDialog: 항상 branch로 생성, 부모 채팅 선택 UI 추가
- checkpointId 없는 branch에서 Adopt 숨김

### Skills UI
- vendor 그룹핑 + 검색/필터 + 추천 프리셋 (Frontend/Review/OpenAI/Claude/MCP)
- SkillDef에 vendor/sourcePath 메타데이터 (backend `_meta.json` 파싱)
- active skills persistence (`lastActiveSkills` → appStore)
- snapshot published_at 표시

### Infrastructure
- rawq: sidecar bundle, daemon startup, background indexing (`start_rawq_index`)
- rawq: timeout 제거, RawqIndexing guard, listener cleanup
- Gemini model discovery: `npm root -g` 기반 (fnm/nvm 호환)
- window-state: `CloseRequested` 시 명시적 save
- App icons: tunaDish tuna.png (전 플랫폼)

### UI/UX 대규모 리팩토링 (Linear-inspired)
- ContextPanel 제거 → CenterPanel 5-tab 구조 (Chat/Plan/Artifacts/Review/Test)
- 사이드바: 프로젝트 드롭다운, Chat 트리 루트 (섹션 헤더 제거), Skills/Files
- Memo: toolbar 아이콘 + 팝오버 (메시지 스크롤 + 하이라이트)
- RuntimeStatusBar: 하단 전체 폭 (trace modal + rawq)
- Linear lch 색상 시스템 + 간격 스펙 + 폰트 통일 (13px/500)
- 프로젝트 soft-delete (hidden), 메인 채팅 삭제 방지, 검색 placeholder

### Agent Profile / Persona 시스템
- Settings: Agents (profile CRUD), Personas (7종 built-in), Skills, Runtime (실제 UI)
- Agent Profile: engine + model + personaId + defaultSkills → chat input binding
- ProfileSelector: 드롭다운 (profile list + Custom fallback)
- Persona: promptFragment → runtime prompt persona section 삽입 (4-engine parity)
- Applied config visibility: message.persona에 profile label 저장 → MessageMeta 표시

### Branch/RT 고도화
- Branch depth 탐색: 드로어 네비게이터 (중앙 배치, «» 오버플로), breadcrumb 전체 경로
- VS Code 스타일 트리: 선택한 branch만 펼침, 들여쓰기 10px
- Git 스타일 삭제: adopted/archived는 메시지 보존, active는 전체 삭제
- History 섹션: adopted/archived branch 분리 표시
- RT: 드로어 내 RT 분기, RT 테이블 뷰 전체 액션 (branch/RT/memo/forward/copy/save-artifact)
- window.confirm → Tauri ask() 네이티브 다이얼로그 전환
- 삭제 시 하위 adopted 경고 강화

### Artifacts 워크플로
- Save as Artifact: assistant 메시지/RT 카드에서 수동 승격 (SaveArtifactDialog)
- Artifacts 탭: 필터(All/Notes/Code/Specs/Harness), 정렬(Newest/Oldest/Title), 통합 리스트
- Artifact 상세 모달: 전체 content 읽기 + status 변경 + copy/forward/delete
- Provenance: source conversation/branch/RT 표시 + 클릭 시 이동 (jumpToSource)
- 카드 + 모달에서 subtask link 표시

### Settings 실제 구현
- Agents: profile CRUD + engine/model/persona/skills 편집 + appStore persistence
- Personas: 7종 built-in + 편집 UI (priorities/behaviors/constraints/tone/outputStyle/promptFragment)
- Skills: Settings로 이동 완료
- Runtime: rawq 상태, model catalog (+refresh), context budget 정책, background execution 설명

### 문서 IA 거버넌스
- documentationNavigationModel: 읽기 순서 + 문서 타입별 역할
- documentMetadataSchema: type/status/canonical/related 메타 기준
- documentVersioningPolicy: reference=같은 파일, plan=새 파일, brainstorm≠SSOT
- documentNamingRule: 짧은 파일명 + title/metas/index 보완
- CLAUDE.md §17에 규칙 요약

### 2026-03-30 세션 2 주요 변경사항

### ContextPack 고도화
- 4-engine context metadata parity: 모든 엔진이 trace_log에 context_mode/sections/length/truncated 기록
- TracePanel/RuntimeStatusBar에서 context 가시화 (mode badge, section pills, truncated 경고)
- rawq 후처리: SearchResult에 scope/confidence 추가, dedup/재정렬/300자 snippet
- Compression: section 유형별 압축 목표 (`maybe_compress_section_typed`)
- Context budget control: Settings UI + appStore 영속 + backend override 전달
- Conversation context framing: author attribution (per-message author 태그)
- Compressed conversation memory: v17 migration, 12+ 메시지 시 구조화 요약 + ContextPack 주입

### Agent Identity
- `## Identity` 블록: profile/engine/persona 3층 분리, 혼합 표현 금지
- Message author attribution: 과거 메시지 작성자 구분 규칙
- 사용자 언어 자동 감지 응답

### context-hub 연동
- `agents/context_hub.rs`: health/search/get CLI 호출 + source policy(bundled/local/private only)
- `commands/context_hub.rs`: 3개 Tauri commands
- Settings > Runtime: 검색/조회/문서 미리보기 + Copy/Send to Context/Save as Artifact handoff

### 코드 품질 개선
- runtimeSlice 팩토리 추출: 4개 중복 send → `sendWithEngine` (509줄 → 311줄)
- SettingsPanel 분할: 904줄 → 74줄 shell + settings/ 폴더
- deprecated `isRunning` 필드 제거
- OpenCode model discovery (`opencode models` CLI) + 바이너리 경로 추가
- Engine 선택 stale closure 버그 수정
- 사이드바 인디케이터/삭제 위치 교체
- 주 모니터 중앙 창 배치 (멀티모니터 대응)

---

## 11. 다음 우선순위

### P0: 실사용 검증 (기능 구현 후)
- 긴 multi-agent 대화 (4엔진 × 3턴 = 24msg) — dynamic window + participant meta 동작 확인
- Compressed memory 참여자 보존 — 긴 대화 후 에이전트 인식 검증
- Cross-conversation retrieval — 다중 대화 프로젝트에서 chunk 회수 확인

### P0: 오케스트레이션 워크플로우 파이프라인
- **Phase A: DB + 타입 + API** — plan phases, events, engine assignment. 프롬프트: `docs/prompts/2026-03-31/orchestrated_workflow_phase_a_prompt.md`
- **Phase B: Chat → Plan 승격** — 마커 파서 + PlanProposalCard
- **Phase C: Plan 승인 게이트** — 3-way + 검토 Branch
- **Phase D: Developer 실행계획 + 구현**
- **Phase E: 테스트 러너 + RT 리뷰**
- 전체 설계: `docs/plans/orchestratedWorkflowPipelinePlan.md`

### P1: 의존성 마이그레이션 (Phase 4 잔여)
- **Phase 4-3: react-virtuoso** — ChatPanel 가상 스크롤. 프롬프트: `docs/prompts/2026-03-31/dependency_migration_phase4_remaining_prompt.md`
- **Phase 4-4: cmdk** — 커맨드 팔레트

### P1: 구조 개선
- **ContextPack DB/assembly 완전 분리**: 프롬프트: `docs/prompts/2026-03-30/contextpack_db_separation_prompt.md`
- **Phase 5: tokio async** — `docs/plans/dependencyAdoptionPlan.md`

### P2: 후순위
- Vector DB Phase 1. 로드맵: `docs/reference/multiAgentContextStrategy.md`
- 긴 multi-agent 대화 실사용 검증
- smoke test 복구

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
npx vitest run                # Frontend (55 tests)
cd src-tauri && cargo test --lib  # Rust unit tests (57 tests)

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
