---
title: Windows 타이틀바 통합 — `decorations: false` + 자체 Window Controls (mac parity)
status: ready (옵션 B 정석 채택, 사용자 결정 2026-04-29)
priority: P1 (베타 UX 정합성 — Windows 사용자 첫 인상 영향)
created_at: 2026-04-29
calling_role: architect (Windows 머신)
related:
  - src-tauri/tauri.conf.json
  - src/components/tunaflow/TitleBar.tsx
  - src/components/tunaflow/AppShell.tsx
  - docs/plans/windowsBetaHardeningPlan_2026-04-26.md
  - docs/plans/windowsDependencyBootstrapPlan_2026-04-29.md
  - docs/plans/windowsCiPipelinePlan_2026-04-29.md
canonical: true
---

# Windows 타이틀바 통합 — `decorations: false` + 자체 Window Controls

## 0. 요약 (1 단락)

mac 은 `tauri.conf.json` 의 `titleBarStyle: "Overlay"` + `hiddenTitle: true` 로 traffic light 영역 위에 `TitleBar.tsx` 가 overlay 되어 *통합 1 라인* UX 를 만든다. 이 두 옵션은 **macOS-only** 라 Windows 에는 적용되지 않고, 결과적으로 Windows 에서는 (1) native title bar (close/min/max) + (2) `TitleBar.tsx` 28px + (3) AppShell 콘텐츠 헤더 = **3 라인** 으로 보인다. 본 plan 은 옵션 B (정석) 를 채택해 (a) Windows 에서 `decorations: false` 적용, (b) 자체 Window Controls 컴포넌트 (Minimize / Maximize-Restore / Close) 구현, (c) `TitleBar.tsx` 의 drag region + Window Controls 통합 → mac 과 동일한 1 라인 통합 UX. INV-1 (mac 사이드 이펙트 0) 은 모든 변경에 cfg 분기 또는 platform-detect 로 격리.

## 1. Invariants

| ID | 내용 |
|---|---|
| **INV-1** 🔴 | mac 측 `titleBarStyle: "Overlay"` / `hiddenTitle: true` 그대로 유지. mac 의 traffic light 통합 UX 변경 0. |
| **INV-2** | PR + CI watch 필수. macOS + Windows 양쪽 (W-CI-1 머지 후 자동, 그 전에는 수동 dev 모드 smoke) 검증. |
| **INV-3** | TitleBar.tsx 의 *비-Window-Controls* 영역 (project name / git branch 표시 등) 변경 0. mac 사용자 시각 회귀 0. |
| **INV-4** | 단일 axis per commit. T-WT-1~5 각 task 마다 별 PR. |
| **INV-WT-A** | **사용성 표준 준수**: Windows native window controls 동작 (double-click maximize / Aero snap / Win+↑ maximize / Win+↓ restore / Alt+space 메뉴) 모두 보존. tauri 가 `decorations: false` 로 일부 native 동작이 깨지므로 보강 필요. |
| **INV-WT-B** | **accessibility**: keyboard tab navigation, ARIA labels (close button 한국어/영문), high-contrast 테마 호환. |
| **INV-WT-C** | **Windows 11 snap layouts overlay** 호환 — Maximize 버튼에 마우스 hover 시 snap layouts 가 떠야 함 (`snapLayouts` Tauri 2 hint 또는 Win32 API 사용). |

## 2. 현황 매트릭스

### 2.1 라인 분해

| OS | 라인 1 | 라인 2 | 라인 3 |
|---|---|---|---|
| **mac (현)** | TitleBar.tsx (traffic light overlay, 28px) | AppShell 콘텐츠 헤더 (sidebar/conversation) | — |
| **Windows (현)** | **native title bar** (~32px, close/min/max + 제목) | TitleBar.tsx (28px) | AppShell 콘텐츠 헤더 |
| **Windows (목표)** | TitleBar.tsx + WindowControls (28px or 32px, 통합) | AppShell 콘텐츠 헤더 | — |

### 2.2 관련 코드

- **`src-tauri/tauri.conf.json:13-24`** — windows[0] 설정. `titleBarStyle: "Overlay"` / `hiddenTitle: true` 가 mac-only.
  ```json
  "windows": [{
    "title": "tunaFlow",
    "width": 1200, "height": 800,
    "minWidth": 720, "minHeight": 480,
    "visible": false,
    "theme": "Dark",
    "titleBarStyle": "Overlay",     // ← macOS-only
    "hiddenTitle": true              // ← macOS-only
  }]
  ```
- **`src/components/tunaflow/TitleBar.tsx`** (60 라인) — line 5 코멘트: *"Custom title bar — overlays the macOS traffic light area"*. `data-tauri-drag-region` 적용된 28px bar. project name + git branch 중앙 표시.
- **AppShell.tsx** — `<TitleBar />` mount. 그 외 OS 분기 없음.

### 2.3 Tauri 2 Window Controls 패턴

Tauri 2 는 `decorations: false` + `set_decorations(false)` 로 모든 native 장식 제거 가능. Window Controls 는 frontend 에서 직접 그리고 Tauri 의 `getCurrentWindow().minimize() / .toggleMaximize() / .close()` API 호출. Windows 11 의 snap layouts overlay 는 별도 hint:
- 옵션 a: `setSnapLayouts(true)` (Tauri 2 plugin / Win32 wrapping)
- 옵션 b: 모스 hover 시 OS 가 maximize button 위치를 자동 감지 (Windows 11 22H2+) — `decorations: false` 시 detection 실패 가능
- 옵션 c: invisible native maximize button 1px 영역 유지하여 OS detect — hack

추후 R-WT-3 에서 어느 옵션 채택할지 결정.

## 3. 설계 핵심

### 3.1 cfg 분기 전략

| 영역 | 분기 방식 |
|---|---|
| `tauri.conf.json` | Tauri 2 platform-specific config — `tauri.windows.conf.json` 또는 단일 conf 안에 plugin 빌드 hook 으로 분기. 본 plan 권장: **단일 conf + Rust setup hook 에서 cfg(windows) 분기로 `set_decorations(false)` 호출** (conf split 회피, 단순) |
| frontend `TitleBar.tsx` | `import { type as platform } from '@tauri-apps/plugin-os'` 로 detect. Windows 면 추가 `<WindowControls />` 렌더, mac 은 그대로 traffic light overlay |
| `WindowControls.tsx` (신규) | cfg 없이 모든 OS 에서 컴파일되지만 mac 에선 미렌더 (TitleBar 의 platform check 가 게이트) — INV-1 안전 |

### 3.2 WindowControls 컴포넌트 spec

```tsx
// src/components/tunaflow/WindowControls.tsx (신규)
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";
import { Minus, Square, X, Copy } from "lucide-react";   // Copy = restore icon

export function WindowControls() {
  const [isMaximized, setIsMaximized] = useState(false);
  const w = getCurrentWindow();

  useEffect(() => {
    w.isMaximized().then(setIsMaximized);
    const unlisten = w.onResized(() => w.isMaximized().then(setIsMaximized));
    return () => { unlisten.then(fn => fn()); };
  }, []);

  return (
    <div className="flex items-center h-full" data-tauri-drag-region={false}>
      <button aria-label="Minimize" onClick={() => w.minimize()}
        className="h-full w-[46px] flex items-center justify-center hover:bg-foreground/10">
        <Minus size={14} />
      </button>
      <button aria-label={isMaximized ? "Restore" : "Maximize"}
        onClick={() => w.toggleMaximize()}
        className="h-full w-[46px] flex items-center justify-center hover:bg-foreground/10">
        {isMaximized ? <Copy size={12} /> : <Square size={12} />}
      </button>
      <button aria-label="Close" onClick={() => w.close()}
        className="h-full w-[46px] flex items-center justify-center hover:bg-status-rejected hover:text-white">
        <X size={14} />
      </button>
    </div>
  );
}
```

크기/색상/아이콘은 디자인 시스템 토큰 (oklch theme) 으로 통일.

### 3.3 TitleBar 통합

```tsx
// src/components/tunaflow/TitleBar.tsx (수정)
import { type as osType } from "@tauri-apps/plugin-os";
import { WindowControls } from "./WindowControls";

const isWindows = osType() === "windows";

export function TitleBar() {
  // ... 기존 useState/useEffect 그대로
  return (
    <div data-tauri-drag-region className="h-[32px] shrink-0 flex items-center justify-between select-none bg-sidebar">
      {/* mac 의 traffic light 영역 — 좌측 64px 정도 padding */}
      <div className={isWindows ? "w-[12px]" : "w-[72px]"} />
      <div data-tauri-drag-region className="flex items-center gap-0">
        {/* 기존 tunaFlow / projectName / gitBranch — 변경 0 */}
      </div>
      {isWindows ? <WindowControls /> : <div className="w-[72px]" />}
    </div>
  );
}
```

높이 `28px → 32px` 통일 (Windows 11 표준 caption 높이와 일치). mac 측 traffic light 도 32px 안에서 렌더되어 자연스러움.

### 3.4 Rust setup hook

```rust
// src-tauri/src/bootstrap/window.rs (또는 services.rs 안)
#[cfg(target_os = "windows")]
{
    use tauri::Manager;
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.set_decorations(false);
        // Windows 11 snap layouts: optional hint via Win32 API
        // (R-WT-3 에서 옵션 결정)
    }
}
```

mac 에는 `titleBarStyle: Overlay` + `hiddenTitle: true` 가 conf 에서 그대로 동작. INV-1 안전.

## 4. Phase 분해

### Phase 1 — 기본 통합 (P0, 베타 hotfix)

#### T-WT-1 — Rust setup hook 으로 Windows `decorations: false` [P0]
- 파일: `src-tauri/src/bootstrap/window.rs` 또는 `services.rs`
- 변경: `#[cfg(target_os = "windows")] { w.set_decorations(false); }`
- 검증: dev 모드 smoke — Windows native title bar 사라짐. mac 은 traffic light 그대로.
- INV-1 안전 (cfg 격리).

#### T-WT-2 — `WindowControls.tsx` 컴포넌트 신규 [P0]
- 파일: `src/components/tunaflow/WindowControls.tsx` (신규)
- 변경: §3.2 spec 그대로. `getCurrentWindow().minimize/toggleMaximize/close`.
- 검증: 단위 테스트 (vitest, snapshot + click handler invoke spy). 통합은 T-WT-3 후.

#### T-WT-3 — `TitleBar.tsx` 에 platform detect + WindowControls 통합 [P0]
- 파일: `src/components/tunaflow/TitleBar.tsx` 수정
- 변경: §3.3 spec. `osType() === "windows"` 분기 + `<WindowControls />` 렌더.
- 검증: dev 모드 smoke — Windows 1 라인 통합, mac 그대로. drag region 동작 (창 드래그) 확인.
- INV-3 안전 (project name / git branch 영역 변경 0).

### Phase 2 — Windows 표준 동작 보강 (P1)

#### T-WT-4 — double-click maximize + Aero snap + accessibility [P1]
- `data-tauri-drag-region` 영역에서 double-click 시 maximize toggle (Tauri 기본).
- Win+↑/↓ keyboard shortcut 은 OS 가 처리 — 별 코드 불필요, 그러나 검증 필수.
- WindowControls 의 ARIA labels (한국어/영문 i18n), keyboard tab navigation (Tab 키로 close 까지 도달), focus ring 명시.
- 단위 테스트 (a11y testing-library) 추가.

#### T-WT-5 — Windows 11 snap layouts overlay [P1, 옵션 결정 필요]
- Q-WT-3 결정 후 옵션 a/b/c 중 하나 채택.
- 옵션 a (Tauri plugin or Win32 API): Maximize 버튼 hover 시 snap layouts overlay 표시. UX 표준.
- 옵션 c (invisible native maximize): hack 이지만 OS detect 자동.
- 비용 vs 가치 평가 후 결정.

### Phase 3 — 디자인 시스템 통합 (P2)

#### T-WT-6 — WindowControls 색상/아이콘 디자인 토큰 통일 [P2]
- 디자인 시스템 (oklch dark/light) 의 `--titlebar-button-hover`, `--titlebar-close-hover` 토큰 신설.
- light/dark 테마 양쪽 호환.

## 5. 작업 분해 — developer 인계용

| Task | 파일 | 검증 명령 | 예상 LOC | 우선순위 |
|---|---|---|---|---|
| **T-WT-1** | `src-tauri/src/bootstrap/{services,window}.rs` | `cargo check` + dev 모드 Windows smoke | +10 / -0 | P0 |
| **T-WT-2** | `src/components/tunaflow/WindowControls.tsx` (신규) | `vitest run` | +60 / -0 | P0 |
| **T-WT-3** | `src/components/tunaflow/TitleBar.tsx` | dev 모드 Windows + mac 양쪽 smoke | +20 / -5 | P0 |
| **T-WT-4** | `WindowControls.tsx` + i18n keys | a11y testing-library | +30 / -5 | P1 |
| **T-WT-5** | (Win32 API 또는 Tauri plugin) | dev 모드 Windows 11 hover snap overlay | 가변 | P1 |
| **T-WT-6** | `src/index.css` (또는 theme tokens) | 시각 검증 | +10 / -3 | P2 |

본 plan 의 베타 hotfix 범위 = **T-WT-1 → T-WT-2 → T-WT-3** (P0 3개). 이 셋만 머지하면 사용자 보고된 *3 라인 → 1 라인* 정상화. T-WT-4/5/6 은 후속.

## 6. 회귀 가드 / 검증 시나리오

### 6.1 macOS 무영향성 (INV-1)

- T-WT-1: `#[cfg(target_os = "windows")]` 안에서 `set_decorations(false)` — mac 측 setup hook 호출 0 → 영향 0.
- T-WT-2: WindowControls 신규 파일, mac 에선 import 안 됨 (TitleBar 의 platform check 게이트).
- T-WT-3: TitleBar 의 platform branch — mac 은 false 분기, 기존 동작 그대로.

### 6.2 Windows 검증 시나리오

| ID | 시나리오 | 기대 결과 |
|---|---|---|
| WT-1 | T-WT-1 머지 후 dev 모드 → Windows 창 native title bar 사라짐 | 1 라인 (28~32px) 만 남음 (TitleBar.tsx) |
| WT-2 | T-WT-3 머지 후 → WindowControls 우상단 표시, click 시 minimize/maximize/close 정상 | 동작 OK |
| WT-3 | TitleBar 영역 드래그 → 창 이동 | drag region 정상 |
| WT-4 | TitleBar 영역 double-click → maximize/restore toggle | OS 표준 동작 |
| WT-5 | Win+↑ / Win+↓ → maximize/restore | OS handler 정상 |
| WT-6 | Aero snap (창 가장자리로 드래그) | 자동 분할 동작 |
| WT-7 | Maximize 후 WindowControls 의 Maximize → Restore icon 으로 변경 | 상태 동기화 (Square → Copy icon) |
| WT-8 | Windows 11 22H2+ 에서 Maximize 버튼 hover → snap layouts overlay | T-WT-5 채택 옵션에 따라 (a 즉시 / b OS auto / c hack) |
| WT-9 | Tab navigation: TitleBar → WindowControls Min → Max → Close | focus ring 명시 |
| WT-10 | high-contrast theme | 모든 버튼 outline 보이는지 |
| WT-11 | dev 모드 hot reload 시 decorations 상태 보존 | re-spawn 없이 유지 |

### 6.3 baseline 회귀

- 본 plan 시작 시점 (PR #226 머지 후 = T-WT-1 PR base): FE 381 / Rust 573 (Windows). +N (테스트 추가) 만 허용.
- mac baseline 별 변화 없어야 함 — T-WT-1/3 cfg 격리 + T-WT-2 미렌더.

## 7. 리뷰어(Codex / mac architect) review 포인트

- **R-WT-1** WindowControls hover/active state 의 접근성 — keyboard Tab navigation 시 focus ring, hover 가 마우스만의 cue 인지 확인.
- **R-WT-2** Maximize ↔ Restore toggle 상태 동기화 — `onResized` listener 의 cleanup, edge case (창 직접 드래그로 unmaximize).
- **R-WT-3** Windows 11 snap layouts overlay 옵션 — a (Tauri plugin / Win32 API) vs b (OS auto-detect) vs c (invisible native maximize hack). 비용/표준성 trade-off.
- **R-WT-4** drag region 이 button area 와 겹치지 않게 — `data-tauri-drag-region={false}` 명시. button click 이 drag 로 오인되지 않는지.
- **R-WT-5** HiDPI / multi-monitor 에서 button 크기 — 32px caption + 46px button width 가 표준. 다른 dpi 환경 검증.
- **R-WT-6** Tauri 2 의 `getCurrentWindow().toggleMaximize()` 가 unmaximize → 원래 크기 정확히 복원하는지.
- **R-WT-7** Linux 환경 — 본 plan 은 Windows 만 다룸. Linux native decorations 는 그대로 유지. 추후 Linux 도 통합할지는 별 plan.

## 8. 오픈 질문

| Q | 결정 필요한 사항 |
|---|---|
| **Q-WT-1** | WindowControls 디자인 — Windows 11 native 스타일 (사각 버튼, 46×32) vs mac traffic light 스타일 (동그란 버튼) vs custom. Windows 사용자 기대 = native 스타일 권장. |
| **Q-WT-2** | 디자인 시스템 토큰 — close 버튼 hover red (Windows native 와 동일) 채택할지, 또는 우리 `status-rejected` 토큰 재사용. |
| **Q-WT-3** | snap layouts overlay (R-WT-3) 옵션 a/b/c 중 어느 것. a 가 표준이지만 비용 ↑, b 가 가장 단순하지만 22H2+ 한정 + decorations:false 시 detection 불확실. |
| **Q-WT-4** | TitleBar.tsx 의 git branch / project name 위치 — 중앙 (현재) vs 좌측 (mac 의 일반 패턴). cross-platform 일관성. |
| **Q-WT-5** | Linux 도 함께 처리할지 — 본 plan 범위 외, 별 plan 후속. |
| **Q-WT-6** | T-WT-5 (snap layouts) 의 베타 포함 여부 — P1 이라 베타 hotfix 외. v0.1.5 정식 release 로 미룰지. |

## 9. 진행 메모

- 본 plan motivation: 사용자 보고 (2026-04-29) — Windows 에서 헤더 3 라인. mac 은 1 라인 통합 (`titleBarStyle: Overlay` + `hiddenTitle: true`).
- 옵션 B (정석) 사용자 결정 — `decorations: false` + 자체 Window Controls.
- T-WT-1~3 (P0) 만 머지하면 베타 사용자 시각 회귀 즉시 해소. T-WT-4/5/6 은 후속 axis.
- Windows architect 또는 디벨로퍼 책임. 디벨로퍼 T4 (dependency dialog) 진행 중이라 *T4 머지 후* 본 plan T-WT-1~3 진행 권장 — UI 영역 axis 충돌 회피.
- mac architect review 권장 (Q-WT-1~6 결정), 그 다음 T-WT-1 PR.
- windowsBetaHardening axis (§B startup race / §C DB path / §D watchdog) 와 별 axis. UI 정합성 영역.
