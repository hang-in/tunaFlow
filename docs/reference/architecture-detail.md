# tunaFlow 아키텍처 상세

> 이 파일은 CLAUDE.md에서 분리된 상세 참조 문서입니다.
> 필요할 때만 읽으세요: RT 수정, Store 수정, DB 변경, 이벤트 추가 시.

---

## 프로젝트 구조

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

## 아키텍처 핵심 원칙

### Project-centric
모든 데이터는 Project 소속. Store는 선택된 프로젝트의 데이터만 보유.
프로젝트 삭제는 soft-hide (hidden=1) — DB 데이터 보존, 같은 경로 재추가 시 복원.

### 레이아웃 구조 (Linear-inspired)
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

### Background execution
- `start_*` 커맨드: DB 준비 후 즉시 반환, background thread에서 subprocess 실행
- 이벤트: `{engine}:progress`, `{engine}:chunk`, `agent:completed`, `agent:error`
- Frontend: fire-and-forget invoke + event listener 패턴
- DB = SSOT: event를 놓쳐도 `list_messages()`로 복구

### Normalized ContextPack (4-engine parity)
- **모든 엔진이 동일한 context를 받음** — `build_normalized_prompt_with_budget()` 공용 함수
- Claude: system prompt 분리 방식, non-Claude: inline prompt에 합침
- 포함 섹션: identity, project, persona, recent context, compressed memory, plan, findings, artifacts, skills, rawq, cross-session, thread inheritance
- Context mode: Lite / Standard / Full (auto 판정)

### Branch = 대화 분기 공간
- git branch와 유사 — 독립 실험, RT 토론, 지식 정리 공간
- `branch:{branchId}` shadow conversation에 저장
- **모든 Branch는 드로어로 열림** — 고정 패널 모드도 지원

### RT = Branch의 협업 모드
- `branches.mode: "chat" | "roundtable"`
- **모든 RT는 채팅의 하위 branch로 생성**
- 드로어 안에서 RoundtableView, RT 컨트롤, 참가자 선택

### rawq = 필수 런타임 의존성
- sidecar binary, daemon 모드 (임베딩 모델 상주, 30분 idle timeout)
- `.gitignore` 존중 인덱싱, 비동기 (UI 블로킹 없음)

---

## RT (Roundtable) 실행 흐름

### RT 생성 경로

| 경로 | 설명 |
|---|---|
| 사이드바 [+] | `CreateRoundtableDialog` → 부모 채팅 선택 → RT branch 생성 → 드로어 |
| 메시지 RT 분기 | `CreateRoundtableDialog(checkpointId)` → RT branch 생성 → 드로어 |

### 실행 흐름
1. 드로어: `sendThreadRoundtable(prompt, participants, mode)` → `invoke("start_roundtable_run")`
2. Backend: `execute_round()` per participant (Sequential: 직렬, Deliberative: 병렬)
3. Events: `roundtable:participant_status`, `roundtable:progress`, `agent:completed`
4. Frontend: `list_messages()` 리로드 → `RoundtableView` 렌더링

### RT config
- `conversations.rt_config` (JSON) — `{ participants: [...], mode: "sequential"|"deliberative" }`
- RT branch는 shadow conversation ID (`branch:{branchId}`)를 키로 사용

---

## Frontend Store 구조

`src/stores/chatStore.ts`가 6개 slice를 합성:

| Slice | 핵심 상태 |
|---|---|
| `projectSlice` | `projects`, `selectedProjectKey`, `selectProject()` |
| `conversationSlice` | `conversations`, `selectedConversationId`, `messages` |
| `branchSlice` | `branches`, `threadBranchId`, `threadMessages`, `openThread()`, `sendThreadMessage()` |
| `runtimeSlice` | `runningThreadIds`, `messageQueue`, `sendWithEngine()` |
| `assetSlice` | `memos`, `artifacts`, `skills`, `activeSkills` |
| `engineModelSlice` | `engineModels`, `loadEngineModels()` |

### 주요 실행 패턴
- 메인 전송: `runtimeSlice.sendWithEngine(engine)` → `ENGINE_CONFIGS[engine].command`
- 드로어 전송: `branchSlice.sendThreadMessage()` → 동일 패턴
- RT 전송: `sendThreadRoundtable()` → `start_roundtable_run`

---

## DB 스키마 (v46)

> 버전별 마이그레이션 요약은 `docs/reference/implementationStatus.md` 의 "DB 스키마" 표 참조. 아래는 현재 스키마의 핵심 테이블 목록.

| 테이블 | 핵심 필드 |
|---|---|
| `projects` | key(PK), name, path, type, source, hidden |
| `conversations` | id, project_key(FK), label, mode(chat/roundtable), rt_config(JSON) |
| `messages` | id, conversation_id(FK), role, content, status, engine, model, persona |
| `messages_fts` | FTS5 가상 테이블 (트리거 기반 동기화) |
| `branches` | id, conversation_id(FK), label, status, checkpoint_id, mode, parent_branch_id, git_branch |
| `memos` | id, message_id, content, type, tags |
| `artifacts` | id, conversation_id, type, title, status, subtask_id, plan_id |
| `failure_lessons` | id, project_key, plan_id, file_path, pattern, finding, resolution |
| `plans` | id, conversation_id, title, status, phase, architect/developer/reviewer engines |
| `plan_subtasks` | id, plan_id(FK), title, status, depends_on(JSON), parallel_group |
| `plan_events` | id, plan_id(FK), event_type, actor, detail, created_at |
| `trace_log` | id, conversation_id, engine, context_mode, context_sections, context_length |
| `agent_jobs` | id, conversation_id, message_id, engine, kind, status, error |
| `conversation_memory` | id, conversation_id(FK), summary, topic, phase, provenance |
| `session_links` | id, conversation_id(FK), linked_conv_id(FK), score, method |
| `conversation_chunks` | id, project_key, conversation_id(FK), kind, text_preview, embedding(BLOB) |
| `insight_sessions/findings/reports` | Insight 분석 세션, 발견사항, 리포트 |
| `vec_chunks` | sqlite-vec vec0 가상 테이블 (float[384] cosine distance) |

---

## 주요 이벤트 모델

| 이벤트 | Payload | 발생 시점 |
|---|---|---|
| `claude:progress` | `{ messageId, text }` | thinking/tool_use 진행 |
| `claude:chunk` | `{ messageId, text }` | assistant 텍스트 누적 |
| `gemini:progress/chunk` | 동일 | Gemini streaming |
| `codex:progress/chunk` | 동일 | Codex streaming |
| `agent:completed` | `{ messageId, conversationId, engine }` | 실행 완료 |
| `agent:error` | `{ messageId, conversationId, engine, error }` | 실행 실패 |
| `roundtable:participant_status` | `{ conversationId, name, engine, model, round, status }` | RT 참가자 상태 |
| `roundtable:progress` | `Message` (full) | RT 참가자 응답 완료 |
| `rawq:indexing` / `rawq:indexed` / `rawq:error` | `RawqStatus` | 인덱스 상태 |
