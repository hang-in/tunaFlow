---
title: Windows 의존성 부트스트랩 — Developer 세션 핸드오프
plan: docs/plans/windowsDependencyBootstrapPlan_2026-04-29.md
created_at: 2026-04-29
calling_role: architect (Windows 머신)
target_role: developer (신규 세션, 프로젝트 디렉토리 cwd)
session_branch_strategy: task 단위 별 브랜치 + PR + CI watch (INV-2)
---

# Windows 의존성 부트스트랩 Developer 핸드오프

당신은 사용자의 **Windows 머신** 에서 새 세션으로 진입한 **developer** 입니다.
본 세션 이전 컨텍스트가 전혀 없으므로 §0 read-first 를 *반드시 그 순서대로* 읽고 시작합니다.
코드를 손대기 전에 §1 invariants 와 §2 환경 점검을 마쳐야 합니다.

이 세션의 목표: **Plan `windowsDependencyBootstrapPlan_2026-04-29.md` §5 의 T1~T7
을 한 task 씩 별 commit + 별 PR 로 구현**. macOS 사이드이펙트 0 을 최우선
으로 한다.

---

## 0. 작업 시작 전 — 반드시 read-first

다음을 **순서대로** 읽고 시작합니다. 각 문서의 의도/제약을 이해하지 않은 상태로
코드를 손대지 마세요.

1. **`CLAUDE.md`** (프로젝트 루트) — tunaFlow 프로젝트 전체 핸드오프. §1~§5 (개요/스택/아키텍처/현재 상태) + §15 작업 안전 규칙 + §16 코딩 컨벤션 + §17 개발 도구 활용.
2. **`docs/reference/dataModelRevised.md`** — 도메인 모델 SSOT. 본 task 는 새 entity 추가 없으므로 빠르게 훑기.
3. **`docs/reference/coding-convention.md`** — 코딩 컨벤션. 한국어 응답 / Zustand selector / 5-engine parity / cfg 분기 패턴.
4. **`docs/reference/work-safety.md`** — 작업 안전 규칙. UI 진입점 변경 전 대체 경로 작동 확인 / 한 번에 한 경로만 수정.
5. **`docs/reference/tool-usage.md`** — 개발 도구 활용. `find → fd` / `grep → rg` / `sed → sd` / 멀티 파일 치환은 `fd ... | xargs sd ...`.
6. **본 plan**: `docs/plans/windowsDependencyBootstrapPlan_2026-04-29.md` — invariants + 의존성 매트릭스 + Phase + T1~T7 작업 분해 + 회귀 가드. **이 문서가 SSOT**.
7. **관련 핸드오프** (architect 가 이미 처리한 axis 와 axis 분리 위해 컨텍스트 확보):
   - `docs/prompts/windowsBetaHardeningArchitectHandoff_2026-04-29.md` — 별 axis (startup race / DB path / watchdog compat).
   - 직전 머지된 fix: `fix(conventions): normalize @import path to forward-slash on Windows` (PR #213, conventions_sync.rs path separator). 본 task 와 무관하나 같은 Windows hardening 흐름.

---

## 1. Invariants — 절대 깨지 마세요

본 plan §1 그대로 적용. 요약:

- **[INV-1] 🔴 macOS tunaFlow 에 사이드 이펙트 0**. Windows 변경은:
  1. `#[cfg(target_os = "windows")]` / `#[cfg(windows)]` 안에 격리, 또는
  2. macOS 무관 새 파일 추가 (e.g. `dependency_install.rs`), 또는
  3. cross-platform 변경 시 *macOS CI 빌드 통과 확인* 후에만 머지.
- **[INV-2]** PR + CI watch 필수. macOS + Windows CI 양쪽 ✓ 후 머지. `gh pr merge --admin` 금지.
- **[INV-3]** macOS-specific 코드 (`bootstrap/env.rs` macOS PATH 보강 등) 변경 X.
- **[INV-4]** 단일 axis per commit. T1 과 T2 같은 PR 에 묶지 말 것 — 회귀 시 이등분 가능해야 함.
- **[INV-DEP-A]** 자동 설치는 user consent 후에만. silent global install 금지.
- **[INV-DEP-B]** 설치 실패 시 graceful degradation. 앱 진입 차단 금지.
- **[INV-DEP-C]** README 표기와 실제 동작 일치.

---

## 2. 환경 점검 (작업 시작 직후)

```powershell
# 위치 확인
cd D:\privateProject\tunaFlow
git status                       # working tree clean 이어야 함
git fetch origin
git pull origin main             # 최신 main 동기화
git log --oneline -10            # PR #213 머지 commit + cc3e14e 이후 head 확인

# Toolchain
rustc --version                  # stable
node --version
npm --version
where chub                       # %APPDATA%\npm\chub.cmd 가 떠야 함 (architect 가 수동 설치 완료)
where code-review-graph          # <python>\Scripts\code-review-graph.exe
where claude                     # claude CLI
```

### Baseline (회귀 비교 기준)

작업 시작 전 baseline 카운트를 *반드시 기록*. 이후 매 PR 마다 비교.

```powershell
npm install --no-audit --no-fund   # 보통 변동 없음
cd src-tauri ; cargo check ; cd ..
npx tsc --noEmit                   # 통과해야 함
npx vitest run                     # 기록: FE __ tests
cd src-tauri ; cargo test --lib ; cd ..   # 기록: Rust __ passed
```

기준치 (PR #213 머지 직후): **FE 381 / Rust 558 (Windows). macOS 측 559**.
같거나 +N 만 허용. 감소 시 회귀.

---

## 3. 작업 순서 — T1 → T2 → T3 → T4 → T5 → (T6 → T7)

각 task 는 **별 commit + 별 PR + CI watch + 머지** 한 사이클.
이전 task 의 PR 이 머지되어야 다음 시작.

### T1 — `context_hub::resolve_bin()` Windows 호환 보강 [P0]

- **파일**: `src-tauri/src/agents/context_hub.rs`
- **현황**: PATH fallback 으로 chub.cmd 인식 가능 *should be*. 현재 candidates Vec 은 unix HOME 기반 + `/usr/local/bin/...` 등 unix 절대 경로뿐. Windows native process 에서 `HOME` 미설정.
- **변경 핵심**:
  ```rust
  #[cfg(target_os = "windows")]
  {
      if let Ok(appdata) = std::env::var("APPDATA") {
          c.push(PathBuf::from(&appdata).join("npm").join("chub.cmd"));
      }
      if let Ok(userprofile) = std::env::var("USERPROFILE") {
          c.push(PathBuf::from(&userprofile).join("AppData").join("Roaming").join("npm").join("chub.cmd"));
      }
  }
  ```
  (HOME 분기 그대로 유지 — Linux/macOS 호환.)
- **테스트**:
  - 단위: temp dir + `set_var("APPDATA", ...)` 모의로 candidate 가 결과에 포함되는지 (`#[cfg(windows)]` 게이트).
  - 통합 (수동): dev 모드 재시작 → Settings → Runtime → context-hub 카드 `ready`.
- **회귀 가드**: macOS path (`/usr/local/bin/chub`) 검사 여전히 동작.
- **PR title**: `fix(context-hub): Windows resolve_bin candidate paths (npm global)`
- **branch**: `fix/win-context-hub-resolve`

### T2 — `crg::resolve_bin()` Windows 호환 추가 [P0]

- **파일**: `src-tauri/src/agents/crg.rs`
- **현황**: candidate 가 `~/.local/bin/code-review-graph`, `/opt/homebrew/...`, `/usr/local/bin/...` (unix 위주). PATH fallback 유무 코드 확인 후 결정.
- **변경 (가장 단순한 안)**:
  ```rust
  // After existing unix candidates:
  for name in ["code-review-graph"] {
      if Command::new(name).no_console().arg("--version")
          .stdout(Stdio::null()).stderr(Stdio::null())
          .output().map(|o| o.status.success()).unwrap_or(false)
      {
          return Ok(PathBuf::from(name));
      }
  }
  ```
  PATH fallback 이라 macOS/Linux 도 동일 코드 — but 이미 unix candidate 가 먼저 hit 하므로 동작 변경 없음 (INV-1 안전).
- **테스트**: T1 패턴.
- **PR title**: `fix(crg): Windows PATH fallback for code-review-graph binary`
- **branch**: `fix/win-crg-resolve`

### T3 — README/INSTALL.md 자동 설치 문구 정정 [P0, docs-only]

- **파일**: `README.md`, `README.ko.md`, `INSTALL.md`
- **변경**: *"Auto-installed on first run"* → 사용자가 결정한 방향 (consent UX 로 가는 것이 권장. plan §1 INV-DEP-C 참조).
- **PR title**: `docs: align context-hub install wording with actual UX (consent-first)`
- **branch**: `docs/install-wording`
- **검증**: docs only — `npm run` 변동 없음. CI lint 만.

### T4 — First-run consent dialog + auto-install [P1]

- **신규 파일**:
  - `src-tauri/src/commands/dependency_install.rs` (Tauri command + install runner)
  - `src/components/tunaflow/FirstRunDependencyDialog.tsx`
- **로직** (plan §4 Phase 2 T4):
  1. 앱 시작 시 `setting("first_run_dependency_check_done")` 검사. 미수행이면 dialog 표시.
  2. detect: chub, code-review-graph 별 `available: bool, command: String, requires: String`.
  3. dialog: 항목별 체크박스 + "건너뛰기" / "설치".
  4. invoke `install_dependency(name)`:
     - chub: `Command::new("npm").args(["install", "-g", "@aisuite/chub"])`
     - crg: `Command::new("pip").args(["install", "code-review-graph"])`
  5. timeout (npm 60s / pip 120s) — INV-DEP-B graceful failure.
  6. 결과 이벤트 `dependency:install_result` emit.
- **i18n**: `src/locales/{ko,en}/dialog.json` 신규 키.
- **테스트**:
  - vitest: dialog snapshot + "건너뛰기" 시 setting 갱신.
  - cargo: install_dependency 의 명령 escape 회귀 (R-1).
- **INV-DEP-A** 충족: consent 후에만 실행.
- **PR title**: `feat(installer): first-run dependency consent dialog + auto-install`
- **branch**: `feat/first-run-dependency-dialog`

### T5 — Settings → Runtime 수동 설치 트리거 [P1]

- **파일**: `src/components/tunaflow/settings/RuntimeSection.tsx` (`ContextHubPanel`, CRG 섹션이 있다면 동등 위치).
- **변경**: 카드의 `unavailable` 상태일 때 "Install via npm/pip" 버튼 노출. 클릭 시 T4 의 `install_dependency` invoke.
- **macOS 영향**: 이미 설치된 사용자에게는 `available:true` 라 버튼 hidden — 영향 0 (INV-1).
- **PR title**: `feat(runtime-settings): manual install trigger for context-hub & crg`
- **branch**: `feat/runtime-manual-install`

### T6 — vendor skills 를 NSIS installer 에 번들 [P2]

- **파일**: `src-tauri/tauri.conf.json` (resources), `src-tauri/src/bootstrap/services.rs` (first-run unpack).
- 사용자 결정 Q-2 에 따라 *P3 로 미룰 수 있음*. P2 채택 시:
  - build 시 `_research/_skills` snapshot 을 `resources` 에 포함 (~5~15MB).
  - 첫 실행 시 `~/.tunaflow/skills/` 비어있으면 unpack.
- **branch**: `feat/skills-bundle`

### T7 — installer post 단계 PATH 갱신 안내 [P2]

- **파일**: NSIS .nsi 또는 `tauri.conf.json` bundle 설정.
- **branch**: `chore/installer-path-notice`

---

## 4. 회귀 가드 — 매 PR 마다 확인

```powershell
# 1. 코드 회귀
cd src-tauri ; cargo check ; cargo test --lib ; cd ..
npx tsc --noEmit
npx vitest run

# 2. baseline 비교 — FE/Rust 카운트 동일 또는 +N
# 3. macOS-specific 파일 미변경 grep
git diff main..HEAD -- src-tauri/src/bootstrap/env.rs scripts/build-rawq.sh   # 비어야 함

# 4. PR description 에 다음 명시:
#    - read-first list (§0 어디까지 읽었는지)
#    - 변경 axis 1개임 (INV-4)
#    - macOS 영향성 평가
#    - 회귀 카운트 비교
```

---

## 5. PR 생성 / CI watch 정책

```powershell
git checkout -b <branch>
git add <changed-files>             # axis 외 파일 add 금지
git commit -m "<message>"
git push -u origin <branch>
gh pr create --base main --head <branch> --title "<title>" --body "<body>"
gh pr checks <pr-number> --watch    # macOS + Windows 양쪽 ✓ 까지 대기 (INV-2)
gh pr merge <pr-number> --merge     # admin 금지 (INV-2)
```

PR body 템플릿:

```markdown
## Summary
- (1~3 bullet)

## Plan / handoff mapping
- Plan: `docs/plans/windowsDependencyBootstrapPlan_2026-04-29.md`
- Handoff: `docs/prompts/windowsDependencyBootstrapDeveloperHandoff_2026-04-29.md`
- Task: T_

## Invariants
- INV-1: (macOS 영향 평가)
- INV-4: 단일 axis (axis 외 변경 없음)

## Verification (Windows host)
- cargo test --lib: __ passed
- vitest run: __ passed
- tsc --noEmit: clean
- (수동 smoke: ...)

## Test plan
- [ ] macOS CI ✓
- [ ] Windows CI ✓
```

---

## 6. 보고 포맷 (각 task 완료 시 chat)

- **Task ID** + 변경 라인 수 + 핵심 파일
- **검증 결과**: cargo test, vitest, tsc PASS/FAIL + 핵심 출력 1~2 줄
- **baseline 카운트**: FE/Rust 비교 (감소 시 즉시 보고)
- **PR URL** + CI 양쪽 ✓ 확인 + 머지 commit hash
- **INV 위반 없음 확인**: macOS-specific 파일 미변경 git diff 결과
- **다음 task 진행 여부** 또는 escalate 사유

---

## 7. 막히면 — escalate (chat 보고 + 사용자 판단 대기)

다음 상황에서는 **임의 fix 금지**. chat 으로 사용자에게 보고:

- macOS CI 회귀 (어떤 task 라도 macOS job 실패) → 즉시 PR 닫고 회귀만 좁혀 별 PR.
- chub.cmd 가 PATH 에 있는데도 backend 가 인식 못 함 → R-1 (Rust escape 회귀) 가설 진단 후 보고.
- pip install 시 권한/네트워크 에러 → 사용자에게 venv 사용 여부 확인 (Q-3 미해결이면).
- consent dialog UX 가 §B startup race 진단을 방해할 가능성 (Q-4) → 사용자 판단.
- Plan §8 의 **오픈 질문 Q-1~4** 가 답 없는 채로 task 가 막힐 때 — 본 plan 세션은 architect 가 작성했으므로 Q 답을 받기 전에 그 task 보류.

---

## 8. 도구 활용 / 안전 규칙 요약 (CLAUDE.md §15-§17 발췌)

- **검색**: `fd`, `rg`, `rawq search` (시맨틱). `grep` 대신 `rg`. 멀티 파일 치환 `fd ... | xargs sd ...` — Read+Edit 루프 금지.
- **그래프 영향 분석**: `code-review-graph detect-changes` — 본 task 후 dependency 가 정상 인식되면 사용 가능.
- **UI 진입점 변경 전**: 대체 경로 작동 확인 (2026-03-29 RT 사고 회피).
- **5-engine parity**: 모든 UI-연결 엔진(claude/codex/gemini/ollama/lmstudio)이 `build_normalized_prompt_with_budget()` 단일 경로. 본 task 는 그 hot-path 미터치.
- **finalize_engine_run**: mutex re-entrant 위험 자리. 본 task 와 무관.

---

## 9. 오늘 작업 종료 시 정리 (architect → developer 인계 후 세션 끝낼 때)

- 머지된 PR 목록 + 머지 commit hash
- 남은 task (T_ 까지 완료, T_+1 부터 미수행) — 다음 세션의 entry point 명시
- baseline 카운트 비교 (시작 vs 종료, 회귀 0 확인)
- `windowsDependencyBootstrapPlan_2026-04-29.md` §4 Phase status 갱신 (P0 완료 / P1 진행 중 등)
- chat 으로 architect (사용자) 에게 day-end 보고:
  - PR ___ ~ ___ 머지 완료
  - 검증 통과 / 회귀 0
  - 잔여 task 와 권장 다음 세션 entry

---

## 10. 빠른 명령 모음 (cheat sheet)

```powershell
# Status / sync
git status; git fetch origin; git pull origin main

# Build / test
cd src-tauri; cargo check; cargo test --lib; cd ..
npx tsc --noEmit
npx vitest run

# Dev mode
npm run tauri dev

# Release smoke (T1~T2 머지 후)
npm run tauri build

# 의존성 재확인
where chub
where code-review-graph
chub --cli-version
code-review-graph --version
```
