---
title: Windows 환경 Architect 세션 — v0.1.4-beta hardening 핸드오프
plan: docs/plans/windowsBetaHardeningPlan_2026-04-26.md
created_at: 2026-04-29
target_environment: Windows (사용자 본인 머신)
calling_role: architect
session_branch_strategy: feature 브랜치 + PR + CI watch (INV-2)
---

# Windows Architect 세션 — v0.1.4-beta hardening 핸드오프

당신은 사용자의 Windows 머신에서 직접 작업하는 **architect** 세션입니다.
macOS 측 main 은 이미 v0.1.4-beta 분량(`8aa944c`)이 머지된 상태이고,
오늘은 Windows 측 hardening + Windows 자산을 v0.1.4-beta release 에 매칭시키는
일을 합니다.

## 0. 작업 시작 전 — 반드시 read-first

다음 SSOT 들을 **순서대로** 읽고 시작합니다. 각 문서의 의도/제약을 이해하지
않은 상태로 코드를 손대지 마세요.

1. **`docs/plans/windowsBetaHardeningPlan_2026-04-26.md`** — Invariants (INV-1~4),
   알려진 issue 표 (P1 startup race, P1 DB path stale, P2 Defender), 작업 우선순위.
2. **`docs/plans/windowsBuildPlan_2026-04-24.md`** — CI matrix / NSIS bundle SSOT.
3. **`CHANGELOG.md`** 의 `[0.1.4-beta] - 2026-04-29` 섹션 — 오늘 release 에
   포함되어야 할 macOS 측 변경 (claude transport flip + result.md contamination
   + watchdog/reviewer guard). Windows 자산도 같은 release 에 묶임.
4. **`docs/plans/claudeResumeSessionTransitionPlan_2026-04-29.md`** — claude
   2.1.121 정책 차단 후 transport 전환. Windows 환경에서도 동일 path 동작 검증
   필요 (claude CLI 동일 동작 확인).
5. **`docs/plans/resultMdContaminationFixPlan_2026-04-29.md`** + 머지 PR #211
   — ContextPack/reportSync 변경. Windows 무관해야 함, INV-1 회귀 검증 항목.
6. **`docs/plans/watchdogAndReviewerReadGuardPlan_2026-04-29.md`** + 머지 PR #212
   — claude.rs watchdog RAII + reviewer template. **Windows compat 점검 필요**
   (§D 참고 — `Command::new("kill")` 는 Unix only).

## 1. Invariants — 절대 깨지 마세요

`windowsBetaHardeningPlan_2026-04-26.md` §27-44 의 INV-1~4 그대로 적용:

- **[INV-1] 🔴 macOS tunaFlow 에 사이드 이펙트 0**. Windows 변경은 다음 셋 중 하나로만:
  1. `#[cfg(target_os = "windows")]` / `#[cfg(windows)]` 블록 안에 격리
  2. macOS 무관한 새 파일 추가 (e.g. `scripts/build-rawq.ps1`)
  3. 변경 후 *macOS CI 빌드 통과 확인* 필수 — main 직접 push 금지
- **[INV-2]** PR + CI watch 필수. self-trust 정책의 *예외* — Windows 작업은 cross-platform 회귀 위험. `gh pr merge --admin` 즉시 머지 **금지**. CI 의 macOS + Windows 두 job 모두 ✓ 확인 후 머지.
- **[INV-3]** Windows 환경에서 macOS-specific 코드 변경 X (`bootstrap/env.rs` 의 macOS PATH 보강 등 손대지 않음).
- **[INV-4]** 단일 axis 한 번에 — startup race + DB path + watchdog compat 등 axis 가 별도라 *각각 별 commit*. 회귀 시 이등분 가능.

## 2. 환경 점검 (작업 시작 직후)

```powershell
# 위치 확인
cd C:\privateProject\tunaFlow
git status                       # working tree clean 이어야 함
git fetch origin
git pull origin main             # 최신 main 동기화 (8aa944c 또는 그 이후)
git log --oneline -10            # PR #211, #212 머지 commit 확인

# Toolchain
rustc --version                  # stable
node --version
npm --version
where claude                     # claude CLI 설치 확인 (transport flip 검증용)
```

3. 미설치/오래된 toolchain 발견 시 → `windowsBetaHardeningPlan_2026-04-26.md` §114 1차 설치 단계 따라 보강.

## 3. 작업 순서 — A → C → B (오늘 권장 흐름)

### A. v0.1.4-beta Windows 자산 빌드 매칭 [P0, release 묶음]

**목표**: main HEAD (`8aa944c` 또는 이후) 에서 Windows NSIS installer 빌드,
v0.1.4-beta Draft release 자산에 업로드.

#### A-1. dev 모드 smoke (코드 회귀 없음 확인)

```powershell
npm install --no-audit --no-fund
cd src-tauri ; cargo check ; cd ..
npx tsc --noEmit
npx vitest run
cd src-tauri ; cargo test --lib ; cd ..
```

baseline 카운트 (PR #212 직후 main): **FE 381 / Rust 559**. 같거나 +N 이어야 함.

#### A-2. dev 모드 실행 + manual smoke

```powershell
npm run tauri dev
```

확인 항목:
- 첫 startup 후 splash → 메인 화면 정상 도달 (P1 startup race 가 다시 보일
  수 있으나 본 task A 의 차단 사유 아님 — 다음 트랙 C/B 에서 다룸)
- claude 메시지 1회 전송. backend stderr 에 `[guardrail] engine=claude-resume status=ok` 가 떠야 함 (transport flip 정상 동작 확인)
- 응답이 30s timeout 안 나는지 (claude 2.1.121 정책 차단 회복 검증)
- branch 1회 생성

이슈 발견 시 → 작업 중단 + chat 보고. trafficc fix 들어가기 전 사용자 결정.

#### A-3. release 빌드

```powershell
npm run tauri build
```

산출물 확인:
- `src-tauri\target\release\bundle\nsis\tunaFlow_*-setup.exe`
- 파일 크기 / 빌드 시간 chat 보고

#### A-4. installer smoke (별도 테스트 머신 또는 같은 머신 install)

- 설치 후 첫 launch
- 한 번 메시지 전송 → 응답 정상
- Windows Defender / 백신 차단 여부 (P2, 차단되면 INSTALL.md 가이드 적용 후 재시도)

#### A-5. v0.1.4-beta Draft release 자산 업로드

```powershell
gh release upload v0.1.4-beta `
  src-tauri\target\release\bundle\nsis\tunaFlow_*-setup.exe
```

태그가 아직 없거나 Draft 가 다른 commit 기준이면 사용자에게 escalate (release ops 결정 영역).

---

### C. P1 — DB path stale fix [hardening plan §96 option A] [P1, 사용자 가시성]

**목표**: 첫 startup 시 `projects.path` 가 OS 다른 경로 (예: `/Users/...`) 라
file IO timeout 으로 hang 되는 회귀 차단.

#### C-1. 코드 위치 확인

후보 파일:
- `src-tauri/src/bootstrap/db.rs` (DB load 단계)
- `src-tauri/src/commands/projects.rs` (project list / load)

`grep -n "projects.path\|fn list_projects\|Path::new" src-tauri/src/bootstrap/ src-tauri/src/commands/projects.rs` 같은 명령으로 정확한 hook point 찾기.

#### C-2. 변경 설계

`projects` 테이블 row 의 `path` 컬럼을 startup load 시점에 validate:

```rust
let path_ok = std::path::Path::new(&p.path).exists();
if !path_ok {
    // 1. log warning (eprintln) — 어떤 path 가 invalid 인지
    // 2. UI 에 "이 프로젝트 경로를 다시 선택하세요" prompt 표시
    //    (또는 projects 선택 화면으로 fallback)
    // 3. DB row 는 삭제하지 말 것 — 사용자가 경로 매핑 후 update 가능
}
```

INV-1 격리: 이 logic 이 macOS / Linux 에서도 합법적으로 동작 (어차피 path 가 invalid 면 어디서든 fallback 이 옳음). 그러므로 cfg(windows) 분기 **불필요** — cross-platform 으로 켜도 무방.

다만 macOS 측 회귀 위험 0 임을 검증하기 위해 macOS valid path 케이스에서 unit test 1개 추가 권장.

#### C-3. 변경 후 검증

```powershell
cd src-tauri ; cargo check ; cargo test --lib bootstrap ; cd ..
npm run tauri dev
# - macOS 가 만든 stale DB 가 있으면 그걸로 띄워서 fallback 동작 확인
# - 또는 path 일부러 invalid 로 만들어 테스트
```

#### C-4. PR

```powershell
git checkout -b fix/windows-db-path-stale-fallback
git add src-tauri/src/...
git commit -m "fix(bootstrap): validate project path on startup, fallback if invalid (Windows hardening)"
git push -u origin fix/windows-db-path-stale-fallback
gh pr create --title "fix(bootstrap): project path validate + fallback (Windows P1)" --body "<plan link + verification>"
gh pr checks --watch          # CI 통과 대기 (INV-2)
gh pr merge --merge           # CI 통과 후 머지 (admin 금지)
```

---

### B. P1 — startup race 진단 [P1, 진단 시간 가변]

**목표**: 첫 startup 후 첫 message "streaming" stuck 의 root cause 좁히기.

#### B-1. 진단 — Plan §82 step 따라

```powershell
# 1. backend stderr 로그 위치 확인
ls $env:LOCALAPPDATA\com.tunaflow.app\logs\ 2>$null
ls $env:APPDATA\tunaflow\ 2>$null
ls $env:TEMP\tunaflow*.log 2>$null

# 없으면 dev 모드 stderr 로 진단
npm run tauri dev > tunaflow.dev.log 2>&1
# 첫 startup 후 즉시 메시지 보내기 + stuck 확인
# 1분 기다린 후 다시 보내기 → 정상이면 race 확정 (a 또는 c)
```

#### B-2. 가설 좁히기

`tunaflow.dev.log` 에서 다음 timing 확인:
- `[ws]` 또는 axum 시작 시점
- 첫 `[guardrail]` 호출 시점
- 두 사이 간격 / connection refused 메시지 유무

확정 후:
- (a) WS server listen race → backend `setup()` 에 ready signal + frontend ready 이벤트 emit
- (b) Defender first-spawn → cfg(windows) `tokio::time::sleep(500ms)` 임시 안전망
- (c) selectProject lock → vector indexing opt-in 으로 변경

각 fix 후보별 **별 PR + CI watch**. axis 분리 (INV-4).

본 트랙은 진단이 우선 — 가설 확정 전에 fix 들어가지 마세요. 진단 완료까지 chat 보고로 사용자에게 가설 + 다음 step 알리기.

---

### D. (보조) Watchdog kill compat 점검 [P3, 별도 axis]

PR #212 의 `src-tauri/src/agents/claude.rs:251` 에서 `Command::new("kill").arg("-9")` 호출. 이는 **Unix only** — Windows 에는 `kill` 명령이 없음.

영향:
- watchdog 가 idle_timeout 도달 시 child process 를 **죽이지 못함**. RAII guard 가 done flag 를 set 하므로 watchdog thread 자체는 정상 break, 다만 child claude.exe 는 zombie 로 남음.
- 실제 occurrence 는 idle 600s 일 때만이라 빈도 낮음. 그러나 release blocker 는 아니지만 짚어둘 가치 있음.

**Fix 방향** (별도 PR, axis 분리):

```rust
#[cfg(unix)]
let _ = std::process::Command::new("kill")
    .no_console()
    .arg("-9")
    .arg(child_id.to_string())
    .output();

#[cfg(windows)]
let _ = std::process::Command::new("taskkill")
    .no_console()
    .arg("/F")
    .arg("/PID")
    .arg(child_id.to_string())
    .output();
```

- `claude.rs` 의 같은 함수 내 cfg 분기. 다른 함수/파일 변경 금지.
- macOS 빌드 회귀 0 (cfg(unix) 가 macOS 에 활성화됨).
- 본 트랙 D 는 A/C/B 다음 우선순위.

## 4. 회귀 가드 (작업 후 매번)

각 PR 머지 전:

```powershell
# 1. macOS CI job pass 확인 (INV-1)
gh pr checks <pr-number>           # macos-latest job 도 ✓ 인지
# 2. Windows CI job pass 확인
# 3. test count baseline 비교 (감소 시 회귀)
# 4. PR description 에 read-first list / verification 결과 / INV 위반 없음 명시
```

## 5. CI 정책 (Windows hardening 한정)

- **PR + CI watch** 필수 (INV-2). admin merge 금지. macOS + Windows 두 job 모두 ✓ 후 머지.
- main 직접 push 금지 (PR 만).
- 머지 후 main HEAD 로 v0.1.4-beta 자산 재빌드 (트랙 A 의 A-3 ~ A-5 반복).

## 6. 보고 포맷 (각 트랙 완료 시 chat)

- 트랙 ID + 변경 라인 수 + 핵심 파일
- 각 verification 결과 (PASS/FAIL + 핵심 출력)
- baseline 대비 테스트 카운트 (FE/Rust)
- PR URL + CI 양쪽 ✓ 확인 / 머지 commit hash
- INV 위반 없음 확인 (macOS-specific 파일 미변경 grep 결과)
- 다음 트랙 진행 여부 또는 escalate 필요 사유

## 7. 막히면 (escalate)

- claude transport (PR #211 결과) 가 Windows 에서 다르게 동작 — chat 보고 후 대기. 임의 fix 금지.
- Defender 가 NSIS installer 를 자동 격리 — INSTALL.md 안내로 우회 가능하면 그대로, 자동 격리 자체가 release blocker 면 chat 으로 escalate.
- 진단 (트랙 B) 이 1시간 이상 답 안 나오면 가설 정리해서 chat 보고. fix 시도 전 사용자 판단 받기.
- main 에 macOS-specific 회귀 의심 (CI macOS job 실패) → 즉시 PR 닫고 별 PR 로 회귀만 fix.

## 8. 오늘 작업 종료 시 정리

- 머지된 PR 목록 + 머지 commit hash
- v0.1.4-beta Windows 자산 업로드 여부
- 남은 트랙 (B 는 진단 시간 변동 → 미완료 가능) 의 다음 step
- `windowsBetaHardeningPlan_2026-04-26.md` 의 status 갱신 (in-progress → A 완료/C 완료/B 진단중 등)
- chat 으로 macOS 측 (Architect) 에게 day-end 보고 — release publish 진행 가능 여부 한 줄 결론
