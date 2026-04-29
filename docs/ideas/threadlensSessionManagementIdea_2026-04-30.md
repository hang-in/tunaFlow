---
title: ThreadLens 패턴 차용 — 좌측 사이드바 세션 관리 기능 도입
status: idea (다음 cycle PR-ready)
created_at: 2026-04-30
canonical: false
priority: P2 (v0.1.5-beta release publish 후 진행)
external_reference:
  repo: https://github.com/hanityx/threadlens
  license: MIT
  compat: tunaFlow Apache-2.0 호환 ✅ (MIT → Apache-2.0 incorporate, attribution 필요)
  current_version: 0.3.0
related:
  - src/components/tunaflow/sidebar/ChatsSection.tsx
  - src/components/tunaflow/sidebar/DocsSection.tsx
  - src/components/tunaflow/sidebar/SidebarContextMenu.tsx
  - docs/ideas/branchViewerFutureIdea (만약 있다면)
trigger:
  reported_at: 2026-04-30
  reporter: 사용자 (d9ng)
  motivation: "좌측 사이드바 트리에 세션 관리 기능 도입 가능성 검토"
---

# ThreadLens 패턴 차용 — 세션 관리 기능

## 0. Context

`/Users/d9ng/privateProject/_research/_util/threadlens/` (MIT, 0.3.0) 의 multi-CLI 로컬 session 통합 관리 패턴 분석 후 tunaFlow 좌측 사이드바 (Chats / Docs / Files / Scratchpad) 에 도입 가능성. 다음 update cycle 적용 가능 idea 정리.

## 1. License + Attribution

- ThreadLens **MIT** — tunaFlow Apache-2.0 호환. incorporate 가능.
- 인용 시:
  - 코드 부분: file header 코멘트 (`// Pattern adapted from threadlens by hanityx (MIT). See docs/ideas/threadlensSessionManagementIdea_2026-04-30.md`)
  - PR description: 출처 링크
  - NOTICE 파일 한 줄 (대규모 인용 시): `This product references patterns from threadlens (MIT): https://github.com/hanityx/threadlens`

## 2. ThreadLens 6 핵심 feature 분석

| # | Feature | 명세 | 가치 |
|---|---|---|---|
| 1 | **Search** | 다중 CLI (Codex, Claude, Gemini, Copilot) session keyword 단일 검색 | session 발견 시간 단축 |
| 2 | **Transcript** | provider-specific folder 탐색 없이 full conversation 열기 | UX cohesion |
| 3 | **Safe cleanup** | backup + dry-run + confirm token (3 단계 안전망) | destructive action 사고 차단 |
| 4 | **Thread review** | Codex thread scope + related sessions + audit history | session 간 관계 가시화 |
| 5 | **Provider health** | provider status / session discovery / path-config 진단 | 환경 문제 self-diagnosis |
| 6 | **TUI** | terminal 모드 (keyboard-first) | power user UX |

추가 fact:
- **Roadmap 0.4**: session navigation, backup visibility, error guidance, session impact analysis
- monorepo (apps/web + packages) — standalone web app + Electron desktop + TUI

## 3. tunaFlow 좌측 사이드바 현황 (fact)

```
src/components/tunaflow/sidebar/
├── AddProjectForm.tsx         # 프로젝트 추가
├── ChatsSection.tsx           # ← "세션" 영역 (대화 목록)
├── DocsSection.tsx            # ← "문서" 영역 (Plan E 의 P3-Lite scope=all 적용 후)
├── FilesSection.tsx           # ← "파일" 영역
├── ScratchpadSection.tsx      # ← 스크래치패드
├── SidebarContextMenu.tsx     # 우클릭 메뉴
├── TreeRow.tsx                # 공통 row 컴포넌트
└── useProjectBranches.ts      # branch hook
```

사용자 발언 "문서/아카이브/세션" 중 **아카이브 컴포넌트는 현재 미존재** — Docs 의 sub-section 이거나 미구현 가능. 본 idea 적용 시 추가 또는 명확화.

## 4. tunaFlow vs ThreadLens — 차별점 분석

| 축 | tunaFlow | ThreadLens |
|---|---|---|
| Session 저장소 | **자체 DB (SQLite)** — conversation 통일 schema | **native CLI session 파일** (`~/.claude/`, `~/.codex/`, etc) |
| Multi-agent 통합 | ✅ (claude/codex/gemini/ollama/lmstudio 5종) | ✅ (Codex/Claude/Gemini/Copilot) |
| Live execution | ✅ (CLI agent spawn + 응답 stream) | ❌ (read-only viewer) |
| Search | tunaflow-mcp/search_documents + retrieval pipeline | full-text 단일 keyword |
| Backup/Cleanup | soft-hide 만 | dry-run + confirm token |
| Provider health | RuntimeStatusBar 일부 | dedicated screen |
| TUI | ❌ | ✅ |

핵심 차이: **tunaFlow 는 *manager + executor*, ThreadLens 는 *manager only***. tunaFlow 가 강점 더 큼 (live execution + 통합 DB) but ThreadLens 의 *safe cleanup / provider health / impact analysis* 패턴은 tunaFlow 부재.

## 5. Gap 분석

| Gap | tunaFlow 현황 | 도입 가치 |
|---|---|---|
| Native CLI session import | tunaFlow 외부 사용된 Claude Code / Codex session 무관 | **⭐⭐⭐ 높음** — tunaFlow 가 *모든 AI session unified hub* 가치 |
| Safe cleanup (backup + dry-run + token) | conversation 삭제는 단순 soft-hide. 잘못된 삭제 시 복구 어려움 | ⭐⭐⭐ |
| Session search UI | 검색 backend 있으나 사이드바 검색 UI 부재 (전체 conversation 통합 검색) | ⭐⭐⭐ |
| Provider health panel | RuntimeStatusBar 일부, dedicated UI 없음 | ⭐⭐ |
| Session impact analysis | plan event 데이터 보유, UI 없음 | ⭐⭐ |
| Aggregate session view (cross-project) | 현재 프로젝트별 분리, 전체 session 통합 view 없음 | ⭐⭐ |
| TUI | 미존재 | P3 (별 axis) |
| Archive 컴포넌트 명확화 | 사이드바에 아카이브 별도 X (Docs 안 일부) | ⭐⭐ (사이드바 정리) |

## 6. 적용 idea (5 영역, P0/P1/P2 분류)

### Idea T1 — Sidebar 통합 검색 (Cross-conversation Search)

**우선**: P1 (사용자 가치 즉시)

**Spec**:
- 사이드바 상단에 검색 input. 키워드 입력 시 모든 프로젝트의 conversation 메시지 검색
- backend: 기존 `tunaflow-mcp/search_documents` 또는 `vector_search::search_chunks_blocking` 활용. UI 만 신규
- 결과 click → 그 conversation 의 해당 메시지 위치로 jump
- 단축키 `Cmd+K` 또는 `Cmd+P`

**파일**: `src/components/tunaflow/sidebar/SearchSection.tsx` 신규 + `SidebarLayout` 통합. 약 150 LoC.

**위험**: 검색 결과가 너무 많을 때 페이지네이션 / 정렬 (relevance vs recent). FTS5 + vector hybrid 활용.

### Idea T2 — Safe cleanup (Backup + Dry-run + Confirm)

**우선**: P2

**Spec**:
- conversation 우클릭 → "삭제" 메뉴에서 직접 soft-hide 대신 modal 표시:
  - **Backup**: JSON/Markdown 으로 conversation export
  - **Dry-run**: 삭제 후 영향 분석 (linked plan / artifacts / branch 의존성)
  - **Confirm token**: 사용자가 conversation label 또는 ID 일부 입력해야 진행
- soft-hide 도 keep (현재 동작 유지) — 새 menu 추가만

**파일**:
- `src/components/tunaflow/sidebar/SafeDeleteModal.tsx` 신규
- `src-tauri/src/commands/conversations.rs` 의 `export_conversation` 명령 신규
- `src-tauri/src/commands/conversations.rs` 의 `analyze_conversation_dependencies` 명령 신규
- 약 250 LoC

**위험**: dependency 분석이 cycle 발생 가능 (branch ↔ main, plan ↔ artifact). 단순화 — direct 1-hop reference 만.

### Idea T3 — Native CLI session import (gateway feature)

**우선**: P2 (가장 strategic, 큰 변경)

**Spec**:
- Settings → "외부 session 가져오기" 또는 사이드바 우클릭 → "Import from native CLI"
- 지원 source:
  - Claude Code: `~/.local/share/claude/projects/<encoded-path>/<session-uuid>.jsonl`
  - Codex: `~/.codex/sessions/`
  - Gemini: 위치 확인 필요
- Import 시:
  1. native session 파일 read + parse
  2. tunaFlow DB 에 새 conversation 으로 insert (별 source = "imported")
  3. 사이드바에 표시 (별 icon 으로 구분)
- 양방향: tunaFlow conversation 을 native CLI 형식으로 export 도 가능

**파일**:
- `src-tauri/src/agents/{claude,codex,gemini}_session_import.rs` 신규
- `src-tauri/src/commands/conversations.rs` import 명령
- frontend Import wizard
- 약 400~600 LoC + DB schema (source 컬럼 추가, migration)

**위험**:
- native session 형식 변경 시 parser 깨짐 (semver-broken vendors)
- import 시 중복 detection (이미 import 한 session 재import?)
- 보안: 외부 session 의 신뢰성 (tunaFlow 의 system_prompt 와 다른 환경의 결과 mix)

### Idea T4 — Provider health panel

**우선**: P2

**Spec**:
- 사이드바 footer 또는 Settings → Diagnostics 페이지
- 각 provider 별 status:
  - **CLI 설치 여부** (claude / codex / gemini 등)
  - **인증 상태** (각 provider 의 `--status` 또는 비슷)
  - **session 파일 위치 + 권한**
  - **최근 호출 결과 + 5h rate limit** (Plan T1 의 rate_limit_event 활용)
- 문제 detect 시 fix 안내 link

**파일**:
- `src/components/tunaflow/diagnostics/ProviderHealth.tsx` 신규
- `src-tauri/src/commands/diagnostics.rs` provider check 명령
- 약 200 LoC

**위험**: CLI 의 `--status` 명령이 provider 별 다름. 추상화 부담 — `agents/{claude,codex,gemini}.rs` 의 health check trait 도입.

### Idea T5 — Session impact analysis (plan/artifact 기여도)

**우선**: P2

**Spec**:
- conversation 우클릭 → "임팩트 분석"
- 그 conversation 이 생성/수정한 plan / artifact / file 표시
- 시간순 timeline + 각 항목 click 시 jump
- backend: 기존 plan event + artifact 데이터 활용

**파일**:
- `src/components/tunaflow/InsightPanel.tsx` 의 새 섹션 또는 새 modal
- `src-tauri/src/commands/insight_extract.rs` 의 새 query
- 약 150 LoC

**위험**: 데이터 sparse — plan/artifact 가 명시적 conversation_id reference 가지지 않을 수 있음. 그 경우 message timestamp 기반 추정.

### (별 axis) Idea T6 — Archive 영역 명확화 + 사이드바 정리

**우선**: P3

**Spec**:
- 사이드바 새 섹션 `ArchiveSection.tsx` — 30일+ 미사용 conversation 자동 표시 (별 group)
- soft-hide 된 conversation 도 archive 로 분리 (현재 미표시)
- restore 또는 hard-delete 결정 UX

**파일**: `src/components/tunaflow/sidebar/ArchiveSection.tsx` 신규. 약 100 LoC.

**위험**: archive criteria (30일?) — 사용자 settings 로 override 가능. P3-Lite 패턴 (Plan E 와 같은 default + override).

### (별 axis) Idea T7 — TUI mode

**우선**: **P3 (별 product 후보)**

ThreadLens 의 TUI (keyboard-first) 패턴을 tunaFlow 에 도입은 큰 작업. 차라리 **별 product** (`tunaTUI`) 로 분리. 메모리 product_scope.md 의 "tunaMicro 같은 별도 프로덕트로 브랜치" 패턴 부합.

본 idea 에서는 P3 후보로 등록만, 별 plan 후속.

## 7. 적용 우선순위 + Timing

| Idea | 우선 | LoC | 의존 | 적용 시점 |
|---|---|---|---|---|
| **T1** Sidebar search | **P1** | 150 | 없음 | v0.1.5-beta publish 후 첫 cycle |
| T2 Safe cleanup | P2 | 250 | T1 인접 | 후속 |
| T3 Native CLI session import | P2 (strategic) | 400~600 + DB | DB schema migration | v0.1.6-beta 또는 v0.2.0 |
| T4 Provider health | P2 | 200 | 없음 | v0.1.5-beta publish 후 |
| T5 Session impact | P2 | 150 | InsightPanel | v0.1.5-beta publish 후 |
| T6 Archive 명확화 | P3 | 100 | 없음 | 사용자 보고 누적 후 |
| T7 TUI | P3 별 product | — | — | 별 plan |

## 8. Cross-link to existing ideas

- `bkitReferenceAdoptionIdea_2026-04-29.md` — bkit 의 Context Engineering / Role / Hooks. 본 idea 와 axis 다름 (bkit = ContextPack / agent 정책 / hook system, threadlens = session 관리 UI/UX). 두 idea 가 **complementary** — 다음 cycle 에서 둘 묶어 v0.1.6-beta 의 비전 정립 가능
- `agentApiQuotaErrorUxIdea_2026-04-29.md` — Layer 4 의 auto fallback 제안 (활성 다른 엔진 자동 detect) 이 ThreadLens T4 (Provider health) 와 cross-link

## 9. 사용자 가치 측정

| 사용자 유형 | 가치 |
|---|---|
| 다중 CLI 사용자 (Claude + Codex + Gemini 동시) | **매우 높음** — T1/T3/T4 immediate 가치 |
| 단일 CLI 사용자 | 중간 — T1/T2 만 활용 |
| Power user / dev | T7 (TUI) 후속 |
| 일반 사용자 | T1 (search) 가 가장 즉시 가치 |

ThreadLens 의 retention pattern (multi-CLI 통합 hub) 이 tunaFlow 의 기존 강점 (multi-agent execution) 과 결합 시 **AOC (agent orchestration client) 의 differentiation 강화**.

## 10. 본 idea 의 cycle position

- 현재 (2026-04-30): cycle 마무리 모드. T9 머지 + v0.1.5-beta release publish 우선
- 본 idea 는 **publish 후 첫 cycle 의 T1 (Sidebar search)** 우선 진행
- T2~T6 은 사용자 보고 / 가치 데이터 누적 후 P0~P1 격상 검토
- T3 (Native session import) 은 **strategic differentiation** — v0.1.6-beta 또는 v0.2.0 의 핵심 가치

## 11. 다음 step

본 idea 가 plan 으로 격상될 조건:
- v0.1.5-beta release publish 완료
- T1 (Sidebar search) 사용자 수요 확인
- 또는 mac/Windows architect 가 다른 priority 작업 마무리 후 capacity 확보

조건 충족 시 plan 이름 후보:
- `sidebarSessionManagementPlan_<date>.md` (T1+T4 묶음, P1)
- `nativeSessionImportPlan_<date>.md` (T3 단독, P2 strategic)
- `safeConversationCleanupPlan_<date>.md` (T2 단독, P2)

본 idea 문서는 SSOT — 적용 결정 시 cross-link.
