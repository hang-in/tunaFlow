# tunaFlow 구현 현황

최종 갱신: 2026-03-26 (실제 코드 기준 검증)
SSOT: `docs/reference/dataModelRevised.md`

---

## 기능 구현 상태

### Core

| 기능 | 상태 | 비고 |
|---|---|---|
| Project CRUD | done | list/create/get |
| Conversation CRUD | done | list/create/get/delete |
| Message CRUD | done | list/create/append/update_status |
| Branch 기본 | done | list/create/adopt/delete/open_stream |
| Memo CRUD | done | list/create/delete |
| Artifact CRUD | done | list/create/update_status/delete + subtask link |
| Skill 로딩 | done | ~/.tunaflow/skills/ 스캔 |
| DB migrations v1-v6 | done | idempotent ADD COLUMN (v2/v4/v6) |

### Multi-Agent

| 기능 | 상태 | 비고 |
|---|---|---|
| Claude send + stream | done | ContextPack 5단계 조립, resume token |
| Codex send | done | lite context prefix |
| Gemini send | done | lite context prefix |
| OpenCode send | done | lite context prefix |
| Roundtable run/followup | done | per-round participant selection, /follow command |
| RT 모드 | done | Sequential + Deliberative (Independent 제거) |
| RT progress event | done | 참가자별 `roundtable:progress` emit |
| RT cancel | done | CancelRegistry thread-aware, participant 간 체크 |
| Claude stream cancel | done | stream line 간 체크 + child.kill() |
| 비스트리밍 agent cancel | UI only | subprocess 완료 대기, 프론트 상태만 해제 |
| Thread-aware cancel | done | CancelRegistry(Arc<Mutex<HashSet>>) conversation_id 기반 |

### Plan / Evaluation

| 기능 | 상태 | 비고 |
|---|---|---|
| Plan CRUD | done | create/get/list/update_status/delete + subtask CRUD |
| Plan → ContextPack link | done | active plan 요약을 system prompt에 주입 |
| Plan → Artifact link | done | artifacts.subtask_id (V4 migration) |
| Branch-scoped plan | done | UI scope 토글 + canonical conversation id 보정 |
| Evaluation harness | done (backend) | eval_runs + eval_results (V5), 6 commands. UI 미연결 |
| Capability registry | done (backend) | list_capabilities (skills + MCP tools). UI 미연결 |

### Observability

| 기능 | 상태 | 비고 |
|---|---|---|
| trace_log write | done | 모든 엔진 + RT에서 기록 |
| OTel span metadata | done | trace_id/span_id/parent_span_id/operation/engine/duration_ms/status |
| RT root/participant spans | done | root span + N participant spans (parent linkage) |
| Trace export (JSON) | done | list_traces + export_traces_otel |
| OTLP collector 전송 | not started | |

### UI

| 기능 | 상태 | 비고 |
|---|---|---|
| 3패널 레이아웃 | done | Sidebar + ChatPanel + ContextPanel |
| ContextPanel 분리 | done | 6개 서브패널 (context-panel/) |
| Plans UI | done | 생성/조회/상태변경/subtask + branch scope |
| RT participant toggle | done | 모드바에서 참가자 선택/제외 |
| Thinking placeholder | done | 모든 엔진에서 streaming 전 표시 |
| API layer | done | src/lib/api/ (plans/artifacts/memos) |

### 최근 구현 (2026-03-26 세션)

| 기능 | 상태 | 비고 |
|---|---|---|
| rawq CLI 실제 연동 | done | rawq 바이너리 직접 호출, 자동 인덱싱, 상태 UI |
| ContextMode (lite/standard/full) | done | 일반 대화 = lite, branch/agent = standard |
| rawq 조건부 skip | done | 코드 신호 없으면 rawq 미실행 |
| 모델 카탈로그 + !models | done | curated catalog, 엔진별 셀렉터 |
| 프로젝트 추가 UX | done | folder picker, path validation, 중복 방지, 기본 conversation |
| Agent collaboration Phase 1-3 | done | shared brief, findings, artifact handoff, ownership, follow-up |
| Progress-first streaming | done | Phase 1-3 완료 |
| Thread-local queue Phase A+B | done | runningThreadIds 복수 지원, 프로젝트간 병렬 |
| RT branch | done | branches.mode, RT분기, brief visibility, adopt summary |
| 자연어 handoff Phase A | done | alias 파서, source 우선순위, no-source guard |
| Markdown 렌더링 | done | react-markdown + remark-gfm + MarkdownComponents |
| Branch-git 기반 정리 | done (1차) | 모델 주석 강화, UI 표시 자리 마련 |
| Sidebar 프로젝트 트리 | done (1차) | Projects / Roundtables / Branches / Files 4섹션, 프로젝트 트리 기반 탐색 |

### 2026-03-27 세션

| 기능 | 상태 | 비고 |
|---|---|---|
| Workspace panel 3-mode | done | Plan / Artifacts / Trace. Phase A 완료 |
| Resizable panels | done | sidebar/workspace/drawer 너비 persist |
| Overlay drawer | done | center+workspace 위 overlay, backdrop blur |
| Thread/RT context inheritance | done | anchor + parent turns + RT inheritance |
| Conversation/Branch rename | done | custom_label + InlineRename + optimistic update |
| Harness summary | done | workflow stage chips + HarnessSummary + subtask↔branch link |
| Harness artifact grouping | done | Harness/Other 분리 + HarnessStrip |
| Chat object tabs | done | main + active branch tab |
| Linear UI tone | done | MessageItem, NewMessageInput, MarkdownComponents, RT, Drawer 전체 정리 |
| Files section (sidebar) | done | list_directory command + 2단 파일 트리 |
| Project-level branches | done | useProjectBranches 훅, 전 conversation branches 집계 |
| Last conversation restore | done | settings.json persist + AppShell init 복원 |

### 2026-03-27 세션 (후반)

| 기능 | 상태 | 비고 |
|---|---|---|
| Agent avatar icons | done | PNG per engine (claude/gpt/gemini/opencode) + AgentAvatar 공용 컴포넌트 |
| Syntax highlighting | done | react-syntax-highlighter + oneDark + lazy load |
| Code block copy + lang label | done | 복사 버튼 + 우측 하단 언어 배지 |
| Model discovery | done | Codex ~/.codex/models_cache.json + Gemini node discovery + fallback |
| Engine normalizeEngine | done | "claude-code" → "claude" 매핑 + AGENT_TEXT_COLORS |
| Enter=send, Shift+Enter=newline | done | NewMessageInput + BranchThreadPanel |
| Sidebar selection-centric | done | Projects flat select, Chats/RT/Branches/Files 현재 프로젝트 전용 |
| Sidebar VS Code tree style | done | TreeRow + indent guides + isParent depth 보정 |
| Non-streaming thread spawn | done | codex/gemini/opencode/claude 모두 std::thread::spawn으로 UI freeze 방지 |
| Click-outside dropdown close | done | NewMessageInput + BranchThreadPanel + MessageItem followup |
| RT creation dialog | done | CreateRoundtableDialog + participant/model/mode 설정 + sidebar [+] 진입 |
| RT config → first run | done | sessionStorage per-conversation + useSendActions 연결 |
| Message pair deletion | done | delete_message_pair command + FK cleanup + UI confirm |
| Adopt empty branch → delete | done | empty_branch 에러 → confirm 삭제 + drawer 정리 |
| Adopt engine/model tracking | done | 마지막 assistant의 engine/model을 adopt 요약에 포함 |
| Branch parent conv auto-load | done | openThread에서 부모 conversation 먼저 로드 |
| Scalability refactor Phase 1-5 | done | chatStore 6 slices, Sidebar 8 sections, Input 5 sub-components, Message 3 shared, agents.rs send_common |
| User message background | done | bg-white/[0.035] 말풍선 텍스트 배경 |
| Project-centric docs | done | 프로젝트 중심 설계 원칙 문서화 |
| Plans audit | done | 27개 plan 분류 (13 완료, 7 부분, 6 보류, 1 예정) |

### 미구현

| 기능 | 우선순위 | 비고 |
|---|---|---|
| FTS 검색 (messages_fts) | P2 | 스키마만 존재 |
| Workspace 자동 스캔 | P3 | 인메모리 개념 |
| Branch-git 실제 연동 | P3 | 필드 + UI 자리만 준비, 실제 git 명령 미실행 |
| Soft delete | P3 | 현재 hard delete |
| Evaluation UI | P2 | backend 완료, frontend 미연결 |
| Capability UI | P3 | backend 완료, frontend 미연결 |
| Sidecar 계층 | 보류 | direct-call로 충분, 필요 시 재검토 |
| 자연어 handoff 고도화 | P3 | 완전 자유 자연어 intent parser |
| Thread 모델 전면 통합 | P3 | branch 확장 방식으로 대체 중 |
| ~~프로젝트별 conversation 캐시~~ | 해당 없음 | 현재 프로젝트만 로드하는 것은 의도된 프로젝트 중심 설계. 전역 캐시 불필요 |

---

## Provider별 기능 비교

| 기능 | Claude | Codex | Gemini | OpenCode |
|---|---|---|---|---|
| ContextPack (full) | O | X | X | X |
| Lite context prefix | - | O | O | O |
| Resume token | O | X | X | X |
| Streaming | O | X | X | X |
| Token/cost tracking | O | O | X | X |
| OTel span recording | O | O | O | O |

---

## 테스트 현황

| 영역 | 테스트 수 | 도구 |
|---|---|---|
| Rust unit | 27 | cargo test |
| Rust DB integration | 13 | in-memory SQLite |
| Frontend API | 13 | vitest + jsdom |
| **Total** | **53** | |

CI: `.github/workflows/ci.yml` (cargo check/test + tsc + vitest + vite build)

---

## DB 스키마 버전

| 버전 | 내용 |
|---|---|
| v1 | Core tables (projects, conversations, messages, branches, memos, artifacts, trace_log) |
| v2 | resume_token columns on conversations |
| v3 | plans + plan_subtasks tables |
| v4 | artifacts.subtask_id column |
| v5 | eval_runs + eval_results tables |
| v6 | trace_log OTel columns (trace_id, span_id, etc.) |

---

## 다음 단계 권장

1. **Evaluation UI** — backend 완료, frontend 연결만 하면 RT 결과 비교 가능
2. **FTS 검색** — messages_fts 트리거 + UI 검색바
3. **rawq 실제 연동** — 프로젝트 코드 검색 → ContextPack 주입
4. **Capability UI** — list_capabilities → ContextPanel에 표시
