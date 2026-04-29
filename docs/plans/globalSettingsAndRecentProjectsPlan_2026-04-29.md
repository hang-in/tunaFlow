---
title: 글로벌 설정 진입점(Cmd+,) + 최근 프로젝트 목록
status: completed
phase: merged
priority: P1 (UX, 외부 사용자 보고)
created_at: 2026-04-29
merged_at: 2026-04-29
task_01_merged_pr: 217
task_01_merge_commit: 50b24ea
task_02_merged_pr: 218
task_02_merge_commit: 6d01c7c
db_migration: v47 → v48 (recent projects, +5 unit tests)
canonical: true
related:
  - src/components/tunaflow/RuntimeStatusBar.tsx
  - src/components/tunaflow/SettingsPanel.tsx
  - src/components/tunaflow/ProjectOnboardingModal.tsx
issue_source: batmania52 보고 (#1, #4, 2026-04-29)
---

# 글로벌 설정 진입점 + 최근 프로젝트 목록

## Context

batmania52 보고:
- "뭐가 됐든 뭔가 프로젝트를 열기 전엔 설정에 접근할 수 없습니다. **Command+, 라던가 메뉴에서 설정에 접근할 수 있는 방법**이 있으면 좋을거 같습니다."
- "최근 열었던 프로젝트 리스트를 확인한다던가 하는 편의성이 있으면 좋을 듯 합니다."

현재:
- `SettingsPanel` 은 `RuntimeStatusBar.tsx` 안에서만 렌더링 (line 8, 450). RuntimeStatusBar 가 프로젝트 선택 후에만 가시 → 진입점 없음.
- "최근 프로젝트" UI 도 코드 내 grep 결과 없음 (`recentProjects|RecentProjects` 매치 0).

두 axis 묶음 — 모두 onboarding/launcher 화면 UX 영역.

## Goals

- (G1) Cmd+, 단축키로 어디에서든 SettingsPanel 열림 (프로젝트 선택 전 포함).
- (G2) macOS 메뉴 항목 (`tunaFlow > 설정...` 또는 `Settings...`) 에서 설정 열림 — 표준 macOS UX.
- (G3) Onboarding/launcher 화면에 "최근 열었던 프로젝트" 목록 표시 (최근 5개 권장). 클릭 시 그 프로젝트로 진입.
- (G4) DB / store 에 recent projects metadata 저장 (timestamp, path, name).

## Non-goals

- ❌ 설정 panel 자체 변경 (레이아웃 / 섹션 / 옵션 등) — 진입점만 추가.
- ❌ 프로젝트 thumbnail / preview 표시 (P3 후속).
- ❌ 최근 프로젝트 검색 / 필터링 (P3 후속).
- ❌ Pin/즐겨찾기 기능 (별 plan 후보).

## Subtasks

### Task 01 — Cmd+, 글로벌 단축키 + macOS 메뉴 항목 [P1]

**Changed files**: `src/App.tsx` (또는 root), `src/components/tunaflow/RuntimeStatusBar.tsx`, `src-tauri/src/menu.rs` (있다면) 또는 `src-tauri/src/lib.rs` 메뉴 setup

**Change description**:
- 전역 keyboard listener (root level useEffect) — `(e.metaKey || e.ctrlKey) && e.key === ','` 이면 settings open. zustand store 또는 context 로 settings open state 끌어올림.
- SettingsPanel 의 mount 위치를 RuntimeStatusBar 안에서 root level (App.tsx) 로 이동. open state 는 store 로 관리.
- macOS 메뉴: Tauri 2 의 `Menu::new()` builder 로 `MenuItem::new("Settings...", "CmdOrCtrl+,")` 추가. 클릭 시 frontend event emit → store 업데이트.
- Cross-platform: Windows/Linux 도 Ctrl+, 동작 확인.

**Verification**:
- macOS dev: 프로젝트 미선택 화면에서 Cmd+, 누르면 settings 모달 가시
- 메뉴 바에 "Settings..." 항목 있고 클릭 시 동일 동작
- 프로젝트 선택 후에도 동일 동작 (회귀 없음)
- ESC 또는 onClose 로 닫힘
- `npx tsc --noEmit` 통과

**회귀 위험 가드**:
- 기존 `RuntimeStatusBar.tsx:450` 의 `<SettingsPanel onClose={...} initialSection={...} />` 호출이 store 기반으로 변경되어도 `initialSection` prop 전달 보존 (다른 진입점이 특정 섹션으로 열기 위해 사용).
- 다른 단축키 (예: 채팅 send Cmd+Enter) 와 충돌 없는지 확인.
- macOS 메뉴 추가가 Windows/Linux 빌드에 cfg 분기로 격리되어야 함 (메뉴 자체는 cross-platform, 단축키 라벨만 OS-aware).

### Task 02 — DB / store 의 recent projects metadata + UI [P1]

**Changed files**:
- `src-tauri/src/db/migrations/` (새 migration 추가, projects 테이블에 `last_opened_at` 컬럼 또는 별 `recent_projects` 테이블)
- `src-tauri/src/commands/projects.rs` (load 시 last_opened_at 갱신, list_recent 명령)
- `src/lib/api/projects.ts` (recent fetch API)
- `src/components/tunaflow/ProjectOnboardingModal.tsx` 또는 launcher 화면 (recent 목록 렌더링)
- `src/locales/{ko,en}/onboarding.json` (라벨)

**Change description**:
- DB migration: `ALTER TABLE projects ADD COLUMN last_opened_at INTEGER DEFAULT 0` (이미 있으면 skip).
- 프로젝트 load 시 `UPDATE projects SET last_opened_at = unix_now() WHERE id = ?`.
- 새 명령 `list_recent_projects(limit: i64)` — `SELECT ... ORDER BY last_opened_at DESC LIMIT ?`.
- Onboarding/launcher 화면에 "최근 열었던 프로젝트" 섹션, 5개 표시. 항목 클릭 시 그 프로젝트로 진입 (path validate — 이미 hardening plan 의 C 트랙 fallback 로직 활용).
- 빈 list 면 섹션 미표시 (첫 사용자 깔끔).

**Verification**:
- 새 프로젝트 1~3개 생성 후 onboarding 화면에서 가시 확인
- 클릭 시 진입
- path 가 invalid 한 (e.g. 삭제된) 프로젝트는 disabled 표시 또는 클릭 시 fallback 처리
- `cd src-tauri && cargo test --lib commands::projects` 통과
- `npx vitest run src/lib/api/projects` 통과
- DB migration 리버스 호환 (기존 사용자 DB 가 ALTER 적용 후 정상 load)

**회귀 위험 가드**:
- DB migration 은 idempotent — 이미 컬럼 있으면 skip (PRAGMA 또는 try/catch).
- `commands/projects.rs` 의 다른 명령 (list, create, delete) 변경 금지.
- ProjectOnboardingModal 의 다른 분기 (분석 진행/실패/성공) 영향 없는지 확인.

## Cross-cutting risks

| 위험 | 대응 |
|---|---|
| 글로벌 keyboard listener 가 chat 입력 중 IME 와 충돌 | textarea/input focus 시 listener 무시. stopPropagation 영역 확인. |
| recent projects 가 다른 OS 경로 (cross-machine sync) 시 hang | hardening plan 의 path validate fallback 로직 재사용. |
| macOS 메뉴 항목 추가가 dev/release 빌드 모두 동일 동작 | Tauri 2 menu API 가 dev 모드에서도 동작 확인. |
| migration 회귀 (테이블 스키마 변경) | rollback migration 동시 작성. user DB 백업 권장은 README 한 줄. |

## Rollback

Task 01 / Task 02 분리 commit. Task 01 단독 revert 가능. Task 02 의 migration revert 는 `last_opened_at` 컬럼 drop SQL — 별도 migration 으로 작성.
