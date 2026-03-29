# tunaFlow — Claude Code Handoff Document

> 최종 갱신: 2026-03-29
> SSOT: `docs/reference/dataModelRevised.md` (도메인 모델), `docs/reference/implementationStatus.md` (구현 현황)

---

## 1. 프로젝트 개요

tunaFlow는 **다중 에이전트 오케스트레이션 IDE**이다. Tauri 2 + React + TypeScript + Rust + SQLite 기반.

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
│   │   ├── agents/         # CLI agent adapters (claude, codex, gemini, opencode, rawq, loader)
│   │   ├── commands/       # Tauri commands
│   │   ├── db/             # SQLite schema, migrations(v1-v12), models
│   │   ├── errors.rs       # AppError enum
│   │   └── guardrail.rs    # Context budget limits + truncation
│   ├── binaries/           # rawq sidecar binary (gitignored)
│   └── Cargo.toml
├── src/                    # React frontend
│   ├── components/tunaflow/  # UI 컴포넌트
│   │   ├── chat/           # MarkdownComponents, FileViewer, fileViewerContext
│   │   ├── context-panel/  # SkillsPanel, TracePanel, PlansPanel, ArtifactsPanel 등
│   │   ├── input/          # EngineSelector, ModelSelector, RoundtableControls, useSendActions
│   │   ├── message/        # MessageMeta, MessageActions, ProgressSurface
│   │   └── sidebar/        # ProjectsSection, ChatsSection, RoundtablesSection, BranchesSection
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

### 4.2 Background execution
- `start_*` 커맨드: DB 준비 후 즉시 반환, background thread에서 subprocess 실행
- 이벤트: `{engine}:progress`, `{engine}:chunk`, `agent:completed`, `agent:error`
- Frontend: fire-and-forget invoke + event listener 패턴
- DB = SSOT: event를 놓쳐도 `list_messages()`로 복구

### 4.3 Normalized ContextPack (4-engine parity)
- **모든 엔진이 동일한 context를 받음** — `build_normalized_prompt()` 공용 함수
- Claude: system prompt 분리 방식, non-Claude: inline prompt에 합침
- 포함 섹션: project, recent context, plan, findings, artifacts, skills, rawq, cross-session, thread inheritance
- rawq는 mode 독립 — `prompt_needs_rawq()` 기준으로 코드 신호가 있으면 항상 포함

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

---

## 5. 현재 상태와 알려진 이슈 (2026-03-29)

### ✅ 해결됨: 드로어 RT 기능 구현 완료

- `BranchThreadPanel`: RT 모드 감지 → `RoundtableView` 렌더링
- `openThread`: shadow conversation을 `conversations` 배열에 추가 (RT 감지 전제 조건)
- `sendThreadRoundtable` / `sendThreadRoundtableFollowup`: thread context RT 전용 함수
- `useSendActions`: threadMode + isRoundtable → thread RT 함수로 라우팅
- `NewMessageInput`: thread mode에서 shadow conv 기준 RT config 로딩
- `RoundtableView`: `conversationId` prop으로 thread conv 기반 running/telemetry 감지

### ✅ 해결됨: 사이드바 RT/Branch 계층 구조

- RT/Branch를 Chats 하위 트리로 표시 (기존 3-section → 1-section)
- 사이드바 [+] RT 생성 시 기존 채팅의 branch로 생성 (독립 conversation 폐기)
- 채팅 1개 → 자동 선택, 여러 개 → 드롭다운 선택
- checkpointId 없는 RT branch에서 Adopt 숨김
- `openThread`: 부모 conversation이 미선택 상태면 자동 로딩

### 기타 알려진 이슈

- 기존 smoke-sidebar/smoke-workspace 테스트 실패 (selector 전환 이후 store mock 불일치)
- Listener timeout: background thread crash 시 event listener가 영영 cleanup 안 될 수 있음
- trace_log context metadata는 `start_claude_stream`만 적용. 다른 엔진은 NULL
- window-state: dev 모드 Ctrl+C 종료 시 상태 미저장 (X 버튼으로 닫아야 함)
- `RoundtablesSection.tsx`, `BranchesSection.tsx` dead code (사이드바 통합 후 미참조)

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
| `runtimeSlice` | `runningThreadIds`, `messageQueue`, `sendMessage()`, `sendWithGemini()`, `sendRoundtable()` 등 |
| `assetSlice` | `memos`, `artifacts`, `skills`, `activeSkills` (persist), `crossSessionIds` |
| `engineModelSlice` | `engineModels`, `loadEngineModels()` |

### 주요 실행 패턴
- **메인 패널 전송**: `runtimeSlice.sendMessage()` → `start_claude_stream` + event listener
- **드로어 전송**: `branchSlice.sendThreadMessage()` → `start_*` + event listener (background)
- **드로어 RT 전송**: `branchSlice.sendThreadRoundtable()` → `start_roundtable_run` + event listener (threadMessages)
- **메인 RT 전송**: `runtimeSlice.sendRoundtable()` → `start_roundtable_run` + event listener (messages)
- **입력 라우팅**: `useSendActions({ threadMode })` — RT + threadMode → `sendThreadRoundtable`, RT → `sendRoundtable`, threadMode → `sendThreadMessage`

---

## 8. DB 스키마 (v12)

| 테이블 | 핵심 필드 |
|---|---|
| `projects` | key(PK), name, path, type, source |
| `conversations` | id, project_key(FK), label, mode(chat/roundtable), rt_config(JSON) |
| `messages` | id, conversation_id(FK), role, content, status, engine, model, persona |
| `branches` | id, conversation_id(FK), label, status, checkpoint_id, mode(chat/roundtable) |
| `memos` | id, message_id, content, type, tags |
| `artifacts` | id, conversation_id, type, title, status, subtask_id |
| `plans` | id, conversation_id, title, status |
| `plan_subtasks` | id, plan_id(FK), title, status, owner_agent |
| `trace_log` | id, conversation_id, trace_id, span_id, engine, context_mode, context_sections |
| `agent_jobs` | id, conversation_id, message_id, engine, kind, status, error |

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
- Gemini model discovery: `npm root -g` 기반 (fnm/nvm 호환)
- window-state: `CloseRequested` 시 명시적 save
- App icons: tunaDish tuna.png (전 플랫폼)

---

## 11. 다음 우선순위

### P0: 대규모 UI/UX 개선
- (다음 세션에서 진행 예정)

### P1: 코드 정합성
- `openBranchStream` dead code 정리 (UI에서 미호출)
- `RoundtablesSection.tsx`, `BranchesSection.tsx` dead code 삭제
- smoke test 복구 (store mock 업데이트)
- token/cost: DB 레벨 `usage_status` 컬럼 추가 (unavailable 구분)

### P2: 후순위
- Evaluation UI 연결 (backend 완료, frontend 미연결)
- Chat virtualization (200+ 메시지 성능 이슈 시)
- FTS 검색 (messages_fts 트리거 + UI)
- Context budget scaling (60k → 단계적 상향)

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
npx vitest run                # Frontend (69 tests)
cd src-tauri && cargo test --lib  # Rust unit tests

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
- **테스트**: vitest + jsdom (frontend), cargo test --lib (Rust unit)
- **4-engine parity**: 새 기능 추가 시 4개 엔진 모두에서 동작하는지 확인
