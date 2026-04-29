---
title: 좌측 문서 패널 범위 — 정책 결정 + 워크스페이스/서브폴더 가시성
status: design (정책 결정 우선)
phase: planning
priority: P2 (외부 사용자 보고)
created_at: 2026-04-29
canonical: false
related:
  - src/components/tunaflow/SidebarDocs.tsx (또는 인근)
  - src-tauri/src/commands/docs.rs (또는 인근)
issue_source: batmania52 보고 (#7, 2026-04-29)
---

# 문서 패널 범위 정책

## Context

batmania52 보고:
> "왼쪽에 나타나는 문서들이 docs 밑에 있는 파일과 루트에 있는 파일만 나타나는거 같은데 다른 폴더에 있는 문서나 **서브 프로젝트들이 모인 워크스페이스 같은걸 프로젝트 열기 대상으로 지정하거나 하면 서브 프로젝트에 있는 문서 등은 아예 나타나질 않습니다**. tunaFlow에서 만든 문서만 보이는게 목적인지 모든 문서를 보는게 목적인지 잘 모르겠어요."

현재: 좌측 문서 패널이 `<project_root>/docs/` 와 `<project_root>/*.md` 만 표시. 모노레포/워크스페이스 (e.g. `packages/*/docs/`) 의 문서는 미표시.

핵심은 **policy 결정** — Goals 정하기 전에 사용자(Architect) 가 의도 명확화 필요.

## Policy 옵션 (사용자 결정 영역)

### Option P1 — tunaFlow 가 만든 산출물만 (현재 동작 유지 + 명확화)

- 좌측 패널은 tunaFlow workflow 가 생성한 plan/result/review/artifact 위주
- "문서 패널" 이름을 "tunaFlow 산출물" 등으로 명확화
- 사용자가 직접 작성한 docs 는 별도 외부 에디터에서 보도록 유도
- 장점: 범위 명확, 성능 부담 적음
- 단점: 외부 사용자 기대 (모든 문서) 와 mismatch

### Option P2 — 프로젝트 전체 문서 (모노레포 포함)

- `<project_root>` 아래 재귀 탐색 (`.gitignore` 존중)
- 모든 `.md` (또는 설정으로 확장자 가능) 표시
- 폴더 트리 구조 유지
- 장점: 외부 사용자 직관 일치
- 단점: 큰 프로젝트에서 성능 부담, 무관 문서 노이즈, indexing 비용 증가

### Option P3 — 사용자 설정 (혼합)

- 기본은 P2 (모든 문서)
- 사용자가 settings 에서 폴더 화이트리스트/블랙리스트 지정 가능
- "tunaFlow 산출물만" 토글
- 장점: 유연
- 단점: 설정 복잡도, 첫 사용자 혼란

### Option P4 — 자동 탐지 + tunaFlow 산출물 강조

- P2 처럼 전체 탐색하되 tunaFlow workflow 산출물 (plan/result/review) 은 별 섹션 또는 색 강조
- 장점: 양쪽 사용자 만족
- 단점: 구현 복잡

## Recommended (Architect 의견)

**P3 with default = "tunaFlow 산출물 + docs/ + 루트 .md"**:
- 첫 사용자는 현재 동작과 유사하게 (low noise)
- 워크스페이스/모노레포 사용자는 settings 에서 "프로젝트 전체 docs" 토글 또는 폴더 추가
- 외부 사용자 기대치 양쪽 충족

## Goals (Recommended 채택 시)

- (G1) Settings 에 "문서 패널 scope" 섹션 — 기본값 / 전체 / 커스텀 폴더 list 옵션
- (G2) 기본 동작 = 현재 (`docs/` + 루트 .md). 회귀 0.
- (G3) "전체" 선택 시 `<project_root>` 재귀 탐색 (`.gitignore` 존중, max depth 5).
- (G4) "커스텀" 선택 시 폴더 추가/제거 UI (예: `packages/*/docs/`, `apps/*/README.md`).
- (G5) 패널 상단에 현재 scope 표시 (e.g. "tunaFlow 산출물 + docs/" 또는 "전체").

## Non-goals

- ❌ 비-md 파일 표시 (.txt, .rst 등) — md 한정.
- ❌ 외부 path (project root 밖) 표시.
- ❌ docs 의 실시간 watcher (별 plan 후보).

## Subtasks (정책 P3 채택 가정)

### Task 00 — 정책 결정 [필수, 사용자 영역]

**Changed files**: 없음

**Change description**: Architect 가 위 4 option 중 선택. 선택 결과를 본 plan 의 Goals 에 반영. Goals 확정 전 Task 01 시작 금지.

### Task 01 — Settings 섹션 + scope state [P2, 정책 P3 가정]

**Changed files**:
- `src/components/tunaflow/settings/DocsScopeSection.tsx` (신규)
- `src/components/tunaflow/SettingsPanel.tsx` (섹션 등록)
- `src/stores/settingsStore.ts` (scope state)
- `src-tauri/src/commands/settings.rs` (영구화 — settings.json 또는 DB)

**Change description**:
- scope state: `'tunaflow' | 'all' | 'custom'`. custom 일 때 `customFolders: string[]`.
- Settings UI: radio group + custom 일 때 folder picker
- 영구화: 기존 settings.json 패턴 활용

### Task 02 — Docs 탐색 로직 분기 [P2]

**Changed files**:
- `src-tauri/src/commands/docs.rs` 또는 인근 (탐색 함수)
- `src/components/tunaflow/SidebarDocs.tsx` (panel 렌더링)

**Change description**:
- scope='tunaflow' (기본) → 현재 로직 유지
- scope='all' → `<project_root>` walkdir, `.gitignore` 적용, depth 5, ext='.md' 필터
- scope='custom' → customFolders 만 walk
- 패널 상단에 scope 표시 (한 줄 i18n)

### Task 03 — 성능 가드 [P2]

**Changed files**: 같은 파일들

**Change description**:
- 'all' scope 에서 결과 file count > 200 시 warning 토스트 + "tunaflow scope 로 전환" 제안
- walkdir lazy loading (UI에 50개씩 표시 + "더 보기")

## Cross-cutting risks

| 위험 | 대응 |
|---|---|
| 'all' scope 에서 큰 모노레포 (e.g. node_modules/) 가 탐색되어 hang | `.gitignore` 적용 + depth 제한 + warning 토스트 |
| settings migration (scope 새 필드) 가 기존 사용자 영향 | 기본값 'tunaflow' 로 자동 fallback (회귀 0) |
| 커스텀 폴더가 cross-machine sync 시 invalid path | hardening plan 의 path validate fallback 재사용 |

## Rollback

각 task 분리 commit. Task 01 (Settings) 단독 revert 시 scope state 무력화 → 기본 동작 (tunaflow scope) 으로 fallback.

## 다음 step

**Architect 결정 대기**: Option P1/P2/P3/P4 중 선택. 결정 후 Goals/Subtasks 확정 + status: ready 로 전환.
