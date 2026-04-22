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
| `docs/reference/work-safety.md` | **작업 안전 규칙** — 코드/UI 변경 전 |
| `docs/reference/coding-convention.md` | **코딩 컨벤션** — 코드 작성 전 |
| `docs/reference/tool-usage.md` | **개발 도구 (fd/rg/rawq 등)** — 도구 사용 전 |

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

> 상세: `docs/reference/work-safety.md` — 코드/UI/스토어 변경 작업 **시작 전**에 읽는다.

요약 3줄:
- UI 진입점 변경 전에 대체 경로 작동 확인 (2026-03-29 RT 사고).
- 한 번에 한 경로만 수정 → 검증 → 다음 경로.
- `finalize_engine_run` 처럼 mutex re-entrant 가능한 자리는 특히 주의 (2026-04-22 deadlock).

---

## 16. 코딩 컨벤션

> 상세: `docs/reference/coding-convention.md` — 코드 작성/수정 전에 읽는다.

요약 3줄:
- 한국어 응답 / 코드·경로·식별자 원문.
- Zustand 는 selector 기반, Tauri sync command 중 UI hot-path 는 `async + spawn_blocking`.
- 4-engine parity: 모든 엔진이 `build_normalized_prompt_with_budget()` 단일 경로.

---

## 17. 개발 도구 활용

> 상세: `docs/reference/tool-usage.md` — 도구 사용 전에 읽는다.

요약 3줄:
- `find → fd` / `grep → rg` / `sed → sd` / `cat → bat`.
- 멀티 파일 치환은 `fd ... | xargs sd ...` — Read+Edit 루프 금지.
- 시맨틱 코드 검색은 `rawq search`, 그래프 영향 분석은 `code-review-graph detect-changes`.

---

## 18. 문서 버전관리 규칙

- **Reference는 같은 파일 갱신** — 날짜 파일 복제 금지, `updated_at` 메타 갱신
- **Plan/Prompt는 작업 단위별 새 문서 허용** — 반드시 index.md 업데이트
- **브레인스토밍/비교 문서는 SSOT 아님** — `canonical: false` 명시, 구현 기준 문서와 분리
- **아카이브는 삭제보다 상태 변경** — `status: archived` + `superseded_by` 관계 명시
- 상세: `docs/reference/documentVersioningPolicy_2026-03-30.md`, `docs/reference/documentationNavigationModel_2026-03-30.md`

