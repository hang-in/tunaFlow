# tunaFlow 구현 현황

최종 갱신: 2026-04-11 세션 19 (코드베이스 기준)
SSOT: `docs/reference/dataModelRevised.md`

---

## 기능 구현 상태

### Core

| 기능 | 상태 | 비고 |
|---|---|---|
| Project CRUD | done | list/create/get/hide(soft-delete) + 같은 경로 복원 |
| Conversation CRUD | done | list/create/get/delete + custom_label rename |
| Message CRUD | done | list/create/append/update_status/delete_pair |
| Message FTS5 search | done | messages_fts (v15) + 트리거 동기화 + search_messages command + CenterPanel SearchBox |
| Branch 기본 | done | list/create/adopt/delete/rename + parent_branch_id + git_branch |
| Branch depth navigation | done | 드로어 네비게이터, breadcrumb, VS Code 트리 |
| Memo CRUD | done | list/create/delete + branch_brief |
| Artifact CRUD | done | list/create/update_status/delete + subtask link + provenance |
| Skill 로딩 | done | ~/.tunaflow/skills/ 스캔 + vendor 그룹핑 + 검색/필터 |
| DB migrations v1-v30 | done | v13-v25 기존 + v26 plan.slug, v27 failure_lessons, v28 artifacts.plan_id, v29 insight_sessions/findings/reports, v30 vec_chunks(sqlite-vec) |

### HTTP API (axum, localhost:19840)

| 기능 | 상태 | 비고 |
|---|---|---|
| Health / Projects / Conversations / Messages | done | GET, Bearer token auth |
| Plans / Plan Events / Artifacts / Agents Status | done | GET, read-only |
| Create Project / Conversation / Branch | done | POST |
| Send Message (dryRun + agent exec) | done | POST, background agent via spawn_blocking |
| Delete Conversation / Branch | done | POST/DELETE |
| Branch Archive / Adopt / Rename | done | POST, adopt summary = all assistant messages |
| Roundtable Run | done | POST, sequential multi-engine, spawn_blocking |
| RT Cancel | done | POST, CancelRegistry integration |
| WebSocket Events | done | agent:completed, agent:error, roundtable:* bridge |
| Auth | done | UUID v4 Bearer token (per startup) |

### Multi-Agent (5-engine parity — UI, + OpenCode backend-only)

UI `ENGINE_CONFIGS` 기준으로 **5 엔진** 디스패치 (`claude` / `codex` / `gemini` / `ollama` / `lmstudio`). OpenCode 는 백엔드(`start_opencode_run`, roundtable executor, skills/identity) 는 완성돼 있으나 **프론트 `ENGINE_CONFIGS` 맵에는 등재 안 돼 있음** → 사용자 노출 경로 없음. 복귀 시 `src/lib/engineConfig.ts` 에 한 줄 추가하면 활성화.

| 기능 | 상태 | 비고 |
|---|---|---|
| Claude send + stream | done | ContextPack, resume token, background start_claude_stream |
| Codex send + stream | done | normalized prompt, JSONL synthetic streaming, background start_codex_run |
| Gemini send + stream | done | normalized prompt, stream-json, background start_gemini_stream |
| Ollama send + stream | done | OpenAI-compat (`start_openai_compat_stream`), UI 디스패치 O |
| LM Studio send + stream | done | OpenAI-compat (`start_openai_compat_stream`), UI 디스패치 O |
| OpenCode send (backend) | done | start_opencode_run, `opencode models` discovery, roundtable executor 연결됨 — **UI 디스패치 X** (ENGINE_CONFIGS 미등재) |
| Roundtable run/followup | done | Sequential + Deliberative, background execution. RT 참가자로는 OpenCode 포함 가능 |
| Unified send factory | done | runtimeSlice.sendWithEngine() — ENGINE_CONFIGS map, 중복 제거 |
| Identity framing | done | ## Identity 블록, profile/engine/persona 3층 분리, 혼합 표현 금지 |
| Message author attribution | done | per-message author 태그 [assistant:Profile (engine)] |
| Agent cancel | done | CancelRegistry thread-aware, stream/background 모두 지원 |

### ContextPack

| 기능 | 상태 | 비고 |
|---|---|---|
| Normalized prompt assembly | done | build_normalized_prompt_with_budget(), 전 엔진 공용 (claude/codex/gemini/ollama/lmstudio + opencode) |
| Mode system (Lite/Standard/Full) | done | mode-specific ModeProfile caps + resolution |
| Auto mode heuristic | done | 신호 기반 scoring → profile 자동 선택 + trace reason 기록 |
| Context budget control | done | Settings UI (mode selector + total cap slider) + appStore + backend override |
| Identity + Persona injection | done | ## Identity + ## Persona, persona_fragment 주입 |
| Recent context with authors | done | load_recent_messages_with_author, author attribution 태그 |
| Conversation retrieval (FTS5) | done | pair/anchor/brief chunk 재조립, scoring + dedup + overlap suppression |
| Compressed conversation memory | done | v17 conversation_memory, 12+ msg threshold, 구조화 요약 |
| Memory status model | done | not_generated/fresh/stale + provenance (source_count, timestamps) |
| Unified memory policy | done | priority 고정 + budget-aware fallback + overlap suppression |
| Threshold tuning | done | mode-aware threshold (retrieval/compressed) |
| Section budget breakdown | done | per-section chars 기록 → contextHash JSON → TracePanel 표시 |
| rawq post-processing | done | confidence filter(0.4+), dedup(±5줄), 300자 snippet, scope/confidence 표시 |
| rawq multi-resolution | done | top 2 full, next 2 skeleton, rest one-line reference |
| Import block folding | done | 3+ 연속 import → [N imports folded] |
| Jaccard block folding | done | cross-session 유사 블록 접기 (0.8+ threshold) |
| Markdown lightening | done | bold/italic 제거, 코드 블록 보존 |
| Typed compression | done | section별 압축 목표 (context=800, cross-session=600, findings=400) |
| context metadata | done | trace_log에 context_mode/sections/length/truncated 기록 (전 엔진 공통) |
| Trace visibility | done | TracePanel pills (active/skipped), mode badge, budget breakdown, RuntimeStatusBar hint |

### context-hub

| 기능 | 상태 | 비고 |
|---|---|---|
| CLI integration | done | health/search/get, source policy (bundled/local/private only) |
| Settings UI | done | 검색 input, 결과 리스트, 문서 미리보기 |
| Explicit handoff | done | Copy / Send to Context / Save as Artifact |

### Agent Profiles & Personas

| 기능 | 상태 | 비고 |
|---|---|---|
| Profile CRUD | done | engine/model/personaId/defaultSkills, appStore persistence |
| ProfileSelector | done | 드롭다운 + Custom fallback |
| Persona 7종 built-in | done | General/Reviewer/Tester/Architect/Implementer/Debugger/UX Critic |
| Persona Settings UI | done | priorities/behaviors/constraints/tone/outputStyle/promptFragment |
| Runtime persona binding | done | promptFragment → ## Persona section, 전 엔진 공통 |
| Applied config visibility | done | message.persona DB 저장 → MessageMeta 표시 |

### Plan / Evaluation

| 기능 | 상태 | 비고 |
|---|---|---|
| Plan CRUD | done | create/get/list/update_status/delete + subtask CRUD |
| Plan → ContextPack | done | active plan 요약 주입 |
| Plan → Artifact link | done | artifacts.subtask_id |
| Evaluation backend | done | eval_runs + eval_results, run_eval_agent (실제 에이전트 실행) |
| Evaluation UI | done | CreateRunForm, ExecuteButton, result cards, cancel/retry |

### Artifacts

| 기능 | 상태 | 비고 |
|---|---|---|
| Save as Artifact | done | MessageActions + RT 카드 → SaveArtifactDialog |
| Artifacts 탭 | done | 필터/정렬 + 통합 리스트 |
| Artifact 상세 모달 | done | content + status + copy/forward/delete |
| Provenance | done | source conversation/branch/RT + jumpToSource |

### Observability

| 기능 | 상태 | 비고 |
|---|---|---|
| trace_log write | done | 모든 엔진 + RT + context metadata |
| OTel span metadata | done | trace_id/span_id/parent_span_id/operation/engine/duration_ms/status |
| Context metadata in trace | done | context_mode/sections/length/truncated + section sizes (contextHash) |
| Memory policy trace | done | active/skipped pills, budget breakdown, Auto reason 표시 |
| Compressed memory status | done | TracePanel Brain 아이콘 + not_generated/fresh/stale badge |
| usage_status | done | exact/unavailable/unknown per engine (v16) |
| Trace export (JSON) | done | list_traces + export_traces_otel |

### UI / UX

| 기능 | 상태 | 비고 |
|---|---|---|
| Linear-inspired layout | done | Sidebar + CenterPanel 5-tab + RuntimeStatusBar |
| CenterPanel 5-tab | done | Chat/Plan/Artifacts/Review/Test |
| Settings 4-section (분리) | done | settings/ 폴더: AgentsSection, PersonasSection, RuntimeSection |
| Project-first startup | done | ProjectStartup 화면, projectless chat 불가 |
| 주 모니터 중앙 배치 | done | primary_monitor() 기준, window-state 복원 |
| Branch 드로어 | done | 모든 branch/RT는 드로어, 드로어 네비게이터 |
| RT 드로어 렌더링 | done | BranchThreadPanel → RoundtableView |
| FTS 검색 UI | done | CenterPanel SearchBox + 결과 드롭다운 + branch 결과 → 드로어 |
| 사이드바 트리 | done | VS Code 스타일, status dot, 삭제/인디케이터 위치 정리 |
| Markdown/CodeBlock | done | react-markdown + remark-gfm + 15줄 collapse + copy + lang label |
| FileViewer | done | inline 경로 감지 + 모달 preview |

### Infrastructure

| 기능 | 상태 | 비고 |
|---|---|---|
| rawq sidecar | done | daemon mode, background indexing, RawqIndexing guard |
| OpenCode discovery | done | `opencode models` CLI + `~/.opencode/bin/` 경로 |
| Gemini discovery | done | `npm root -g` 기반 |
| Codex discovery | done | `~/.codex/models_cache.json` |
| window-state | done | CloseRequested 시 명시적 save |
| Vite watch ignore | done | docs/**, README*.md, CLAUDE.md 제외 |

---

## Provider별 기능 비교

| 기능 | Claude | Codex | Gemini | OpenCode |
|---|---|---|---|---|
| Normalized ContextPack | O (system prompt) | O (inline) | O (inline) | O (inline) |
| Identity framing | O | O | O | O |
| Author attribution | O | O | O | O |
| Skills injection | O | O | O | O |
| Plan/Findings/Artifacts | O | O | O | O |
| rawq code context | O | O | O | O |
| Retrieval (FTS5 chunks) | O | O | O | O |
| Compressed memory | O | O | O | O |
| Cross-session context | O | O | O | O |
| Thread inheritance | O | O | O | O |
| Continuation | O (native resume) | O (context replay) | O (context replay) | O (context replay) |
| Streaming | O (native) | O (JSONL synthetic) | O (native) | partial (progress only) |
| Background execution | O | O | O | O |
| Token/cost tracking | exact | exact | streaming exact | N/A |
| Model discovery | fallback only | cache file | npm node | CLI `opencode models` |

---

## 테스트 현황

| 영역 | 테스트 수 | 도구 |
|---|---|---|
| Rust unit | 84 | cargo test --lib |
| Frontend smoke/integration | 96 | vitest + jsdom |
| **Total** | **180** | |

---

## DB 스키마 버전

| 버전 | 내용 |
|---|---|
| v1 | Core tables |
| v2 | resume_token |
| v3 | plans + plan_subtasks |
| v4 | artifacts.subtask_id |
| v5 | eval_runs + eval_results |
| v6 | trace_log OTel columns |
| v7 | plan_subtasks agent ownership |
| v8 | branches.mode |
| v9 | branches.subtask_id |
| v10 | agent_jobs |
| v11 | trace_log context metadata |
| v12 | (reserved) |
| v13 | projects.hidden |
| v14 | branch shadow conversation fix |
| v15 | messages_fts FTS5 + triggers |
| v16 | usage_status (trace_log + conversations) |
| v17 | conversation_memory |
| v18 | plan_events + plans 6개 컬럼 확장 |
| v19 | plans.version_major/minor |
| v20 | plans.revision |
| v21 | session_links + conversation_memory topic/provenance/model_used |
| v22 | conversation_chunks (벡터 임베딩 BLOB) |
| v23 | trace_log.message_id |
| v24 | plan_subtasks.depends_on + parallel_group |
| v25 | plans.parent_plan_id |

---

## 다음 단계 권장

### P0 (세션 11)
- 문서 정합성 유지 자동화 (DB 버전, 테스트 수 등 수동 문서 지표 최소화)
- 스트리밍 로직 공용 추출 (RuntimeSlice ↔ ThreadSlice 중복)
- 통합 테스트 추가 (스트리밍, RT, 워크플로우 흐름)

### P1
- 대형 컴포넌트 분할 (BranchThreadPanel, TracePanel)
- RT 전용 페르소나 설계
- ContextPack DB/assembly 파일 분리

### P2
- Long-term memory Phase 2: vector/embedding path 재평가
- context-hub auto ContextPack injection
- Chat virtualization (200+ 메시지)
