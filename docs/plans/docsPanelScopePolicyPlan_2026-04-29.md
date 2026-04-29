---
title: 좌측 문서 패널 범위 — P3-Lite (scope 토글, default='all')
status: completed
phase: merged
priority: P2 (외부 사용자 보고)
created_at: 2026-04-29
updated_at: 2026-04-29
merged_at: 2026-04-29
merged_pr: 219
merge_commit: d7cffa9
canonical: true
related:
  - src/components/tunaflow/SidebarDocs.tsx (또는 인근)
  - src-tauri/src/commands/docs.rs (또는 인근)
  - src/stores/settingsStore.ts
issue_source: batmania52 보고 (#7, 2026-04-29)
policy_decision:
  selected: P3-Lite
  default_scope: all
  reason: 외부 사용자 첫 인상 (즉시 직관 일치) > 노이즈 우려 (settings 토글로 회피 가능). 'custom' 옵션은 Phase 2 로 격하.
---

# 문서 패널 범위 — P3-Lite

## Context

batmania52 보고:
> "왼쪽에 나타나는 문서들이 docs 밑에 있는 파일과 루트에 있는 파일만 나타나는거 같은데 다른 폴더에 있는 문서나 **서브 프로젝트들이 모인 워크스페이스 같은걸 프로젝트 열기 대상으로 지정하거나 하면 서브 프로젝트에 있는 문서 등은 아예 나타나질 않습니다**. tunaFlow에서 만든 문서만 보이는게 목적인지 모든 문서를 보는게 목적인지 잘 모르겠어요."

현재: 좌측 문서 패널이 `<project_root>/docs/` 와 `<project_root>/*.md` 만 표시. 모노레포/워크스페이스 (e.g. `packages/*/docs/`) 의 문서는 미표시.

## Policy 결정 (2026-04-29 채택)

**P3-Lite — settings 토글 2 옵션, default='all'**

| 항목 | 결정 |
|---|---|
| scope 옵션 | `'all'` (전체 탐색) / `'tunaflow'` (현재 동작 — `docs/` + 루트 `.md`) |
| 기본값 | **`'all'`** — 외부 사용자 즉시 직관 일치 |
| `'custom'` (폴더 화이트리스트) | **Phase 2 로 격하** (사용자 요청 누적 후 별 plan) |
| 비-md 확장자, 외부 path, watcher | Non-goal (현재 plan scope 외) |

근거:
- 첫 인상 좌절(`'tunaflow'` default 시 "왜 내 docs 안 보여?")이 노이즈 짜증보다 retention 영향 큼.
- 노이즈 우려 사용자는 settings 1 클릭으로 `'tunaflow'` 좁힐 수 있음.
- tunaFlow 정체성 (agent-first AOC) 의 산출물 = `docs/plans/` 안에 모이므로 `'all'` 에서도 명확히 가시.
- 'custom' 은 구현 복잡도 ↑ 라 후속 — 'all' / 'tunaflow' 두 옵션만으로도 외부 사용자 보고 즉시 해소.

## Goals

- (G1) Settings 에 "문서 패널 scope" 토글 — `'all' | 'tunaflow'` 2 옵션.
- (G2) 신규 사용자 default = `'all'`. 기존 사용자 (settings 없음) 도 자동으로 `'all'` 로 fallback.
- (G3) `'all'` 선택 시 `<project_root>` 재귀 탐색 (`.gitignore` 존중, max depth 5, ext=`.md`).
- (G4) `'tunaflow'` 선택 시 현재 로직 (docs/ + 루트 .md) 유지.
- (G5) 패널 상단에 현재 scope 1줄 표시 (e.g. "📁 전체 문서" / "📁 tunaFlow 산출물").
- (G6) 'all' scope 에서 결과 file count > 200 시 warning 토스트 + "tunaflow scope 로 전환" 제안.

## Non-goals

- ❌ `'custom'` 폴더 화이트리스트 — Phase 2 (사용자 요청 누적 후 별 plan).
- ❌ 비-md 파일 표시 (.txt, .rst 등).
- ❌ 외부 path (project root 밖) 표시.
- ❌ docs 의 실시간 watcher.
- ❌ 문서 thumbnail/preview.

## Subtasks

### Task 01 — Settings 섹션 + scope state [P2]

**Changed files**:
- `src/components/tunaflow/settings/DocsScopeSection.tsx` (신규, 약 60 줄)
- `src/components/tunaflow/SettingsPanel.tsx` (섹션 등록 1~2 줄)
- `src/stores/settingsStore.ts` (scope state 추가)
- `src/locales/{ko,en}/settings.json` (라벨)
- `src-tauri/src/commands/settings.rs` 또는 settings.json 영구화 hook (기존 패턴 활용)

**Change description**:
- scope state: `'all' | 'tunaflow'`. default `'all'`.
- 영구화: 기존 settings.json 패턴 그대로. 새 키 `docsPanel.scope`.
- Settings UI: radio group 2 옵션 + 짧은 설명 ("전체: 프로젝트의 모든 .md / tunaFlow: docs/ + 루트만").

**Verification**:
- `npx tsc --noEmit` 통과
- `npx vitest run src/stores/settingsStore` 통과 (default 값 + 토글 + 영구화)
- dev 모드 manual: settings 열고 토글 → 패널 즉시 반영

**회귀 위험 가드**:
- `settingsStore` 의 다른 state / 영구화 로직 변경 금지.
- SettingsPanel 의 다른 섹션 (Runtime / Profile / Skills 등) 변경 금지.
- 기존 사용자 (settings 에 `docsPanel.scope` 키 없음) 가 `'all'` 로 자동 적용되는지 확인 (G2 보장).

### Task 02 — Docs 탐색 로직 분기 + 패널 표시 [P2]

**Changed files**:
- `src-tauri/src/commands/docs.rs` 또는 인근 탐색 함수
- `src/components/tunaflow/SidebarDocs.tsx` (panel 렌더링 + scope 표시 1줄)
- `src-tauri/Cargo.toml` (`ignore` crate 또는 `walkdir` + `.gitignore` 처리 필요시. 이미 있으면 skip)

**Change description**:
- scope='tunaflow' → 현재 로직 유지 (회귀 0).
- scope='all' → `walkdir::WalkDir::new(project_root).max_depth(5)` + `.gitignore` 적용 + ext=`.md` 필터.
  - `.gitignore` 처리: `ignore` crate (Rust) 또는 manual parsing. `node_modules/`, `target/`, `dist/`, `build/` 같은 흔한 무거운 폴더 자동 제외.
- 결과 정렬: 폴더 트리 구조. 같은 폴더 내 알파벳순.
- 패널 상단 1줄: scope 라벨 (i18n key 활용).

**Verification**:
- `cd src-tauri && cargo check && cargo test --lib commands::docs` 통과
- `npx tsc --noEmit && npx vitest run src/components/tunaflow/Sidebar` 통과
- dev 모드 manual:
  - tunaFlow 자체 (모노레포 X) 에서 `'all'` 토글 → docs/ + 루트 + 그 외 (`scripts/` 의 `.md` 등) 가시
  - 모노레포 (예: tunaInsight 같은) 에서 `'all'` → `packages/*/docs/` 등 가시. node_modules/ 안의 .md 는 미가시 (.gitignore 효과)
  - `'tunaflow'` 토글 → 현재 동작과 동일 (회귀 0 시각 확인)

**회귀 위험 가드**:
- scope='tunaflow' path 의 코드 변경 금지 (분기만 추가).
- 다른 sidebar 패널 (Conversations / Plans / Insight 등) 변경 금지.
- `.gitignore` 미존재 프로젝트에서도 graceful (전체 탐색 + warning 한 번).

### Task 03 — 성능 가드 (warning 토스트) [P2]

**Changed files**: Task 02 와 같은 파일

**Change description**:
- `'all'` scope 결과 file count > 200 → 1회 warning 토스트: "전체 문서 N개. 패널이 무거우면 settings 에서 'tunaFlow 산출물' 로 전환 가능."
- threshold 는 const (e.g. `DOCS_PANEL_WARNING_THRESHOLD = 200`). 환경/UX 데이터 기반 후속 조정.
- lazy load (50개씩 + "더 보기") — 1차 구현 안 해도 무방. count > 200 토스트만 충분.

**Verification**:
- 큰 모노레포 (200+ .md) 에서 `'all'` 진입 → 토스트 1회 표시
- 100개 미만 프로젝트에선 토스트 안 뜸

**회귀 위험 가드**:
- 토스트는 1회만 (세션 동안). 매 패널 갱신마다 spam X — 1회 표시 후 dismiss flag.

## Cross-cutting risks

| 위험 | 대응 |
|---|---|
| `'all'` scope 에서 큰 모노레포 (node_modules/, target/) 탐색 hang | `.gitignore` 적용 + depth 5 제한 + Task 03 warning |
| settings migration (scope 새 필드) 가 기존 사용자 영향 | 미존재 시 default `'all'` 로 자동 fallback (G2 명시) |
| 모바일 (tunaflow-mobile) 또는 다른 frontend 영향 | 본 plan 은 desktop 한정. 모바일 영향 없음. |
| `'all'` 의 file count 가 너무 많아 first paint 지연 | Task 03 의 토스트로 사용자 인지. lazy load 는 Phase 2. |

## Rollback

Task 01 / Task 02 / Task 03 분리 commit 권장. Task 01 단독 revert 시 scope state 자체 사라져 default 'all' 동작은 유지 (UI 토글만 사라짐 — graceful degrade).

## Phase 2 (out-of-plan, 후속 후보)

- `'custom'` 폴더 화이트리스트 — 사용자 보고 누적 후
- lazy load (50개씩 + 더 보기)
- 비-md 확장자 (`.rst`, `.txt`)
- 폴더 즐겨찾기 / pin
- realtime watcher (FS 이벤트 기반 패널 자동 갱신)
