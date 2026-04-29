---
title: 알림 native UNUserNotificationCenter 전환 (AppleScript 의존 제거)
status: ready
phase: planning
priority: P2 (UX)
created_at: 2026-04-29
canonical: true
related:
  - src/stores/notificationStore.ts
  - src-tauri/Cargo.toml
issue_source: batmania52 보고 (#6, 2026-04-29)
---

# Native notification 전환

## Context

batmania52 보고:
> "알림은.. apple script 말고 네이티브로 해주셨으면 하는 작은 바람이 있습니다. **누르면 apple script 앱이 뜹니다..**"

현재: `notificationStore.ts:137` 에서 `tauri-plugin-notification` 의 `sendNotification` 사용. **Tauri 2 의 plugin-notification 은 macOS 에서 `osascript` 를 호출**해서 표시 (알려진 동작). 알림 클릭 시 macOS 가 어떤 앱을 dispatch 할지 결정하는데, source 가 osascript binary 라 Script Editor.app (또는 osascript 자체) 이 frontmost 로 올라가는 회귀.

해결 방향:
- **Plugin 옵션 점검**: tauri-plugin-notification 신버전이 native API 옵션 제공하는지 (Tauri 2.10 release note 확인)
- **Plugin 교체**: 또는 사용자 contribute plugin (`tauri-plugin-notify` 등) 사용 — 라이선스/안정성 검토
- **Direct ObjC bridge**: 직접 `UNUserNotificationCenter` 호출 — Rust `objc2` crate 또는 `cocoa` crate. 가장 확실하나 코드량 큼

## Goals

- (G1) 알림 클릭 시 osascript / Script Editor 가 뜨지 않음. tunaFlow 본체가 frontmost 로 올라옴.
- (G2) 알림 표시 자체는 macOS 표준 알림 센터 패턴 유지.
- (G3) cross-platform 호환 — Windows/Linux 의 알림 path 는 변경 없음 (이미 native).

## Non-goals

- ❌ 알림 액션 (버튼 / 답장) 추가 — 기본 dispatch 만.
- ❌ 알림 sound / icon 커스터마이즈 — 기본 사용.
- ❌ Push notification (외부 서버) — 별 영역.

## Subtasks

### Task 01 — 진단: 현재 Tauri plugin-notification 의 macOS path 확인 [P1, 진단]

**Changed files**: 없음 (조사)

**Change description**:
- `Cargo.toml` 의 `tauri-plugin-notification` 버전 확인
- 해당 버전 source 에서 macOS path 가 osascript 인지 native UNUserNotification 인지 read
- Tauri 2.x 최신 plugin-notification 이 native 옵션 제공하는지 (release note / changelog 검토)
- 가능하면 plugin upgrade 만으로 해결되는지 확인

**Verification**:
- 진단 결과 chat 보고: "현재 X 버전, native option Y 제공/미제공, 다음 step Z"

### Task 02 — 진단 결과에 따라 fix path 결정 [P2]

**Path A — plugin upgrade 만으로 해결**:
- `Cargo.toml` 버전 bump
- frontend 측 변경 없음 (`@tauri-apps/plugin-notification` package.json 도 동기화)
- Verification: 알림 1회 발송 + 클릭 → tunaFlow frontmost

**Path B — plugin 교체 또는 직접 bridge**:
- `cocoa` 또는 `objc2` crate 추가
- `src-tauri/src/notification.rs` 신규 — macOS path 직접 구현, cfg(target_os = "macos") 격리
- frontend `notificationStore.ts` 의 `sendNotification` 호출을 새 Tauri command 로 변경 (cross-platform: macOS = native, 그 외 = 기존 plugin)
- 사용자 권한 요청 (UNUserNotificationCenter `requestAuthorization`) 흐름 추가

진단 (Task 01) 결과 받기 전 path 확정 X.

**Verification (공통)**:
- macOS dev/release 빌드 양쪽에서 알림 표시 + 클릭 → tunaFlow frontmost (회귀 없음)
- Windows/Linux 빌드 회귀 없음
- 알림 권한 거부 시 graceful degrade (silent skip)

**회귀 위험 가드**:
- 기존 `notificationStore.ts:137-140` 의 호출 흐름은 변경 최소화. 내부 native bridge 만 추가/교체.
- macOS-specific 변경은 cfg 격리 필수 (Windows/Linux 회귀 0).
- 권한 요청은 첫 알림 발송 시점에만 (앱 시작 시 spam X).

## Cross-cutting risks

| 위험 | 대응 |
|---|---|
| 직접 ObjC bridge (Path B) 가 macOS 26.x 회귀 가능 | 진단 (Task 01) 단계에서 plugin upgrade path 우선 시도. Path B 는 마지막 수단. |
| Tauri 2.x plugin-notification 신버전이 다른 breaking change 동반 | upgrade 시 다른 plugin (clipboard / opener / store) 와의 호환 확인. |
| 알림 권한 미부여 시 silent | 첫 시도 후 권한 거부면 console warning + UI 토스트로 fallback (사용자가 인지). |

## Rollback

Path A: Cargo.toml 버전 revert + lock 파일 revert.
Path B: 새 파일 / store 변경 통째 revert.
