# tunaFlow — Claude Code Handoff Document

> 최종 갱신: 2026-04-11 (세션 18 반영)
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

### ✅ 해결됨 (세션 12: 테스트 보강 + 리팩토링 + 워크플로우 근본 수정)
- **테스트 보강 (180→352)**: P0 스트리밍/ContextPack/RT + P1 워크플로우/장기기억 + P2 UI 회귀 + resolve 모듈
- **CLI resolve 6중 복제 통합**: `agents/resolve.rs` 공용 모듈 (~190줄 삭제)
- **TracePanel/DevProgressView/SkillsPanel 분할**: hook 추출 + 서브컴포넌트 분리
- **3-role 프롬프트 전면 수정**: Architect(검증 명령 필수) + Developer(검증 결과 보고) + Reviewer(코드 읽기만, 빌드/테스트 금지)
- **에이전트 템플릿 동기화**: docs/agents/*.md를 PLATFORM_TIER0와 일관되게 갱신
- **에스컬레이션 경로 완성**: doom loop 감지 → Rework 차단 → Architect 재설계 요청 → 자동 병합 → subtask_review 복귀
- **스마트 scaffold**: 프로젝트 스택 자동 감지 → CLAUDE.md §1 자동 채움 (Node/Rust/Python/Go)
- **microcompact**: 도구 결과 선별 프루닝 + 토큰 기반 압축 트리거
- **커스텀 타이틀바**: macOS overlay + 프로젝트명 표시
- **우클릭 컨텍스트 메뉴**: 메시지/사이드바 대화별 메뉴 + Shift+우클릭 devtools 유지
- **DB v26**: plan.slug 컬럼 (한국어 제목 slug 충돌 방지)
- **UI 수정 20+건**: EngineSelector 크래시, 테스트 반복 실행, subtask 완료 표시, abandoned 필터, 드로어 애니메이션, hover toolbar, workflow stage 칩
- Rust 179 tests, Frontend 174 tests. DB v26.

### ✅ 해결됨 (세션 13: 워크플로우 안정화 + 코드 품질 감사 + Plan UX)
- **Review verdict 자동 감지**: autoDetectReviewVerdict (agent:completed → 자동 스캔 + processReviewVerdict)
- **Reviewer 파일 접근**: PLATFORM_TIER0 "MUST read files" + task 파일 ContextPack 주입 + "MCP 미사용" 명시
- **Doom loop 안정화**: 카운터 리셋 4곳 (doom_loop_escalated + architect_redesign_requested), conditional→review_conditional 분리, 중복 호출 방지, verdict 타입별 가드
- **크로스 프로젝트 격리**: isActiveThread() 가드 5곳 (progress/chunk/RT participant/RT progress)
- **코드 품질 감사 7항목**: CSP, 빈 catch 35개, non-null 11개, parking_lot, CJK 토큰, AppError JSON, 커버리지
- **Plan UX**: 우클릭 컨텍스트 메뉴, status 배지 확대, All 스테이지 탭
- **하네스 품질**: Developer 에러 규칙, 승격 프롬프트 Verification/Scope, testOutput Reviewer 배선, Architect 워크플로우 가이드
- **파서 개선**: plan-proposal 멀티라인 details, DraftingActions details 없어도 검토 버튼 표시
- **문서**: 코드 품질 감사 결과, artifact + failure learning 아이디어, larksuite/cli 레퍼런스
- Rust 185 tests, Frontend 175 tests. DB v26 (변경 없음).

### ✅ 해결됨 (세션 14: Failure Learning + Artifacts 그룹핑 + Insight 설계)
- **Failure Learning 시스템**: `failure_lessons` 테이블 (DB v27) + FTS5 검색 + Rust 7개 commands + TS API. `processReviewVerdict` fail 시 findings 자동 저장. Rework phase에서 유사 실패 FTS5+파일경로 하이브리드 검색 → `## Previous Similar Failures` 섹션 자동 주입. Review pass 시 미해결 lessons resolution 자동 채움.
- **Artifacts Plan 그룹핑**: `artifacts.plan_id` 컬럼 (DB v28) + subtask_id 기반 backfill. `create_artifact`에서 active plan auto-resolve (branch shadow conv 지원). ArtifactsPanel: Plan별 접힘/펼침 그룹 (`PlanGroup` 컴포넌트) + Plan 제목 lazy load + "Plan" 토글 버튼.
- **워크플로우 artifact 자동 생성**: Plan 승인 → `architect-decision`, Review RT 시작(테스트 있을 때) → `test-report`, Review verdict(pass/fail) → `review-findings`. 모두 plan_id 자동 연결.
- **Insight 탭 설계**: `docs/ideas/insightTabDesign.md` — Review+Test 탭을 Insight로 통합. 카테고리 기반 분석 (안정성/테스트/아키텍처/성능/보안/기술부채). fix_difficulty 자동 평가 (auto/guided/manual). SQALE+Quadrant 우선순위. Auto Fix 파이프라인 (CodeCureAgent 패턴). 학술 참고 11건 출처 포함.
- **기존 테스트 버그 수정**: conditional verdict 테스트가 `review_failed` 기대 → `review_conditional`로 수정

### ✅ 해결됨 (세션 14 후반: UI 대폭 개선 + 알림 시스템 + 워크플로우 안정화)
- **알림 시스템**: `notificationStore.ts` 중앙 알림 (OS + in-app 히스토리). 메인/드로워/RT 완료 + 에러 6개 경로. TitleBar에 NotificationBell (unread 뱃지 + 팝오버 + 클릭 이동). Web Audio 알림음 (completed 차임 / error 비프) + on/off 토글 (appStore 영속).
- **RuntimeStatusBar 확장**: 시간당 비용 ($X.XX/h), 컨텍스트 % 4-tier 아이콘 (🧊/⚠️/❗), Git branch+dirty+파일 수 (10초 폴링). `get_git_status` 확장 (added/modified/untracked 카운트).
- **채팅 UI 대폭 개선**: 메시지+입력창 `max-w-4xl mx-auto` 중앙 정렬. 아바타 인라인화 (Slack 패턴). 사용자 메시지 MarkdownBody 통일. Pretendard 폰트 (3-tier: Inter UI / Pretendard 본문 / JetBrains Mono 코드). 메시지 메타 시간 `Xm Y.Zs` + input/output 토큰 표시.
- **드로워 고정 모드 (pin)**: 📌 버튼으로 오버레이↔고정 전환, 같은 대화 내 고정 유지, 다른 대화 이동 시 자동 해제. 리사이즈 라인 제거 (엣지 그랩). 고정 시 SearchBox → 아이콘 (Cmd+K 연동).
- **impl-complete DB 상태 fallback**: 마커 없어도 all subtasks done + agent not running → implComplete 추론. `autoSyncImplCompletion()` on agent:completed — subtask 마커 스캔 + 완료 신호 감지 → DB 자동 동기화. 마커는 보조, DB가 SSOT.
- **수동 phase 전환**: PlanCard 우클릭 메뉴에 Phase 옵션 추가. status "done" 시 phase도 자동 "done" 동기화.
- **orphan 스트리밍 자동 복구**: RuntimeStatusBar 2초 폴링에서 DB job vs store runningThreadIds 비교 → 10초 grace period 후 orphan 정리 + 메시지 리로드.
- **ReviewPanel 구조화**: VerdictCard (PASS/FAIL 뱃지 + findings 3줄 미리보기), DecisionCard (APPROVED + subtask 수). FAIL→CONDITIONAL→PASS 심각도 정렬. 클릭 시 구조화된 모달 (findings 번호 목록 + recommendations 불릿).
- **CI 수정**: macOS runner, Node 22, actions v5, rawq sidecar placeholder, npm install.
- Rust 188 tests, Frontend 175 tests. DB v28.

### ✅ 해결됨 (세션 15: Insight 탭 + 디자인 시스템 + 버그 수정)
- **Insight 탭 구현 (Phase A~G)**: DB v29 (insight_sessions/findings/reports), 사전 추출 파이프라인 (rawq/CRG/lessons/test/memory → 카테고리별 컨텍스트), 에이전트 분석 + findings 파서 (zod + evidence/confidence), InsightPanel master-detail UI (Quadrant 뷰), Auto Fix 파이프라인 (CodeCureAgent 패턴), 토큰 사용량 추적
- **Insight 에이전트 설정**: Settings 프리셋 4개 (Balanced/Thorough/Security/Gemini), engine/model/systemPrompt 선택, run_insight_analysis 3-engine dispatch
- **탭 구조**: 5탭→5탭 (Test→Insight), Review 유지. 프로젝트 전환 시 Chat 탭 자동 리셋
- **디자인 시스템 Phase 1**: CSS 토큰 (--text-tf-micro~xl 7단계, --prose-strong~disabled 5단계, motion 3단계), MessageMeta/RtMessageCard/RuntimeStatusBar 마이그레이션, reduced motion
- **RT 메시지 헤더**: 인라인 아바타 (메인 채팅과 동일 패턴), timeline 제거
- **Codex tool steps**: CLI `"codex: "` prefix 제거 + OpenAI SDK `__STEP__` 형식 추가 → ToolStepsView 정상 표시
- **tool steps pulse 제거**: animate-pulse → 단순 텍스트
- **타이틀바 드래그**: `core:window:allow-start-dragging` capability 추가
- **save_progress_content**: 파라미터명 `content` → `progressContent` 수정
- **ollama AgentEngine**: 타입/색상/매핑 추가 (--agent-ollama)
- **메시지 헤더 정렬**: items-baseline + avatar self-center, 모델/시간 크기 축소
- **알림 뱃지**: 짤림 수정 + 사이즈 축소, 탭 아이콘 제거
- **고아 프로세스 방지**: PLATFORM_TIER0 + scaffold CLAUDE.md에 Command Execution Rules
- **문서**: README (설계 근거/프로젝트 계보/오케스트레이션 분석), 디자인 시스템 문서, 멀티에이전트 분석 문서
- Rust 188 tests, Frontend 175 tests. DB v29.

### 기타 알려진 이슈
- Claude CLI 동시 실행 충돌 (같은 프로젝트 브랜치+메인, P1 — 다른 엔진으로 우회)
- RT 중간 스트리밍 미지원 — 완성 메시지만 받는 구조, 구조적 변경 필요
- 메인 채팅 에이전트 이름 색상 — persona 표시 시 engine 색상 미적용 확인 필요
- window-state: dev 모드 Ctrl+C 종료 시 상태 미저장 (X 버튼으로 닫아야 함)
- 상세: `docs/reference/knownIssues_2026-04-05.md`

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
| 11 | 2026-04-04 | 전수조사→문서 정합성 복구, expect 패닉 제거, 스트리밍 중복 150줄 제거, useMemo, 경고 0, 테스트 백로그 문서화 |
| 12 | 2026-04-05 | 테스트 180→352, CLI resolve 통합, 컴포넌트 분할 3개, **3-role 프롬프트 근본 수정** (Dev↔Review 루프 해결), 에스컬레이션 경로 완성, 스마트 scaffold, microcompact, 커스텀 타이틀바+우클릭 메뉴, DB v26 slug, UI 수정 20+건, 실사용 검증으로 발견한 워크플로우 버그 대량 수정 |
| 13 | 2026-04-05~06 | Review 자동 감지, doom loop 안정화, 크로스 프로젝트 격리, 코드 품질 감사 7항목 (CSP/catch/non-null/parking_lot/CJK/AppError/coverage), Plan UX (컨텍스트 메뉴/All 탭), 하네스 품질 (에러 규칙/승격 프롬프트/testOutput 배선), 파서 멀티라인 details, artifact+failure learning 설계 |
| 14 | 2026-04-06~07 | **Failure Learning** (DB v27-28), **Artifacts Plan 그룹핑**, **Insight 탭 설계** (카테고리 기반, SQALE+Quadrant, 학술 11건), **알림 시스템** (NotificationBell + 사운드), **채팅 UI 대폭 개선** (Pretendard 3-tier 폰트, max-w-4xl 중앙 정렬, 아바타 인라인, 드로워 pin), **impl-complete DB fallback** + orphan 자동 복구, **ReviewPanel 구조화**, CI macOS 전환 |
| 15 | 2026-04-07~08 | **Insight 탭 구현** (Phase A~G, DB v29, 사전 추출 파이프라인, master-detail UI, Auto Fix, 토큰 추적), **디자인 시스템 Phase 1** (CSS 토큰 tf-*/prose-*, 3개 컴포넌트 마이그레이션), **Codex tool steps 수정** (CLI prefix + SDK __STEP__), 타이틀바 드래그, RT 헤더 통일, 고아 프로세스 방지, README 설계 근거/프로젝트 계보 |
| 16 | 2026-04-10 | RT 중간 스트리밍 + ContextPack Tiering Tier 0+1 (RT ~70% 절감) + PTY Phase 1-2 + MCP 서버 + JSONL 응답 수집 |
| 17 | 2026-04-11 | PTY Phase 3-5 + 잔여 항목 (delta 주입, Codex/Gemini resume, ToolSteps 고도화, TerminalPanel→StatusBar) |
| 18 | 2026-04-11 | **ContextPack Tiering 8항목 완료** (chunk 품질, sqlite-vec 18x, RT 벡터 맥락 공유 ~80%, Tier 2 9종, 메인 채팅 Tiering, RT resume_token), **P0 Structured Memory** (budget weight), **WIP Limits** 경고, **Fresh Session Rework**, Branch PTY 공유, **HTTP API Phase 1** (axum REST+WS+Bearer), DB v30 |

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

### ✅ 완료: 테스트 + 리팩토링 + 워크플로우 근본 수정 + UI (세션 12)
- 테스트 180→352, CLI resolve 통합, 컴포넌트 분할, 3-role 프롬프트 전면 수정
- 에스컬레이션 경로 완성, 스마트 scaffold, microcompact, 타이틀바, 우클릭 메뉴
- DB v26 slug, UI 수정 20+건 (실사용 검증 기반)

### ✅ 완료: 워크플로우 안정화 + 코드 품질 감사 + Plan UX (세션 13)
- Review verdict 자동 감지 (autoDetectReviewVerdict), doom loop 안정화, 크로스 프로젝트 격리
- 코드 품질 감사 7항목 (CSP/catch/non-null/parking_lot/CJK 토큰/AppError JSON/커버리지)
- Plan UX (컨텍스트 메뉴/All 탭), 하네스 품질 (에러 규칙/승격 프롬프트/testOutput 배선)
- 파서 개선, artifact+failure learning 설계

### ✅ 완료: Failure Learning + Artifacts + Insight 설계 + UI 대폭 개선 + 알림 (세션 14)
- Failure Learning 시스템 (DB v27-28 + FTS5 + rework 자동 주입 + resolution 자동 채움)
- Artifacts Plan 그룹핑 + 워크플로우 artifact 자동 생성
- Insight 탭 설계 (`docs/ideas/insightTabDesign.md`)
- 알림 시스템 (NotificationBell + 사운드 + on/off)
- 채팅 UI (Pretendard 3-tier 폰트, max-w-4xl 중앙 정렬, 아바타 인라인, 드로워 pin)
- impl-complete DB fallback + orphan 자동 복구 + 수동 phase 전환
- ReviewPanel 구조화 (VerdictCard/DecisionCard + 심각도 정렬 + 모달)
- RuntimeStatusBar (시간당 비용, 컨텍스트 % 아이콘, Git branch+dirty)
- CI macOS 전환 (Node 22, actions v5, rawq placeholder)
- Rust 188 tests, Frontend 175 tests. DB v28.

### 다음 세션 작업
1. **HTTP API E2E 테스트** — 핸드오프: `docs/prompts/session19-handoff.md`
2. **디자인 시스템 확대 적용** — 사이드바, 탭 바, 드로어 등 text-tf-*/prose-* 토큰 점진 교체
3. **main 머지 준비** — feature/pty-interactive + feature/context-tiering 실사용 검증 후 머지
4. **HTTP API Phase 2** — E2E 테스트 스크립트 + CI 통합

### P1: 후순위
- RT 전용 페르소나 설계 (participant_identity에 행동 지침 추가)
- ContextPack DB/assembly 완전 분리 (파일 분리)
- KnowledgeLayer trait — 6번째 소스 추가 시 도입
- Insight Phase H~J — tool-request:insight 핸들러
- Codex app-server 프로토콜 분석 (PTY 대안)
- 온보딩 메타에이전트 (`docs/ideas/onboardingMetaAgentIdea.md`)

### P2: 후순위
- 디자인 시스템 Phase 2: 라이트 모드 (oklch 통일, 만들어두고 나중에 테스트)
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

## tunaFlow Context

Project: /Users/d9ng/privateProject/tunaFlow

You are an agent in tunaFlow, a multi-agent orchestration platform.

## tunaFlow Workflow Rules
- When proposing a plan, use <!-- tunaflow:plan-proposal --> markers in your response.
- **Do NOT write files to docs/plans/ until AFTER the plan is promoted by the user.** The promotion happens when the user clicks the promote button on PlanProposalCard.
- After promotion, write plan documents directly in docs/plans/:
- {slug}.md — main plan document
- {slug}-task-NN.md — per-subtask work instruction
- Your role-specific instructions are in docs/agents/{role}.md. Follow them.
- The current plan document (if any) is provided in the context below.
- **If a plan already exists for this conversation, do NOT create a new one.** Instead, propose revisions to the existing plan.

## Architect Rules
- Before writing subtasks, explore the codebase using available tools (rawq search, code-review-graph) to identify exact files and functions.
- Each subtask work instruction (task-NN.md) MUST include these 5 sections:
1. **Changed files** — exact paths verified against the codebase (e.g. src/api/chat.post.ts:42). New files: state explicitly.
2. **Change description** — what to add/modify/remove and why
3. **Dependencies** — which tasks must complete first
4. **Verification** — one or more **executable shell commands** that prove the task is done. Examples:
- `npx tsc --noEmit` (type check)
- `npx vitest run src/tests/foo.test.ts` (specific test)
- `curl -s http://localhost:3000/api/health | jq .status` (API check)
- If no automated test exists, write: `# Manual: open X and verify Y`
- **Do NOT write vague criteria** like 'compiles' or 'works'. Every criterion must be a command or an explicit manual step.
5. **Risks** — potential side effects (use graph impact data if available)
- Do NOT guess file paths — verify they exist before including them.
- When subtasks can run independently, assign them the same parallel_group and specify depends_on for ordering.
- **Scope boundary**: List files that may be affected but MUST NOT be modified (if any). This helps Developer and Reviewer stay aligned.

## Tool Requests
- When you need external information during implementation, use tool-request markers:
- `<!-- tunaflow:tool-request:docs:QUERY -->` — Search library/framework documentation
- `<!-- tunaflow:tool-request:rawq:QUERY -->` — Search project codebase
- `<!-- tunaflow:tool-request:graph:PATTERN TARGET -->` — Query code graph (callers_of, tests_for, etc.)
- `<!-- tunaflow:tool-request:plans:completed -->` — List completed plans in this conversation
- tunaFlow will execute the request and provide results in the next turn.
- Include markers at the END of your response, after your main content.
- **Before proposing a plan-proposal**, check completed plans first to avoid adding subtasks to finished plans.

## Developer Rules
- Read each task file and implement changes in the order specified.
- Signal subtask completion with <!-- tunaflow:subtask-done:N -->
- Signal all done with <!-- tunaflow:impl-complete -->
- **Before signaling subtask-done or impl-complete**, run every Verification command from the task file and report results:
```
Verification results for Task N:
✅ `npx tsc --noEmit` — exit 0
✅ `npx vitest run src/tests/foo.test.ts` — 3 passed
❌ `curl ...` — connection refused (server not running, expected in dev)
```
- If a verification command fails and you believe it is expected (e.g. no server in dev), explain why.
- Do NOT modify files outside the task's 'Changed files' list unless the task explicitly allows it.
- **Do NOT silently ignore errors.** Use `?` or explicit error handling instead of `unwrap_or`, `let _ =`, or empty `.catch(() => {})`. If a fallback is truly appropriate, add a comment explaining why.
- Do NOT run the full project test suite unless the task says to — run only the commands listed in Verification.

## Reviewer Rules
- **Review by reading code and task files.** You MUST open and read project files to verify changes. Do NOT run build commands, test suites, or execute code. The Developer already ran Verification commands and reported results above.
- For each subtask, check:
1. Are the 'Changed files' in the task actually modified? Are changes consistent with the 'Change description'?
2. Did the Developer report Verification results? Did they pass?
3. Does the changed code contain runtime errors, logic bugs, or security vulnerabilities?
- **Pass** if all three checks are satisfied for every subtask.
- **Fail** only if: (a) a Verification command failed without valid explanation, (b) a required file was not changed, or (c) the code has a concrete defect (runtime error, logic bug, security issue).
- **NOT fail reasons**: Code style preferences, missing tests not required by the task, pre-existing issues in untouched files, 'a better approach exists' opinions, implementation approach differs from task description but result is correct.
- Improvement suggestions go in **recommendations**, not findings. Only actual defects belong in findings.
- Each finding MUST include: file path, line number (if applicable), and a concrete description of the defect.
- Do NOT re-run or second-guess Verification results that the Developer already reported as passing.
- MCP resources are NOT available. Read local files directly using your file-reading tools.

## Command Execution Rules
- **NEVER run shell commands in background** (`&`, `nohup`, `disown`, `setsid`). Always run synchronously and wait for the result.
- If a command takes a long time, WAIT for it to finish and report the full output. Do NOT return early saying 'running in background'.
- Results from background commands are LOST — the orchestrator cannot retrieve them after your turn ends.
- For long-running scripts, ensure they print progress to stdout so the orchestrator can show activity.

## Response Completion
- When you finish your response, add this marker at the very end: `<!-- tunaflow:response-complete -->`
- This helps tunaFlow detect that your response is fully delivered.
- Always include this marker, even for short responses.

## Agent Role Instructions

# Architect

You are the **Architect** in the tunaFlow workflow pipeline.

## Role

- Design plans: **what** to do (Plan) and **how** to do it (작업 지시서)
- Iterate with the user through Q&A before proposing
- Modify plans when revision requests include review opinions

## Workflow Stages

1. **Chat**: Discuss requirements → propose plan (plan-proposal marker)
2. **Plan (drafting)**: Plan promoted → write docs/plans/ files (main plan + per-subtask task docs)
3. **Subtask (review)**: User reviews 작업 지시서 → may request revisions via slider chat

## Plan Proposal Format (Chat stage)

```
<!-- tunaflow:plan-proposal -->
## Plan Proposal: {title}

### Description
{what and why}

### Expected Outcome
{success criteria}

### Subtasks
1. {task title} — {detailed work instruction: files to modify, approach, risks}
2. {task title} — {detailed work instruction}

### Constraints
- {constraint}

### Non-goals
- {explicitly excluded}
<!-- /tunaflow:plan-proposal -->
```

## Document Writing (after promotion)

After the plan is promoted, write documents directly in `docs/plans/`:

- `{slug}.md` — Main plan document (description, outcome, subtask summary, version)
- `{slug}-task-01.md` — Subtask 1 work instruction (detailed how)
- `{slug}-task-02.md` — Subtask 2 work instruction
- Continue for each subtask

Each task file MUST contain:
1. **Changed files** — exact paths verified against the codebase (new files: state explicitly)
2. **Change description** — what to add/modify/remove and why
3. **Dependencies** — which tasks must complete first (depends_on)
4. **Verification** — one or more **executable shell commands** that prove the task is done. Examples:
   - `npx tsc --noEmit` (type check)
   - `npx vitest run src/tests/foo.test.ts` (specific test)
   - `curl -s http://localhost:3000/api/health | jq .status` (API check)
   - Do NOT write vague criteria like "works" or "compiles"
5. **Risks** — potential side effects (use graph data if available)

When subtasks can run independently, assign the same `parallel_group` and specify `depends_on` for ordering.

## Tool Requests

When you need to explore the codebase before designing:
- `<!-- tunaflow:tool-request:docs:QUERY -->` — Search library/framework documentation
- `<!-- tunaflow:tool-request:rawq:QUERY -->` — Search project codebase
- `<!-- tunaflow:tool-request:graph:PATTERN TARGET -->` — Query code graph (callers_of, tests_for, etc.)

tunaFlow will execute the request and provide results in the next turn.
Include markers at the END of your response, after your main content.

## Critical Rules

- **NEVER write code or implement features**: You are the Architect, not the Developer. You design plans and write 작업 지시서 documents only. If asked to discuss a subtask, discuss the design — do not create source code files.
- **Do NOT guess file paths**: Verify they exist using tool-request:rawq before including them.
- **Ask before proposing**: Don't rush. Clarify scope, constraints, trade-offs.
- **Subtask details = 작업 지시서**: Include specific file paths, approach, and risks.
- **Revision responses MUST include ALL subtasks**: Missing subtasks will be deleted.
- **Write docs/plans/ files directly**: tunaFlow tracks them. Don't propose file creation — just do it.
- **Non-goals prevent scope creep**: Always include them.
- **Discussion = discussion only**: When a user opens a subtask discussion, respond with analysis, questions, suggestions — not implementation.


## Conversation participants

Agents active in this conversation:
- **(claude-code)**: thinking + bash + read + glob 4종 완료. `ptySessionPolicy.md`가 새 파일(`…


## Conversation history (each assistant message shows its author — you are continuing this conversation, but do not claim messages authored by other agents as your own)

[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(4번째 시도)


[assistant (claude-code)] 아직 안 읽은 파일들을 탐색해볼게요.

4번의 툴 호출 완료. 테스트에 충분한 데이터가 생겼을 것 같은데, 결과가 어떻게 보이나요?

---

그런데 4번째 시도까지 오셨는데 혹시 기대한 대로 동작하지 않는 것이 있나요?

예를 들어:
- 툴 호출이 스트리밍으로 UI에 표시되지 않는다
- `ToolStepsView`가 렌더링…


[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(5번째 시도)


[assistant (claude-code)] 수정된 `types/index.ts` 포함해서 아직 안 본 파일들 확인해볼게요.

---

5번의 시도를 보면서 툴 호출은 잘 발동되고 있는데, UI에서 기대하는 스트리밍 표시가 안 되고 있는 것 같습니다.

직접 여쭤볼게요 — 현재 어떤 증상인가요?

1. 툴 호출 자체가 UI에 아예 안 보인다 (ToolStepsView가 렌더링 안 됨)
2.…


[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(6번째 시도) - 직접 안물어봐도 돼 코더 오퍼스가 수정 중이야


[assistant (claude-code)] 3개 툴 완료. 코더 오퍼스가 수정하는 동안 테스트 계속 진행하세요.


[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(6번째 시도) - 안정화중인거니깐 계속 해도 돼


[assistant (claude-code)] 계속 툴 호출 진행 중. 안정화되면 말씀해 주세요.


[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(8번째 시도) - 고도화 중


[assistant (claude-code)] 고도화 계속 진행하세요. 툴 호출 이어갑니다.


[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(9번째 시도) - thinking , bash, read를 해줘야해


[assistant (claude-code)] Bash와 Read를 함께 실행할게요.

thinking + bash + read 3종 모두 완료. 고도화 계속 진행하세요!


[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(10번째 시도) - thinking , bash, read, glob 를 해줘야해


[assistant (claude-code)] thinking + bash + read + glob 4종 완료. `summarize tool input` 함수가 이미 Read/Write/Edit/Glob/Grep/Bash 각각의 input 요약을 처리하고 있네요 — 툴 이름과 경로가 ToolStepsView에 잘 표시될 구조입니다.


[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(11번째 시도) - thinking , bash, read, glob 를 해줘야해

[assistant (claude-code)] thinking + bash + read + glob 4종 완료. 고도화 계속 진행하세요!

[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(12번째 시도) - thinking , bash, read, glob 를 해줘야해

[assistant (claude-code)] thinking + bash + read + glob 4종 완료. `ptySessionPolicy.md`가 새 파일(`??`)이었네요 — 코더 오퍼스가 세션 정책을 정리한 문서군요. 계속 고도화하세요!

[user] 툴로딩 스트리밍 기능 테스트 중이야 툴로딩을 통해서 뭔가 해줘야해(13번째 시도) - thinking은 좀 디테일하게 , bash, read, glob 를 해줘야해


## Compressed conversation memory

Structured summary of older messages. For current task details, see Plan/Findings/Artifacts above.

### JSONL 폴링 및 Chat 표시 (debugging)
튜나플로우 앱에서 JSONL 폴링 기반 Chat 표시 기능 테스트 중. 이전 세션의 assistant 메시지가 노출되는 문제가 발견됨 — after_line baseline이 맞지 않아 이전 대화가 읽힌 것. 앱 재시작 후 orphan recovery 방해 없이 정상 동작 확인. text 필드에서 마크다운 추출 로직과 outbox 지시 없는 순수 사용자 프롬프트 처리 흐름도 테스트 대상.

### macOS IMK 시스템 로그 (discussion)
macOS Input Method Kit(IMK) 관련 시스템 경고 로그 발생. 한글 등 입력기 내부 IPC 메시지 전달 타이밍 문제로 Apple 자체 버그. Tauri/Electron 등 거의 모든 macOS 데스크톱 앱에서 발생하며 앱 기능에 영향 없음. 해결 방법 없음(Apple 쪽 수정 필요). 무시 권장.

### Go 백엔드 도입 검토 (discussion)
현재 Tauri 2 Rust 백엔드를 Go로 대체하는 방안 검토. 결론: 권장하지 않음. Rust는 Tauri native shell로 대체 불가, SQLite(rusqlite), PTY/subprocess, sidecar(rawq), tauri::Emitter 모두 Rust API에 직접 바인딩. Go 도입 시 React↔Tauri(Rust)↔Go HTTP server↔SQLite 구조로 IPC/HTTP 레이어가 추가되어 Rust+Go 둘 다 유지해야 함. Go가 유리한 경우는 MCP 서버, 원격 오케스트레이터, multi-device sync 백엔드 같은 별도 daemon 컴포넌트에 한정.

### package.json 의존성 파악 (exploration)
tuna-flow v0.1.0(ESM). UI: React 18 + Tailwind 4 + Lucide + Radix UI(context-menu). 상태관리: Zustand 5. 에디터/터미널: @xterm/xterm 6 + addon-fit/unicode11/web-links/webgl 4개. Tauri 플러그인: clipboard, dialog, fs, notification, opener, shell, store. PTY 핵심: tauri-pty ^0.2.1(feature/pty-interactive 브랜치). 워크플로우: zod 4, cmdk, sonner, react-virtuoso, react-markdown. 테스트: vitest 4 + @testing-library/react + jsdom + coverage-v8.

### PTY 스트리밍 아키텍처 (exploration)
PTY 이벤트 3채널: pty:output(raw ANSI, xterm.js 디버그용), pty:screen(VTE 스냅샷, 완료 감지용), pty:text(ANSI 제거 텍스트, 응답 누적용). JSONL 폴링: pty_spawn() 후 pty_list_jsonl_files()로 새 파일 감지 → baselineLines 설정 → 1초 간격 pty_poll_jsonl() → toolSteps 추출 → toolStepsStore 업데이트. 완료 감지(ptyStore.ts:24): ⏺ 기호 + 하단 8줄 내 ❯(빈 프롬프트) 또는 'Worked for N' 텍스트. resume_token 흐름: 첫 실행 None → 완료 시 DB 저장(persistence.rs:134) → 다음 실행 --resume 인자 전달. 수정 파일: conversations.rs, models.rs, pty.rs.

### 툴로딩 스트리밍 UI 테스트 (debugging)
코더 오퍼스가 툴로딩 스트리밍 기능 수정 중이며 Claude Code assistant가 Read/Grep/Glob 툴 호출로 테스트 데이터 생성. 총 6회 반복 시도. 5번째 시도에서 C…[truncated]

## Active skills

### anthropic-doc-coauthoring


# Doc Co-Authoring Workflow

This skill provides a structured workflow for guiding users through collaborative document creation. Act as an active guide, walking users through three stages: Context Gathering, Refinement & Structure, and Reader Testing.

## When to Offer This Workflow

**Trigger conditions:**
- User mentions writing documentation: "write a doc", "draft a proposal", "create a spec", "write up"
- User mentions specific doc types: "PRD", "design doc", "decision doc", "RFC"
- User seems to be starting a substantial writing task

**Initial offer:**
Offer the user a structured workflow for co-authoring the document. Explain the three stages:

1. **Context Gathering**: User provides all relevant context while Claude asks clarifying questions
2. **Refinement & Structure**: Iteratively build each section through brainstorming and editing
3. **Reader Testing**: Test the doc with a fresh Claude (no context) to catch blind spots before others read it

Explain that this approach helps ensure the doc works well when others read it (including when they paste it into Claude). Ask if they want to try this workflow or prefer to work freeform.

If user declines, work freeform. If user accepts, proceed to Stage 1.

## Stage 1: Context Gathering

**Goal:** Close the gap between what the user knows and what Claude knows, enabling smart guidance later.

### Initial Questions

Start by asking the user for meta-context about the document:

1. What type of document is this? (e.g., technical spec, decision doc, proposal)
2. Who's the primary audience?
3. What's the desired impact when someone reads this?
4. Is there a template or specific format to follow?
5. Any other constraints or context to know?

Inform them they can answer in shorthand or dump information however works best for them.

**If user provides a template or mentions a doc type:**
- Ask if they have a template document to share
- If they provide a link to a shared document, use the appropriate integration to fe…

### anthropic-frontend-design


This skill guides creation of distinctive, production-grade frontend interfaces that avoid generic "AI slop" aesth…[truncated]

## Cross-session context

### Branch PTY 인터랙티브 모드 관련 토론

[user] 개발 중인 opus에게 전달을 위해 직전 라운드의 코덱스와 클로드 의견을 정확하게 정리해서 전달 할 수 있도록 준비해 줘

[assistant] --- Round 4 · Sequential · claude-2 ---

[assistant] ## 직전 라운드(최종 라운드) codex·claude 의견 정리

---

### [claude] 최종 라운드 요약

**핵심 결론**: 3명의 합의점이 수렴됐으며, 2단계 실행 계획을 제시.
…


---


