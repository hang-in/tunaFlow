# tunaFlow — Claude Code Handoff Document

> 최종 갱신: 2026-04-04 (세션 10 반영)
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

### ✅ 해결됨 (세션 5: 오케스트레이션 워크플로우 파이프라인)
- **Phase A**: DB v18 migration (plans 확장 + plan_events + plan 6개 컬럼), Rust 모델/commands 5개, TS 타입/API
- **Phase B**: `<!-- tunaflow:plan-proposal -->` 마커 파서 + PlanProposalCard (승격/수정요청/무시 3버튼)
  - 수정 요청: 피드백 입력 → 에이전트에게 재제안 요청 전송
- **Phase C**: ApprovalGate (승인→엔진 선택→Implementation Branch 자동 생성 / 검토 요청→의견 입력→Review Branch 자동 생성 / 보류)
  - Review Branch에서 plan-proposal 마커 감지 → "Plan에 병합" 버튼 → `replace_plan_subtasks()` 호출
  - Event timeline (plan_events) 표시
- **Phase D**: 승인 시 Implementation Branch 자동 생성 + Developer pre-implementation report 프롬프트 자동 전송
  - `<!-- tunaflow:impl-plan -->` 마커 → PlanCard 내 ImplPlanCard (파일/의존성/위험 표시 + "구현 시작" 게이트)
  - `<!-- tunaflow:impl-complete -->` 마커 감지 → "Review RT 시작" 버튼
- **Phase E**: `run_project_tests` Tauri command (cargo/vitest 자동 감지 + 결과 파싱)
  - Review RT 자동 실행 (2-agent, plan context + impl summary + test 결과 포함)
  - `<!-- tunaflow:review-verdict -->` 마커 감지 → ReviewVerdictCard (pass→done / fail→rework / conditional→사용자 판단)
  - Rework 루프: phase=rework → Implementation Branch로 복귀
- **인프라**: `link_plan_branch` command, `workflowOrchestration.ts` 유틸, ImplPlanCard/ReviewVerdictCard/MergeBranchButton 컴포넌트
- Rust 60 unit tests, Frontend 66 tests (파서 테스트 포함)

### ✅ 해결됨 (세션 6: 내부 품질 + 엔진 확장 + tool 가시화)
- **zod 스키마 검증 인프라**: `src/lib/schemas/` 5개 워크플로우 스키마 SSOT (planProposal, implPlan, reviewVerdict, subtaskDone, implComplete). `planProposalParser.ts` 3개 파서에 zod 검증 추가 (graceful degradation). 17개 스키마 테스트 추가.
- **OpenAI Compatible 엔진 (Ollama)**: `openai_compat.rs` — Ollama/LM Studio/vLLM 범용 HTTP 클라이언트. SSE 스트리밍 + tool call 지원 + tools 미지원 모델 자동 fallback. `base_url` 교체로 모든 OpenAI 호환 백엔드 지원. 전체 엔진 통합 (command, RT executor, model discovery, frontend ENGINE_CONFIGS).
- **Tool Steps 가시화**: 3개 CLI 엔진(claude/codex/gemini)에서 중간 이벤트(thinking, tool_use, command_execution, file_change) 구조화 전송 (`__STEP__:{json}` 프로토콜). `toolStepsStore.ts` 경량 store + `ToolStepsView.tsx` 컴포넌트. 스트리밍 중 3줄/5줄 스크롤, 완료 후 접힘 요약. `progressContent`에 JSON 저장 (lazy-load, 검색/ContextPack 미사용).
- **Silent error 표면화**: 12개 파일에서 `catch { /* silent */ }` → `console.error` + `toast.error`. 워크플로우 버튼(Dev 시작, 완료, Rework, 병합, 상세설계 요청 등) 실패 시 사용자에게 에러 표시.
- **Developer 프롬프트 수정**: 결과 문서 이중 작성 방지 (tunaFlow 자동 생성 명시, Developer 직접 작성 금지). 마커는 채팅 메시지에만 포함 (파일에 쓰지 않음). 검증 범위 제한 (변경 파일만 확인, 전체 프로젝트 타입 체크 결과 주장 금지).
- **Reviewer 프롬프트 수정**: result report는 자동 생성이므로 문서 품질로 fail 주지 않도록. 프로젝트 전체 체크 실패는 Developer 실패가 아님.
- Rust 60 unit tests, Frontend 96 tests (스키마 17개 포함)

### ✅ 해결됨 (세션 7: 문서 정리 + 장기기억 Phase 1-3)
- **문서 정리**: `docs/plans/index.md` 전면 재분류 (완료 47개, 부분 21개, 보류 13개, 진행 예정 27개). `futureWorkBacklog.md` 동기화. CLAUDE.md §10 113줄→10줄 압축.
- **주제별 메모리**: `SUMMARY_PROMPT` → JSON 배열 출력, 토픽별 다중 행 저장 (graceful fallback). `load_compressed_memory_topics()` + `format_topics_as_section()`.
- **운영 보강**: `provenance` (auto/manual) + `model_used` 기록. `force_recompress_memory` 커맨드. TracePanel에 토픽 수/재압축 버튼 표시.
- **자동 세션 발견**: `session_links` 테이블 (DB v21). FTS5 기반 관련 대화 자동 발견 + 수동 핀. `send_common.rs`에서 crossSessionIds 비었을 때 자동 로드. CrossSessionPanel 리팩토링 (auto/pinned/available 3섹션).
- DB v21 (topic columns + provenance + session_links), v22 (conversation_chunks + BLOB 임베딩).
- **Vector DB**: rawq embed CLI 활용 (새 의존성 없음), conversation_chunks 테이블, brute-force cosine 검색, FTS5+Vector 하이브리드 병합, session_discovery 벡터 시그널 추가.
- **rawq 고도화**: SearchOptions + search_with_options (rerank/text-weight/rrf-weight), prompt_needs_rawq 완화
- **Doom Loop 감지**: plan_events 기반 review_failed 3회 → 자동 subtask_review 에스컬레이션
- **채팅 가독성**: 워크플로우 프롬프트 ASCII→마크다운 (8개), 폰트/대비/간격/prose 개선
- **워크플로우 스킬 자동 주입**: phase→스킬 매핑 (appStore), effectiveSkills = 수동 ∪ phase 자동, Settings UI
- **마커 strip**: syncResultReport에서 tunaflow 마커 제거, Reviewer 템플릿 모순 해소
- **chops ContextPack 자동 주입**: context-hub search를 프롬프트 키워드로 호출 → Standard+ 모드 자동 삽입
- Rust 79→84 tests (세션 8-9에서 증가), Frontend 96 tests. DB v22→v25 (세션 10에서 v25).

### ✅ 해결됨 (세션 8-9: 이벤트 격리 + RT 전면 수정 + 스트리밍 안정화)
- **이벤트 격리**: ChunkPayload에 conversationId 추가, 5엔진 10 emit 지점 + frontend 리스너 필터링
- **스트리밍 보호**: isStillActive() 가드, _staleConversations 네비게이션 복귀 관리
- **스트리밍 race condition 근본 해결**: pendingChunk=null before cleanup (flushChunk의 set(status:'streaming')이 done을 덮어쓰는 Zustand 배칭 race)
- **Virtuoso re-render**: messagesRef → context prop (데이터 변경 시 visible item 자동 갱신)
- **identity 모델명**: build_identity_block에 model 파라미터 추가
- **async command**: compress/refresh/index/force_recompress → spawn_blocking
- **RT tokio panic**: RT command 4개 pub fn → pub async fn
- **RT 라운드 번호**: next_round_number() + 1 이중 가산 제거
- **RT Participant Status**: store로 이동 + conversationId 스코핑 + 리스너 ���이밍 해결
- **RT ContextPack 주입**: 상용 auto / 로컬 lite(15k cap) + RtContextCache 캐싱 (N회→1-2회)
- **RT 로딩 상태**: 토픽 + "Waiting for participants..." 스피너
- **ollama 엔진**: 5곳 하드코딩 엔진 목록에 ollama 추가
- **메시지 duration/token**: agent:completed에 메타데이터 추가 + trace_log LEFT JOIN (DB 영속)
- **Trace duration 음수**: ms/s 단위 통일
- **Persona 중복**: profile.label === persona.name 체크
- **콘솔 경고**: button nesting, parser validation, fs watcher → debug 레벨
- **SQLite**: synchronous=NORMAL, busy_timeout=5000, DB v23 (trace_log.message_id)
- Rust 84 tests, Frontend 96 tests. DB v23.

### ✅ 해결됨 (세션 10: 스킬 고도화 + 워크플로우 에이전트 고도화)
- **Trace Phase 1**: tok/s 스파크라인 + 컨텍스트 윈도우 % 프로그레스바 + RuntimeStatusBar % 배지
- **스킬 4-layer**: A(프로젝트 자동 감지) + B(프로젝트별 영속) + C(프롬프트 동적 활성화) + D(Persona recommendedSkills)
- **멀티툴 스킬 스캔**: chops 포팅 (MIT) — 12개 도구 경로 + Claude 플러그인 스킬 수집
- **스킬 레지스트리**: skills.sh API 검색 + 다운로드 설치 + 프로젝트 스킬팩 UI
- **임베딩 지연 최적화**: is_daemon_ready() 가드 + 단순 쿼리 벡터 검색 스킵
- **B안 (subtask 타겟 rework)**: ReviewVerdictSchema.failedSubtaskIds + rework 프롬프트 대상 필터링
- **code-review-graph 통합**: CLI query/impact 추가 + Rust sidecar + ContextPack 주입 + agent:completed 시 auto update
- **Architect/Developer/Reviewer 고도화**: PLATFORM_TIER0 역할별 규칙 + 역할 템플릿 갱신 (tool-request, failedSubtaskIds, graph, 자가 검증)
- **전역 selectedProfileId 제거**: _convEngineMap만 SSOT — 리뷰 브랜치 후 프로필 잔존 버그 근본 해결
- **마커 기반 멀티턴 도구 호출**: `<!-- tunaflow:tool-request:TYPE:QUERY -->` — docs/rawq/graph/plans 4종
- **후속 플랜 인프라**: plans.parent_plan_id (DB v25) + tool-request:plans:completed
- **context-hub 수정**: chub 바이너리 인식 + --cli-version + JSON 파싱 호환
- **Rework 이력 주입**: 이전 review_failed findings 누적 포함
- **syncResultReport 클린**: rework 후 마지막 Rework 프롬프트 이후 메시지만 사용
- **Plan Hints 강화**: subtask별 ✅/🔧/⬜ + 그룹/의존성 표시
- **병렬 서브태스크 그룹**: DB v24 (depends_on + parallel_group)
- **피드백 판단**: 2회 실패 시 findings 파일 겹침 → design_review_suggested 이벤트
- **메타에이전트 경량**: requestPlanRevision에 detect_project_stack 자동 주입
- **코드 리뷰 버그 수정**: background catch → console.error, failCount === 2 → >= 2
- Rust 84 tests, Frontend 96 tests. DB v25.

### 기타 알려진 이슈
- window-state: dev 모드 Ctrl+C 종료 시 상태 미저장 (X 버튼으로 닫아야 함)
- Rust 84 unit test + Frontend 96 test이나, integration test 부재
- 긴 multi-agent 대화 (24+ 메시지) 실사용 검증 미완
- Tool steps: Gemini CLI 버전에 따라 `tool_use` 이벤트 미지원 가능
- RT INTENT 표시 오류 (이전 데이터 오염, 새 RT에서 재현 확인 필요)
- RT에서 ollama 단독 라운드 누락 가능성 (trace JOIN dedup 적용, 재검증 필요)
- RT 전용 페르소나 미구현 (participant_identity에 행동 지침 없음)
- 상세: `docs/reference/knownIssues_2026-04-04.md`

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

## 8. DB 스키마 (v25)

| 테이블 | 핵심 필드 |
|---|---|
| `projects` | key(PK), name, path, type, source, hidden |
| `conversations` | id, project_key(FK), label, mode(chat/roundtable), rt_config(JSON), usage_status |
| `messages` | id, conversation_id(FK), role, content, status, engine, model, persona |
| `messages_fts` | FTS5 가상 테이블 (v15, 트리거 기반 동기화) |
| `branches` | id, conversation_id(FK), label, status, checkpoint_id, mode(chat/roundtable), parent_branch_id, git_branch |
| `memos` | id, message_id, content, type, tags |
| `artifacts` | id, conversation_id, type, title, status, subtask_id |
| `plans` | id, conversation_id, title, status, phase, architect_engine, developer_engine, reviewer_engines, implementation_branch_id, review_branch_id, parent_plan_id |
| `plan_subtasks` | id, plan_id(FK), title, status, owner_agent, depends_on(JSON), parallel_group |
| `plan_events` | id, plan_id(FK), event_type, actor, detail, created_at (v18) |
| `trace_log` | id, conversation_id, trace_id, span_id, engine, context_mode, context_sections, context_length, context_truncated, usage_status |
| `agent_jobs` | id, conversation_id, message_id, engine, kind, status, error |
| `conversation_memory` | id, conversation_id(FK), summary, source_count, created_at, updated_at, topic, phase, message_range, provenance, model_used (v21) |
| `session_links` | id, conversation_id(FK), linked_conv_id(FK), score, method, created_at (v21) |
| `conversation_chunks` | id, project_key, conversation_id(FK), kind, root_message_id, text_preview, embedding(BLOB), created_at (v22) |

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

## 10. 세션 이력 요약

> 상세 내역은 §5 참조. 아래는 세션별 핵심 성과 압축.

| 세션 | 날짜 | 핵심 성과 |
|------|------|-----------|
| 1 | 2026-03-28~29 | Linear UI 리팩토링, 4-engine parity Wave 1+2, 드로어/Branch/RT 통합, Skills UI, Agent Profile/Persona, Artifacts 워크플로, Settings, rawq sidecar, 문서 IA 거버넌스 |
| 2 | 2026-03-30 | ContextPack 전체 파이프라인 (visibility/compression/budget/identity/memory), context-hub 연동, runtimeSlice/SettingsPanel 리팩토링, 108 tests |
| 3 | 2026-03-30 | Claude parity fix (unified `build_normalized_prompt_with_budget()`), auto mode +1 bias 수정, compression DB lock 3-phase 분리, agents.rs 1168→260줄 |
| 4 | 2026-03-31 | Multi-agent context 3-layer, retrieval 품질 튜닝, Gemini auto/fnm/nvm, streaming UX 정리, project scaffold, deps Phase 1-4.2, rawq fs watcher |
| 5 | 2026-04-01 | 오케스트레이션 워크플로 파이프라인 Phase A-E 전체 완료 (DB v18, 마커 파서 4종, Approval Gate, Test Runner, Review RT, Verdict, Rework 루프) |
| 6 | 2026-04-02 | zod 스키마 검증 인프라, OpenAI Compatible 엔진 (Ollama), Tool Steps 가시화, silent error 표면화, Developer/Reviewer 프롬프트 수정 |
| 7 | 2026-04-02~03 | 장기기억 4단계, Vector DB, virtuoso/cmdk, tokio async, rawq 고도화, 워크플로우 스킬/doom loop/가독성, 코드 리팩토링 Tier1, 실사용 검증 50+ 버그 수정 (model race/Virtuoso/marker/FTS5/rawq/Mutex/stagger) |
| 8-9 | 2026-04-03~04 | 이벤트 격리, RT 전면 수정 (async panic/라운드번호/ContextPack 주입+캐싱/participant status), 스트리밍 race condition 근본 해결, Virtuoso re-render, 메시지 duration/token 표시, trace_log JOIN (v23), SQLite PRAGMA, ollama 엔진 전면 추가 |
| 10 | 2026-04-04 | Trace Phase 1 (tok/s + context %), 스킬 A/B/C/D + 멀티툴 스캔 + 레지스트리 + 스킬팩, 임베딩 지연 최적화, B안 (subtask 타겟 rework), code-review-graph 통합, Architect/Developer/Reviewer 고도화 (PLATFORM_TIER0 + 역할 템플릿), 전역 profileId 제거, 마커 기반 멀티턴 도구 호출 (docs/rawq/graph/plans), 후속 플랜 인프라 (v25), context-hub chub 수정, 코드 리뷰 버그 수정 (DB v25, Rust 84 tests, Frontend 96 tests) |

---

## 11. 다음 우선순위

### ✅ 완료: 문서 정리 + 세션 연속성 (세션 7)
- `docs/plans/index.md` 전면 재분류 (완료 47개, 부분 완료 21개, 보류 13개, 진행 예정 27개)
- `docs/plans/futureWorkBacklog.md` 동기화 (세션 4b-6 완료 항목 반영)
- CLAUDE.md §10 세션 이력 113줄→10줄 압축

### ✅ 완료: 장기기억 Phase 1-4 (세션 7)
- **주제별 메모리** — JSON 배열 토픽 분할, graceful fallback, 다중 행 저장
- **운영 보강** — provenance/model_used 기록, force recompress, TracePanel 가시화
- **자동 세션 발견** — FTS5 기반 session_links, auto+manual+available UI
- **Vector DB** — rawq embed CLI 활용 (snowflake-arctic-embed-s 384차원), conversation_chunks 테이블 (v22, BLOB 임베딩), brute-force cosine 검색, FTS5+Vector 하이브리드 병합, 자동 세션 발견 벡터 시그널 추가

### ✅ 완료: 내부 품질 강화 (세션 6)
- zod 스키마 검증 인프라, OpenAI Compatible 엔진 (Ollama), Tool Steps 가시화
- Silent error 표면화, Developer/Reviewer 프롬프트 수정

### ✅ 완료: 오케스트레이션 워크플로우 파이프라인 (Phase A-E)
- **Phase A-E 전체 완료** — DB v18, 마커 파서 4종, PlanProposalCard, Approval Gate, Test Runner, 전체 자동화
- Chat→Plan 승격→Approval(3-way)→Implementation Branch(Developer 자동 호출)→Review RT(2-agent)→Verdict→Done/Rework 루프

### ✅ 완료: react-virtuoso + cmdk (세션 7)
- **react-virtuoso**: ChatPanel 가상 스크롤 전환 (Virtuoso + followOutput + scrollToIndex)
- **cmdk**: Cmd+K 커맨드 팔레트 (탭 전환, 대화 전환, 프로젝트 전환, 새 대화, 설정)

### ✅ 완료: tokio async RT (세션 7)
- `execute_round()`, `run_participant()`, `execute_sequential()`, `execute_parallel()` async 전환
- `run_participant()`: `std::thread::spawn` + `.join()` → `tokio::task::spawn_blocking`
- `execute_parallel()`: `std::sync::mpsc` → `tokio::sync::mpsc`, `std::thread::spawn` → `tokio::spawn`
- background 커맨드: `std::thread::spawn` → `tokio::spawn(async move { ... .await })`

### ✅ 완료: rawq 고도화 (세션 7)
- `SearchOptions` 구조체 + `search_with_options()` — rerank, token-budget, text-weight, rrf-weight 지원
- `prompt_needs_rawq()` 게이트 완화 — 코드 키워드 없어도 10자+ 프롬프트에 검색 포함
- 개념 쿼리 감지 → text-weight 0.8 + rrf-weight 0.7 (문서 검색 강화)
- 코드 쿼리 → text-weight 0.5 + auto rrf-weight (rawq 자체 판단)
- rerank 항상 활성화 (2-pass keyword overlap)

### ✅ 완료: 워크플로우 품질 강화 (세션 7)
- **Doom Loop 감지**: plan_events review_failed 3회 → subtask_review 자동 에스컬레이션
- **채팅 가독성**: ASCII 박스→마크다운 (8개 프롬프트), 폰트/대비/간격/prose-chat
- **워크플로우 스킬 자동 주입**: phase→스킬 매핑, effectiveSkills, Settings > Runtime UI
- **마커 strip**: syncResultReport에서 tunaflow 마커 제거, Reviewer 모순 해소
- **chops ContextPack 자동 주입**: context-hub 라이브러리 문서 → Standard+ 자동 삽입

### ✅ 완료: 실사용 검증 (세션 7, 6+ 풀사이클)
- 워크플로우 15+ 버그 수정, 장기기억 품질 튜닝, rawq 94초→500ms
- 모델 선택 경쟁 조건 해결 (resolveModel)
- 미해결: `docs/reference/knownIssues_2026-04-03.md` 참조

### ✅ 완료: 이벤트 격리 + 스트리밍 안정화 (세션 8-9)
- ChunkPayload conversationId, isStillActive() 가드, flushChunk race condition 해결
- RT 전면 수정 (async panic, 라운드 번호, participant status, ContextPack 주입+캐싱)
- Virtuoso re-render, 메시지 duration/token, trace_log JOIN, SQLite PRAGMA

### ✅ 완료: 스킬 고도화 + 워크플로우 에이전트 고도화 (세션 10)
- 스킬 4-layer (A/B/C/D) + 멀티툴 스캔 (chops 포팅) + skills.sh 레지스트리 + 프로젝트 스킬팩
- code-review-graph 통합 (CLI query/impact + Rust sidecar + ContextPack + auto update)
- 마커 기반 멀티턴 도구 호출 (docs/rawq/graph/plans 4종)
- Architect/Developer/Reviewer 역할 템플릿 전면 갱신
- 전역 selectedProfileId 제거, 후속 플랜 인프라 (v25), context-hub chub 수정

### 다음 세션 첫 작업
1. **실사용 검증** — 세션 10 기능을 tunaInsight에서 테스트 (마커 도구 호출, 스킬팩, 후속 플랜, CRG)
2. **온보딩 메타에이전트** — 프로젝트 최초 진입 시 분석 + 추천 (아이디어 단계)
3. **리팩토링** — `docs/ideas/codeReviewRefactoringIdea.md` 기준

### P1: RT 재검증
- RT INTENT 표시 오류 재현 확인 (새 RT에서)
- RT ollama 단독 라운드 누락 재현 확인
- RT 전용 페르소나 설계 (participant_identity에 행동 지침 추가)

### P1: 구조 개선
- **ContextPack DB/assembly 완전 분리** (논리적 2-phase 분리 완료, 파일 분리는 후순위)
- **KnowledgeLayer trait** — 6번째 소스 추가 시 도입

### P2: 후순위
- Gemini SDK 직접 통합 (보조 경로, CLI 기본 유지)
- smoke test 복구
- Trace Phase 2: Git 상태 + OTel 중첩 스팬

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
