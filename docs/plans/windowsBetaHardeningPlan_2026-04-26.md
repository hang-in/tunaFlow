---
title: Windows 베타 hardening — Windows 환경에서 직접 작업, macOS 무영향 invariant
status: in-progress (사용자 직접 Windows 환경에서 작업)
priority: P0 (Windows v0.1.2-beta 사용자 가시 issue 처리 + 정합 release)
created_at: 2026-04-26
related:
  - docs/plans/windowsBuildPlan_2026-04-24.md            # CI / matrix / NSIS bundle SSOT
  - docs/plans/selfTrustCiTriggerOptimizationPlan_2026-04-25.md  # CI policy
  - .github/workflows/build.yml                          # Release Build matrix
  - src-tauri/tauri.conf.json                            # bundle / icons / version
  - src-tauri/src/no_console.rs                          # Windows console suppression
canonical: true
owners:
  - architect (Windows 환경에서 직접 작업)
---

# 배경

2026-04-26 기준 v0.1.2-beta 가 Windows 첫 release. 사용자 첫 smoke test 결과:

- ✓ NSIS installer 설치 OK (백신 한 번 차단 → 예외 처리 후 통과)
- ✓ macOS dmg 빌드 정상 (회귀 없음)
- ⚠️ 첫 startup 시 응답 stuck — "streaming" 상태에서 timeout, 재시작 후 정상
- ⚠️ 기존 macOS 작업 DB 가 Windows 환경에서 로딩 hang (수십 초)

사용자 결정: **Windows 환경에서 직접 작업**. 매번 macOS 에서 push → Windows 검증 흐름이 비효율 (사용자 어록: "왔다갔다하니 효율이 별로 안좋네").

# Invariants — 절대 깨면 안 되는 제약

- **[INV-1]** 🔴 **macOS tunaFlow 에 사이드 이펙트 0** — 사용자 가장 강한 제약. 모든 Windows 작업은 다음 셋 중 하나로만:
  1. `#[cfg(target_os = "windows")]` / `#[cfg(windows)]` 블록 안에 격리
  2. macOS 에선 무관한 새 파일 추가 (e.g. `scripts/build-rawq.ps1`, `src-tauri/src/no_console.rs` 의 cfg 분기)
  3. 변경 후 *macOS CI 빌드 통과 확인* 필수 — main 직접 push 금지

- **[INV-2]** PR 형태 commit + CI watch 필수 — self-trust 정책의 *예외*. 본 작업은 *cross-platform 회귀 위험* 이 있어 `gh pr merge --admin` 즉시 머지 금지. CI 의 macOS + Windows 두 job 모두 ✓ 확인 후 머지.

- **[INV-3]** Windows 환경에서 macOS-specific 코드 변경 X — `src-tauri/src/bootstrap/env.rs` 의 macOS PATH 보강 같은 영역은 손대지 않음. 혹시 손대야 하면 macOS 에서 별도 PR.

- **[INV-4]** 단일 axis 한 번에 — 첫 startup race + DB hang + 백신 등 axis 가 별도라 *각각 별 commit*. 회귀 시 이등분 가능.

- **[INV-5]** 검증 안 한 commit push X — Windows dev 모드에서 *직접 smoke test* 후 push.

# 현재 상태 (2026-04-26)

## v0.1.2-beta 에 들어간 변경 (Windows-related, commit 시간순)

| Commit | 설명 |
|---|---|
| `da2e755` | UTF-8 char boundary panic fix (CJK byte slicing) |
| `1307f4b` / `ff096dd` | DB transaction wrap (cross-platform, 본 release 와 별 axis) |
| `967835a` | Windows build matrix + frontend `basename` utility |
| `5e80ce2` | workflow_dispatch VERSION fallback (CI smoke test) |
| `02e2f86` | tauri `--config` 영구 설정 (Windows shell escape 회피) |
| `94c698e` | `icon.ico` tauri-cli 재생성 (proper Windows ICO) |
| `6116a1b` | MSI bundle target 제외 (`-beta` prerelease identifier reject) |
| `fec7cd6` | manifest version bump 0.1.1 → 0.1.2-beta |
| `cf2a637` | `NoConsole` trait — 50 spawn sites cmd 창 깜박 fix |
| `26dfd32` | Splash UI + 백신 / SmartScreen 안내 강화 |
| `4d0f2e8` | CHANGELOG update |

CI: macOS 빌드 ✓, Windows 빌드 ✓ (run 24953057499). Release `v0.1.2-beta` draft 상태 (사용자 publish 결정 대기).

## 알려진 issue (사용자 보고)

| 우선순위 | 증상 | 추정 root cause | release blocker |
|---|---|---|---|
| **P1** | 첫 startup 후 첫 message 가 "streaming" 에서 stuck (재시작 시 정상) | (a) WS server listen race / (b) Windows Defender first-spawn scan latency / (c) selectProject background lock | No — workaround 있음 |
| **P1** | 기존 macOS DB 로딩 시 수십 초 hang | macOS path stale (DB row 의 path 가 `/Users/...`) → Windows file IO timeout | No — splash UI 가 단계 가시화 |
| **P2** | Windows Defender / 백신 false positive | unsigned NSIS installer | No — INSTALL.md 안내 + Authenticode 후속 |
| P3 | `agents/context_hub.rs:99-102` / `agents/opencode.rs:36-38` 의 macOS-only path probe | 본 release scope 외 — Lite 트랙 우선 | No |
| P3 | `bootstrap/env.rs` PATH 보강이 macOS-only — Windows 분기 추가 안 됨 | 보통 Windows PATH 가 system-level 이라 영향 적음 | No |

# 작업 우선순위 (Windows 환경에서)

## P1 — 첫 startup race condition

**진단 가설**:
- (a) Tauri `setup()` 의 axum WS server start 와 사용자 첫 message spawn 의 timing race. WS server listen 전에 `claude --sdk-url` connect → connection refused → claude side timeout
- (b) Windows Defender real-time scan 이 새 binary 첫 spawn 시 1~5s freeze
- (c) AppShell `selectProject` background DB / vector indexing lock

**진단 step**:
1. Production NSIS install 후 backend stderr log 확인 — 위치 후보:
   - `%LOCALAPPDATA%\com.tunaflow.app\logs\`
   - `%APPDATA%\tunaflow\`
   - `%TEMP%\tunaflow*.log`
   - 없으면 dev 모드 `npm run tauri dev` 으로 stderr 확인
2. 첫 startup 후 *1분 기다리고* 메시지 보내면 정상인지 확인 → race 확정 (a 또는 c)
3. 즉시 보낼 때만 fail 이면 `setup()` 의 axum start 시점에 `tracing::info!("[ws] listening on {}", port)` 로그 + 사용자 메시지 spawn 시점 비교

**Fix 후보** (진단 결과에 따라):
- (a) Backend `setup()` 안에서 *WS server ready signal* 까지 await 한 후 frontend 에 ready 이벤트 emit. Frontend 가 ready 받기 전엔 send 비활성
- (b) Windows-only `tokio::time::sleep(Duration::from_millis(500))` 첫 spawn 전 buffer (Defender 우회) — 임시 안전망
- (c) `selectProject` 안의 vector indexing 자동 시작을 *opt-in* 으로 변경 (사용자가 첫 메시지 보낸 후로 미루기)

### Status (2026-04-30 갱신)

- **진단 도구 도입 완료** — PR #233 (Track 2 A 안) 으로 `claude_sdk_session.rs` 의 stderr 가 `Stdio::null()` → `Stdio::piped()` + `[sdk-session-stderr]` 라인 forward. 다음 cold start 시 root cause 가 backend stderr 에 자동 노출.
- **정적 분석 결과** (디벨로퍼 보고): 가설 (a) WS race 는 unlikely (claude 가 그 포트 안 씀, per-session WS micro-race 는 OS backlog 로 가려짐), 가설 (c) selectProject lock 은 정적으로 미발견. **가설 (b) Defender first-spawn 가장 유력**.
- **베타 publish 결정 (사용자 2026-04-30)** — 옵션 C 채택. 본 §B fix 는 v0.1.4-beta 에 포함하지 않고 **알려진 이슈로 등재**, 1분 후 재시도 workaround 안내. CHANGELOG `[0.1.4-beta]` 의 *Known issues (Windows)* 섹션 (2026-04-30 추가) 참조.
- **Fix axis 진행 시점** — 다음 자연 cold start 시 `[sdk-session-stderr]` 캡처 결과 paste 받은 후 가설 확정 → 별 PR 진행. 또는 v0.1.5 정식 release 사이클에 묶음 진행.
- **즉시 fix 가능한 안전망 옵션 (보류)** — defensive 로 (b) 가설 가정한 cfg(windows) `tokio sleep(500ms)` first-spawn buffer 추가는 잘못된 가설이어도 fail-safe (정상 spawn 영향 0). 사용자가 즉시 안전망 원할 시 별 axis 로 진행 가능. 현재는 stderr 캡처 우선 정책 유지.

## P1 — DB path stale

**진단**: 기존 macOS DB 의 `projects` 테이블 row 의 `path` column 이 `/Users/d9ng/...`. Windows 에서 그 경로 file IO 시도 → no such directory or timeout.

**Fix 옵션**:
- **(A)** 첫 startup 시 모든 project row 의 path validate (`Path::new(&p).exists()`) → invalid 시 *projects 화면으로 fallback* 또는 *경로 재선택 prompt*
- **(B)** OS 별 DB 분리 — Windows 의 AppData 에 별 SQLite. 기존 macOS DB 무관
- **(C)** project path 의 OS-aware migration — `/Users/<user>` → `C:\Users\<user>` (사용자 매핑 필요)

(A) 가 가장 간단 + 안전. 사용자가 직접 path 다시 선택하면 됨. (B) 는 큰 변경.

## P2 — Windows Defender / 백신 false positive

- 단기: INSTALL.md 안내 (이미 적용됨)
- 장기: **Authenticode 코드 서명** — Azure Trusted Signing 또는 EV cert (~$500/year). 후속 plan.

# Windows 환경 setup

## 1차 설치 (사용자 본인 Windows 머신)

```powershell
# Rust toolchain (rustup-init.exe)
winget install --id Rustlang.Rustup
rustup default stable

# Node.js
winget install --id OpenJS.NodeJS

# Visual Studio Build Tools (C++ workload)
winget install --id Microsoft.VisualStudio.2022.BuildTools

# WebView2 (대부분 사전 설치됨)

# Git for Windows
winget install --id Git.Git

# 저장소 clone
git clone https://github.com/hang-in/tunaFlow C:\privateProject\tunaFlow
cd C:\privateProject\tunaFlow

# Rust 의존성 빌드
npm install --no-audit --no-fund
cd src-tauri && cargo build && cd ..

# rawq 사이드카 빌드 (PowerShell 스크립트)
.\scripts\build-rawq.ps1

# dev 모드 실행
npm run tauri dev
```

## 검증 흐름 (매 commit 전)

```powershell
# 1. Rust check
cd src-tauri
cargo check
cargo test --lib

# 2. Frontend check
cd ..
npx tsc --noEmit
npx vitest run

# 3. dev 모드 smoke
npm run tauri dev
# - 첫 startup splash 확인
# - 메시지 1회 전송
# - cmd 창 깜박 안 보이는지
# - branch 1회 생성

# 4. release 빌드 (선택, 시간 들음)
npm run tauri build
# 결과: src-tauri/target/release/bundle/nsis/tunaFlow_*-setup.exe
```

# Architect 호출 시 read-first list

Windows 환경에서 새 architect 세션 시작 시 다음 SSOT 들 우선 읽고 작업:

1. **본 plan** — 현재 상태 + Invariants + 작업 우선순위
2. `docs/plans/windowsBuildPlan_2026-04-24.md` — CI / matrix / bundle SSOT
3. `docs/reference/branchSessionPolicy.md` — brand session 정책 (PR #198 후속)
4. `docs/reference/flexboxConventions.md` — frontend layout invariant
5. `CLAUDE.md` — top-level handoff + 빌드 / 실행 / 테스트 명령

# 본 plan 의 Architect 자기참조 프롬프트 (사용자 본인 작업용)

```
[작업] Windows 베타 hardening — Windows 환경에서 직접 작업

[SSOT] docs/plans/windowsBetaHardeningPlan_2026-04-26.md

[현재 상태]
- v0.1.2-beta release draft 생성 완료 (CI 통과, 사용자 publish 결정 대기)
- 사용자 smoke test 결과 P1 issue 2개 확인 (첫 startup race, DB path stale)
- macOS / Windows 둘 다 빌드 정상

[다음 작업 — 우선순위 순]
1. 첫 startup race 진단 (backend log / 1분 대기 테스트)
2. DB path stale → invalid path 시 projects 화면 fallback
3. (선택) Windows Defender 우회 — 첫 spawn buffer

[Invariants — 절대 위반 X]
- macOS 영향 0: 모든 변경은 #[cfg(windows)] 블록 또는 OS-aware 파일에만
- 매 commit PR 형태 + macOS CI 통과 확인 후 admin merge (예외적으로 watch 필수)
- 단일 axis 단일 commit (회귀 시 이등분 가능)
- Windows dev 모드 smoke test 후 push

[검증]
- cargo check / cargo test --lib (Rust)
- npx tsc --noEmit / npx vitest run (Frontend)
- npm run tauri dev → smoke
- gh pr create → CI watch (macOS + Windows 둘 다 ✓)
- gh pr merge --admin (자체 PR)

[커밋 분리 가이드]
- fix(windows-startup): 첫 startup race fix
- fix(windows-db): macOS path stale fallback
- (각각 별 commit)
```

# 후속 / Sibling

- `metaFloatingChatPosClampPlan_2026-04-25` — frontend 의 다른 axis (이미 PR #205 머지)
- `mainChatBrandRunningGuardPlan_2026-04-25` — brand running guard (이미 PR #208 머지)
- (P2 후속 plan 후보) **Authenticode 코드 서명 도입** — 백신 false positive 영구 fix
- (P2 후속 plan 후보) **DB OS 분리 또는 path migration tool** — 기존 macOS DB 와 Windows 환경 정합성
