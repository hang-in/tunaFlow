---
title: 알림 native UNUserNotificationCenter 전환 (Path B 정공법 채택)
status: ready
phase: planning
priority: P2 (UX, 외부 사용자 보고)
created_at: 2026-04-29
updated_at: 2026-04-29
canonical: true
related:
  - src/stores/notificationStore.ts
  - src-tauri/Cargo.toml
issue_source: batmania52 보고 (#6, 2026-04-29)
path_decision:
  selected: B
  reason: |
    batmania52 가 release DMG 와 로컬 빌드 모두 시도한 정황. 두 빌드 모두에서
    osascript 회귀 가능성 (C1) 또는 release DMG 만 회귀 (C3) 모두 Path B 가 정공법.
    Path A' (plugin is_dev() 분기 제거) 는 dev 한정 fix 라 release DMG 사용자에겐 효과 없음.
    외부 사용자에게 추가 round-trip 비용 > Path B 직진 비용.
  permission_ux: D (첫 알림 시 자동 prompt + Settings 토글 병행)
---

# Native notification 전환 — Path B 직접 ObjC bridge

## Context

batmania52 보고:
> "알림은.. apple script 말고 네이티브로 해주셨으면 하는 작은 바람이 있습니다. **누르면 apple script 앱이 뜹니다..**"

현재: `notificationStore.ts:137` 에서 `tauri-plugin-notification` 의 `sendNotification` 사용. Tauri 2 plugin-notification 의 macOS path 가 osascript 호출이라 알림 클릭 시 macOS 가 source 를 osascript binary 로 인식 → Script Editor.app (또는 osascript 자체) 가 frontmost.

추가 정황:
- 사용자가 release DMG 와 로컬 빌드 모두 시도. 두 빌드에서 같은 회귀 가능성 ↑.
- Path A' (is_dev() 분기 제거 / plugin fork) 는 dev 한정 부분 fix — release DMG 사용자 무관.
- 외부 사용자 round-trip 비용 회피 + 정공법 직진을 채택.

## Goals

- (G1) 알림 클릭 시 osascript / Script Editor 가 뜨지 않음. tunaFlow 본체가 frontmost 로 올라옴 (dev/release 둘 다).
- (G2) 알림 표시 자체는 macOS 표준 알림 센터 패턴 유지.
- (G3) cross-platform 호환 — Windows/Linux 의 알림 path 는 변경 없음 (이미 native).
- (G4) 권한 요청 UX — 첫 알림 발생 시 자동 prompt + Settings 에서 미리 켜는 토글 병행 (옵션 D).
- (G5) 권한 거부 시 graceful degrade — silent skip + 1회 console warning.

## Non-goals

- ❌ 알림 액션 (버튼 / 답장) 추가 — 기본 dispatch 만.
- ❌ 알림 sound / icon 커스터마이즈 — 기본 사용.
- ❌ Push notification (외부 서버) — 별 영역.
- ❌ Path A' (plugin is_dev() 분기 제거 fork) — 부분 fix 라 채택 안 함.
- ❌ tauri-plugin-notification 자체 제거 — Windows/Linux path 는 기존 plugin 그대로 사용.

## Subtasks

### Task 01 — Tauri plugin-notification source 진단 (fix 설계 일부) [P1]

**Changed files**: 없음 (조사)

**Change description**:
- `Cargo.toml` 의 `tauri-plugin-notification` 버전 확인 (현재 v2.3.3 추정)
- 해당 버전의 macOS path source code read (`~/.cargo/registry/src/...` 또는 GitHub):
  - 어떤 함수가 osascript 호출하는지
  - dev/release 분기 (`is_dev()` 호출 여부) 확인
  - 권한 요청 로직 유무
- 결과로 다음 결정:
  - Task 02 의 ObjC bridge 가 plugin 의 어떤 호출 chain 을 우회할지
  - frontend `notificationStore.ts:137` 의 `sendNotification` 호출을 그대로 두고 Rust 측만 가로챌지, 또는 새 Tauri command 로 우회할지

**Verification**:
- 진단 결과 chat 보고 — plugin macOS path 의 osascript 호출 위치 + dev/release 분기 유무 1줄
- Task 02 의 hook point 결정 + plan §Task 02 갱신 (필요시)

**회귀 위험 가드**:
- 코드 변경 0. 진단만.

### Task 02 — Direct ObjC bridge 구현 (UNUserNotificationCenter) [P2]

**Changed files**:
- `src-tauri/Cargo.toml` (`objc2` 또는 `cocoa` crate 추가, macOS-only feature)
- `src-tauri/src/notification.rs` (신규, ~150-250 LoC)
- `src-tauri/src/lib.rs` (Tauri command register)
- `src/lib/api/notification.ts` (신규 또는 수정, frontend wrapper)
- `src/stores/notificationStore.ts:137` (macOS 경로 새 command 호출, Windows/Linux 는 기존 plugin)

**Change description**:
- `objc2` crate 권장 (활발한 maintenance, 최신 macOS API 지원). `objc2-user-notifications` feature 사용.
- `src-tauri/src/notification.rs` 의 `cfg(target_os = "macos")` 영역에서 다음 함수 구현:
  - `request_authorization() -> Result<bool, AppError>` — `UNUserNotificationCenter::current().requestAuthorization(options:completionHandler:)` 호출. options = `[.alert, .sound]`.
  - `send_notification(title: String, body: String) -> Result<(), AppError>` — `UNMutableNotificationContent` + `UNNotificationRequest` + `add(request:withCompletionHandler:)`.
  - `notification_authorization_status() -> Result<NotificationAuthStatus, AppError>` — 현재 권한 상태 조회 (notDetermined / denied / authorized / provisional).
- macOS 외 OS (`#[cfg(not(target_os = "macos"))]`) → 기존 `tauri-plugin-notification` 통과 (변경 0).
- Tauri command 등록: `notification_send`, `notification_request_permission`, `notification_status`.
- `notificationStore.ts:137` 분기:
  ```ts
  const isMacOS = await import("@tauri-apps/api/os").then(m => m.platform()) === "macos";
  if (isMacOS) {
    await invoke("notification_send", { title, body });
  } else {
    const { sendNotification } = await import("@tauri-apps/plugin-notification");
    sendNotification({ title, body });
  }
  ```

**Verification**:
- macOS dev: 알림 1회 발송 → 클릭 → tunaFlow frontmost (osascript / Script Editor 안 뜸)
- macOS release (codesigned): 같은 시나리오 → tunaFlow frontmost
- 권한 거부 후 알림 발송 → silent skip + console warning 1회 (UI hang 없음)
- Windows/Linux dev 빌드 → 기존 알림 path 동작 확인 (회귀 0)
- `cd src-tauri && cargo check && cargo test --lib notification`
- `npx tsc --noEmit && npx vitest run src/stores/notificationStore`

**회귀 위험 가드**:
- macOS 외 path 절대 변경 금지 — `cfg(target_os = "macos")` 격리 엄격.
- frontend `sendNotification` 의 다른 호출처 (있다면) 도 같은 분기 적용 확인 (`rg "sendNotification" src/`).
- `objc2` crate 가 macOS 26.x (Tahoe) 호환 확인. 호환 안 되면 `cocoa` crate fallback (안정성 기존 검증).
- Bundle id 가 codesigned/Notarized release 에서 `UNUserNotificationCenter` 권한 요청에 영향 — `tauri.conf.json:identifier=com.tunaflow.app` 그대로. 미서명 dev 빌드도 권한 요청 가능 (macOS 가 dev signature 로 식별).

### Task 03 — 권한 요청 UX (옵션 D: 첫 알림 시 + Settings 토글 병행) [P2]

**Changed files**:
- `src/stores/notificationStore.ts` (권한 상태 state + 첫 알림 시 자동 prompt 분기)
- `src/components/tunaflow/settings/NotificationsSection.tsx` (신규 또는 기존 settings 섹션 보강)
- `src/locales/{ko,en}/settings.json` (라벨)

**Change description**:
- `notificationStore` 에 `permissionStatus: 'notDetermined' | 'denied' | 'authorized'` state.
- 첫 알림 발송 시점:
  1. `notification_status` 호출
  2. `notDetermined` → `notification_request_permission` 호출 (macOS native dialog 표시)
  3. 사용자 응답 후 status 갱신, authorized 시 알림 발송. denied 시 silent skip + console warning + UI 토스트 1회 ("알림 권한이 거부되었습니다. 시스템 설정에서 활성화 가능합니다.")
- Settings → Notifications 섹션:
  - 현재 status 표시 (notDetermined / denied / authorized)
  - "지금 권한 요청" 버튼 (notDetermined 또는 denied 일 때) → `notification_request_permission` invoke
  - denied 인 경우 macOS 시스템 설정 deep link 안내 (`x-apple.systempreferences:com.apple.preference.notifications`)

**Verification**:
- 첫 알림 발송 시 권한 dialog 자동 표시
- "허용" 선택 → 알림 표시 + 클릭 시 tunaFlow frontmost
- "거부" 선택 → silent skip + 토스트 1회
- Settings → Notifications 에서 미리 권한 요청 가능
- denied 상태에서 시스템 설정 link 동작
- `npx tsc --noEmit && npx vitest run`

**회귀 위험 가드**:
- 권한 요청은 *첫 알림 시* 또는 *Settings 명시 클릭* 시에만. 앱 시작 시 자동 spam 금지 (G1 정책 부합).
- 토스트 중복 방지 — denied 알림 토스트는 세션당 1회 (dismiss flag).
- Settings UI 의 다른 섹션 변경 금지.

## Cross-cutting risks

| 위험 | 대응 |
|---|---|
| `objc2` crate 가 macOS 26.x 회귀 (사용자 OS Tahoe) | 진단 단계 (Task 01) 에서 호환성 확인. 회귀 시 `cocoa` crate fallback. |
| 직접 ObjC bridge 의 권한 요청이 미서명 dev 빌드에서 거부됨 | dev 빌드도 dev signature 로 식별되므로 권한 요청 동작 (Apple 표준). 거부 시 graceful degrade. |
| frontend platform detection 비동기 (await import os) → 첫 알림 발송 지연 | 앱 시작 시 platform 한 번 cached → 이후 동기 분기. |
| 알림 click 시 frontmost 가 안 올라가는 추가 회귀 | UNNotificationRequest 의 default category 가 dispatch 처리. dock badge / 창 활성화 추가 검토 — Phase 2 후속. |

## Rollback

Task 02 단독 revert → frontend 분기는 macOS 도 기존 plugin path 로 복귀 (osascript 회귀 재발). Task 03 단독 revert 시 권한 UX 만 사라지고 알림 자체는 동작.

각 task 분리 commit 권장. cargo 빌드 실패 회복 위해 Task 02 의 Cargo.toml 변경은 별 commit.

## Phase 2 (out-of-plan, 후속 후보)

- 알림 액션 (버튼 — "답장" / "보기") 추가
- dock badge count
- 알림 카테고리/그룹화
- Linux native (libnotify direct) — 현재는 plugin 그대로
- Windows native (Toast Notifications direct) — 현재는 plugin 그대로
