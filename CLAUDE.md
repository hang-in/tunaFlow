# tunaFlow — Claude Code Handoff Document

> 최종 갱신: 2026-04-13 (세션 34 반영)
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

## 3. 아키텍처 요약

> **상세 참조**: `docs/reference/architecture-detail.md` — 프로젝트 구조, 레이아웃, RT 흐름, Store 구조, DB 스키마, 이벤트 모델. **해당 영역 작업 시에만 읽을 것.**

- **Project-centric**: 모든 데이터는 Project 소속. soft-hide 삭제.
- **Background execution**: `start_*` 커맨드 → 즉시 반환 → background subprocess → 이벤트 통지. DB = SSOT.
- **ContextPack (4-engine parity)**: `build_normalized_prompt_with_budget()` 단일 함수. Lite/Standard/Full auto mode. 동적 예산 배분.
- **Branch**: 대화 분기. shadow conversation. 드로어 또는 고정 패널.
- **RT**: Branch의 확장 모드. Sequential/Deliberative. 드로어 안에서 동작.
- **rawq**: sidecar daemon. 임베딩 기반 코드 검색. FS watcher 자동 재인덱싱.

---

## 5. 현재 상태 (세션 35 기준)

- **DB**: v30 / **Rust**: 232 tests / **Frontend**: 188 tests
- **현재 브랜치**: `main`
- **알려진 이슈**
  - RT 중간 스트리밍 미지원 (구조적 변경 필요)
  - window-state: dev 모드 Ctrl+C 종료 시 상태 미저장
  - JSONL 빠른 완료 감지 실패 (P1): PTY 응답 UI 미반영 간헐적 발생
  - bge-m3 CPU 스파이크 수정됨 (s35): ONNX 스레드 제한 + 세마포어 + 점진적 인덱싱
- **전체 이력**: `docs/reference/sessionHistory.md`

---

> **§6~9 (RT 흐름, Store 구조, DB 스키마, 이벤트 모델)**: `docs/reference/architecture-detail.md` 참조. 해당 영역 작업 시에만 읽을 것.

---

## 10. 세션 이력

> 전체 이력: `docs/reference/sessionHistory.md` — 새 세션 첫 요청 시 또는 과거 결정 맥락 필요 시 읽을 것

---

## 11. 다음 우선순위

### P0: 완료
- ~~PTY write queue (FIFO 순서 보장)~~ — s24 완료
- ~~ptySpawnLock → per-conversation Map~~ — s24 완료
- ~~PTY 완료 후 결과 미표시 (adoptBranch 충돌)~~ — s25 완료
- ~~adopt 중 스트리밍 메시지 소멸~~ — s25 완료
- ~~main 머지 준비~~ — s23 완료
- ~~리팩토링 v3 Tier 1(2.4+2.5) + Tier 2(2.6+2.8)~~ — s26 완료

### P1: 진행 대상
- ~~**리팩토링 v3 잔여**~~ ✅ — s27~s29 완료 (http_api/pty/executor/threadSlice/streamingUtils 모두 분리)
- ~~**ContextPack DB/assembly 완전 분리**~~ ✅ — `send_common/` 4파일로 이미 분리 완료 (context_loading/prompt_assembly/persistence/mod)
- ~~**브랜치 label git 스타일 slug화**~~ ✅ — s24~s27에서 `slugify_label()` 구현 완료
- **라이트 모드** ✅ — ~~oklch 기반 다크/라이트 토글 (디자인 시스템 Phase 2)~~
- ~~RT 전용 페르소나 설계~~ ✅ — s27 완료 (role_guidance() 4종)
- Project-per-window 아키텍처 (`docs/ideas/projectPerWindowIdea.md`) — VS Code 패턴
- KnowledgeLayer trait — 6번째 소스 추가 시 도입
- Insight Phase H(auto-export) ✅ / J(plan done→findings resolved) ✅ / I(tool-request:insight 핸들러) ✅ — s29 완료
- 온보딩 메타에이전트 (`docs/ideas/onboardingMetaAgentIdea.md`)

### P2: 후순위
- 디자인 시스템 확대 — text-tf-*/prose-* 토큰 점진 교체
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

### 세션 핸드오프 규칙

세션이 끝나거나 context 압축이 발생할 때:

1. **완료된 것과 안 된 것을 구분해서 기록** — "X 완료, Y는 미완 (이유: Z)" 형식. 모호한 "대부분 완료" 금지.
2. **변경한 파일 목록을 명시** — 파일 경로 + 변경 내용 요약. 다음 세션에서 grep 없이 파악 가능하도록.
3. **미완성 작업의 구체적 재개 지점** — "A 파일의 B 함수에서 C를 추가해야 함" 수준. "이어서 하면 됨" 금지.
4. **사이드이펙트 경고** — 변경으로 인해 다른 부분에 영향 가능성이 있으면 명시. "X를 바꿨으므로 Y를 확인해야 함".
5. **테스트 상태** — cargo test / vitest 결과. 실패한 것이 있으면 원인 + 재현 방법.
6. **sessionHistory.md는 과거 맥락 필요할 때만** — 매 세션 시작 시 전체를 읽지 않음. 특정 과거 결정이 필요하면 그때 참조.

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

## 17. 개발 도구 활용 규칙

아래 도구들이 설치되어 있다. 기본 도구(find, grep, cat) 대신 사용한다.

### 코드 검색/조작 (speedy-claude)

| 대신 | 사용 | 이유 |
|------|------|------|
| `find . -name` | `fd -e ts` | 64x 빠름, .gitignore 존중 |
| `grep -r` | `rg "pattern"` | SIMD 가속, 자동 멀티스레드 |
| `sed -i` | `sd 'old' 'new'` | BSD/GNU 차이 없음, 12x 빠름 |
| `cat file` | `bat file` | 구문 강조, 줄 번호 |
| `diff a b` | `difft a b` | AST 기반 구조 비교 |
| `ls` | `eza -la` | 아이콘, 색상, git 상태 |

멀티 파일 치환:
- 단순: `fd -e ts | xargs sd 'old' 'new'` (1 커맨드)
- 대화형: `ambr 'old' 'new'`
- **Read+Edit 루프 금지** — 한 번의 커맨드로 일괄 처리

### 프로젝트 분석 도구

| 도구 | 명령 | 용도 |
|------|------|------|
| **rawq** (v0.1.1) | `rawq search "키워드"` | 코드 시맨틱 검색 (임베딩 기반 하이브리드) |
| | `rawq map .` | AST 기반 코드베이스 구조 출력 |
| | `rawq daemon status` | daemon 상태 확인 |
| **code-review-graph** (v2.3.1) | `code-review-graph status` | 그래프 통계 (노드/엣지/파일) |
| | `code-review-graph detect-changes` | 변경 영향 분석 + risk score |
| | `code-review-graph update` | 증분 인덱스 업데이트 (변경 파일만) |
| **context-hub** | `chub search "react hooks"` | 라이브러리/프레임워크 문서 검색 |

### 사용 시 주의

- rawq daemon이 꺼져있으면 첫 검색이 수분 걸림 → `rawq daemon start --background` 먼저
- rawq/CRG는 인덱스가 오래되면 결과 부정확 → 대규모 리팩토링 후 `rawq index build` + `code-review-graph build` 재실행
- CRG `detect-changes`는 git diff 기반 → commit되지 않은 변경도 감지

---

## 18. 문서 버전관리 규칙

- **Reference는 같은 파일 갱신** — 날짜 파일 복제 금지, `updated_at` 메타 갱신
- **Plan/Prompt는 작업 단위별 새 문서 허용** — 반드시 index.md 업데이트
- **브레인스토밍/비교 문서는 SSOT 아님** — `canonical: false` 명시, 구현 기준 문서와 분리
- **아카이브는 삭제보다 상태 변경** — `status: archived` + `superseded_by` 관계 명시
- 상세: `docs/reference/documentVersioningPolicy_2026-03-30.md`, `docs/reference/documentationNavigationModel_2026-03-30.md`

