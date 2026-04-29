---
title: Windows 의존성 부트스트랩 플랜 — context-hub / code-review-graph 인식 + 배포 패키지 포함
created_at: 2026-04-29
calling_role: architect (Windows 머신)
target_role: developer → reviewer (Codex, optional)
related_plans:
  - docs/plans/windowsBetaHardeningPlan_2026-04-26.md
  - docs/plans/windowsBuildPlan_2026-04-24.md
  - docs/plans/cicdReleasePlan.md
status: draft
---

# Windows 의존성 부트스트랩 플랜

## 0. 요약 (1 단락)

`context-hub` (`@aisuite/chub` npm) 와 `code-review-graph` (pip) 두 사이드카가
mac 환경에는 미리 설치돼 있어 Settings → Runtime 에 `ready` 로 표기되지만,
Windows 환경(이 머신 포함)에는 미설치 상태라 두 항목 모두 `unavailable` 로 보인다.
README 의 *"Auto-installed on first run"* 표기와 달리 backend 코드에는 자동 설치 로직이
**전혀 없다** (`auto_install`/`install_if_missing`/`npm install -g`/`pip install` grep 0건).
즉 mac 측 정상 동작은 *사용자 / architect 가 수동 설치한 결과* 이며 Windows 측에서는
그 manual setup 이 누락된 채 silent unavailable 로 빠진다. 이 플랜은
(1) 두 의존성을 Windows 측에서도 정상 인식되게 하고,
(2) 배포 패키지(NSIS installer) 첫 실행 단계에서 *user consent 후* 설치를 시도하며,
(3) 호환성 문제 시 안내 → 재시도 가능한 회복 경로를 제공한다.

## 1. Invariants

| ID | 내용 |
|---|---|
| **INV-1** 🔴 | **macOS tunaFlow 에 사이드 이펙트 0**. Windows 측 변경은 `#[cfg(target_os = "windows")]` 격리, macOS 무관 새 파일 추가, 또는 macOS CI 빌드 통과 검증 후에만. |
| **INV-2** | **PR + CI watch 필수**. macOS + Windows 양쪽 CI ✓ 후 머지. `gh pr merge --admin` 금지. |
| **INV-3** | macOS-specific 경로/스크립트(`bootstrap/env.rs` macOS PATH 보강, `scripts/build-rawq.sh` 등) 변경 X. |
| **INV-4** | **단일 axis per commit**. T1~T7 각 task 마다 별 commit + 별 PR. |
| **INV-DEP-A** (신규) | **자동 설치는 user consent 후에만**. silent global install 금지. 첫 실행 시 다이얼로그 → 사용자가 "설치" 선택 시에만 진행. |
| **INV-DEP-B** (신규) | **설치 실패 시 graceful degradation**. 의존성 미설치는 unavailable 상태로 두고 앱 진입 차단 금지. context-hub 미설치 = 검색 비활성, code-review-graph 미설치 = CRG 섹션 skip. 다른 기능 정상 동작. |
| **INV-DEP-C** (신규) | **README 표기와 실제 동작 일치**. "Auto-installed on first run" 이 silent install 을 의미하지 않음을 명확히 (consent UX 포함하도록 README 수정 또는 실제 silent 자동 설치 + opt-out 토글). 둘 중 한 쪽으로 통일. |

## 2. 현황 매트릭스

### 2.1 의존성 인벤토리 (Windows 머신, 2026-04-29 기준)

| 의존성 | 설치 위치 (mac/win) | 자동? | UI 영향 | Status |
|---|---|---|---|---|
| Node.js + npm | (toolchain) | 사용자 사전 | base | ✅ 설치됨 |
| Python 3 + pip | (toolchain) | 사용자 사전 | base | ✅ 설치됨 |
| claude CLI | `~/.local/bin/claude.exe` | 사용자 수동 | engine 미인식 | ✅ 설치됨 |
| codex CLI | `%APPDATA%\npm\codex.cmd` | 사용자 수동 | engine 미인식 | ✅ 설치됨 |
| gemini CLI | `%APPDATA%\npm\gemini.cmd` | 사용자 수동 | engine 미인식 | ✅ 설치됨 |
| **rawq sidecar** | `src-tauri/binaries/rawq-*.exe` | 빌드 시 (`scripts/build-rawq.{sh,ps1}`) | rawq footer | ✅ 빌드 (PR 후 NSIS 번들) |
| rawq daemon spawn | `bootstrap/services.rs` | 자동 (앱 시작 시) | — | ✅ 자동 |
| rawq snowflake 임베딩 | `%LOCALAPPDATA%\rawq\models\` | 자동 (rawq daemon 다운로드) | — | ✅ 자동 |
| **bge-m3 ONNX 모델** | `init_global_embedder_async` | 자동 (huggingface 다운로드) | secall RAG | ⚠ 첫 실행 시 ~2GB |
| **vendor skills** | `~/.tunaflow/skills/` | `scripts/publish-skills.sh` 수동 | Settings → Skills | ⚠ 수동 publish 필요 |
| **chub** (`@aisuite/chub`) | `%APPDATA%\npm\chub.cmd` | **없음** (코드 grep 0건) | context-hub 카드 | 🟢 본 PR 로 설치 + 부트스트랩 |
| **code-review-graph** | `<python>/Scripts/code-review-graph.exe` | **없음** | ContextPack CRG 섹션 | 🟢 본 PR 로 설치 + 부트스트랩 |
| ollama / lmstudio | (옵션, 사용자 사전) | — | 옵션 엔진 | (본 PR 범위 외) |

### 2.2 백엔드 detection 코드 검토

- **`src-tauri/src/agents/context_hub.rs:resolve_bin()`** — `HOME` env var 기반 후보 + `/usr/local/bin` 등 unix 절대 경로 + 마지막 PATH fallback (`Command::new("chub").arg("--help")`).
  - Windows native process 는 `HOME` 이 보통 미설정 (대신 `USERPROFILE`). 첫 1단계 candidate Vec 이 통째로 skip.
  - PATH fallback 은 정상 동작 — Windows 에서 `Command::new("chub")` 가 `chub.cmd` shim 을 발견. 그러나 Rust 1.77+ 의 `BatBadBat` (CVE-2024-24576) 보안 패치 이후 인자 sanitization 이 추가됨 — `--help` 정도의 단순 호출은 무관, 그러나 `chub search "query with spaces"` 같은 실 호출에서 escape 회귀 위험 있음. 별도 회귀 점검 필요.
- **`src-tauri/src/agents/crg.rs:resolve_bin()`** — `HOME/.local/bin/code-review-graph`, `/opt/homebrew/...` 등 unix 경로만. Windows PATH fallback 유무 미확인 (T2 에서 점검).

### 2.3 README 와 실제 동작 불일치

- README.{md,ko.md} : *"context-hub …  Auto-installed on first run"*
- 실제 코드 : detect → silent unavailable. 사용자 안내 없음.
- INSTALL.md §128 표 : "앱 내 안내 표시" — 안내 UI 도 미구현.
- 결론: 표기·문서·코드 셋 다 정합성 깨짐. **INV-DEP-C** 로 통일 방향 결정 필요.

## 3. 설계 선택 — bundle vs first-run vs 안내만

각 의존성을 다음 중 하나로 분류:

| 모드 | 의미 | 적합 의존성 |
|---|---|---|
| **A. bundle** | NSIS installer 자체에 포함, 별도 설치 불필요 | rawq sidecar (이미 적용), vendor skills (작음), 향후 chub binary 형태 가능 시 |
| **B. first-run with consent** | 앱 첫 실행 시 다이얼로그 → 사용자 동의 후 백그라운드 install | **chub** (npm i -g), **code-review-graph** (pip install) |
| **C. 안내만** | 미설치 감지 → UI 안내 + 사용자 가이드 링크. 자동 설치 없음 | claude/codex/gemini/ollama 같은 *사용자 정체성에 묶이는* CLI |
| **D. 자동 다운로드** | 앱이 직접 fetch (코드 내장) | bge-m3 모델 (이미 적용) |

**권장 분류**:

| 의존성 | 권장 모드 | 이유 |
|---|---|---|
| chub | **B** (first-run consent) | npm 글로벌 — Node 환경 의존, 글로벌 install 은 사용자 환경에 영향이라 silent 금지 |
| code-review-graph | **B** (first-run consent) | pip 글로벌 — 큰 의존성 트리 (tree-sitter 등), Python venv 우선 권유 안내 포함 |
| vendor skills | **A** (bundle) | 작음, 정적, idempotent. NSIS 설치 시 `%USERPROFILE%\.tunaflow\skills\` 에 풀어두면 됨 |
| bge-m3 | **D** (현행 유지) | 2GB → bundle 비현실. 첫 indexing/search 시 progress UI |

## 4. Phase 분해 (P0 → P3)

### Phase 1 — Backend 인식 정상화 (P0, 본 PR 의 즉시 차단 사유 해소)

#### T1 — `context_hub::resolve_bin()` Windows 호환 검증 + 보강
- **파일**: `src-tauri/src/agents/context_hub.rs`
- **현황**: PATH fallback 으로 chub.cmd 인식 가능 *should be*. dev 빌드에서 실제 동작 확인.
- **변경**:
  - `Windows` cfg 분기 추가: `USERPROFILE\\AppData\\Roaming\\npm\\chub.cmd` 후보를 candidates 에 push (PATH fallback 보다 명시적이라 빠름 + 안정).
  - `HOME` 분기는 그대로 유지 (Linux/macOS 호환).
- **테스트**: 단위 테스트 — Windows env 모의(`std::env::set_var("USERPROFILE", ...)` + temp dir) 에서 candidate path 가 결과에 포함되는지.
- **INV**: cfg 격리, macOS 영향 0.

#### T2 — `crg::resolve_bin()` Windows 호환 추가
- **파일**: `src-tauri/src/agents/crg.rs`
- **현황**: unix 경로 candidate + which 호출. Windows PATH fallback 부재 가능성.
- **변경**:
  - cfg(windows) 분기: `<python>/Scripts/code-review-graph.exe` 후보 (USERPROFILE 또는 sys.executable 추론). 또는 PATH fallback 으로 `Command::new("code-review-graph").arg("--version")` 추가.
  - 가장 단순한 안: PATH fallback (`Command::new("code-review-graph")`). cfg 분기 없이 cross-platform.
- **테스트**: T1 과 동일 패턴.
- **INV**: PATH fallback 이면 macOS 도 동일 코드 실행 — but 이미 unix candidate 가 먼저 hit 하므로 동작 변경 없음.

#### T3 — README/INSTALL.md 의 자동 설치 문구 정정
- **파일**: `README.md`, `README.ko.md`, `INSTALL.md`
- **변경**: *"Auto-installed on first run"* → *"prompted to install on first run"* (consent UX 명시) 또는 silent install 을 채택할 경우 그대로 유지하고 코드를 맞추는 방향. INV-DEP-C 결정 후 한 쪽.
- **INV**: docs only, 코드 회귀 0.

### Phase 2 — First-run consent UI + auto-install (P1)

#### T4 — installer 후보 detection + dialog
- **파일**: `src-tauri/src/commands/dependency_install.rs` (신규), `src/components/tunaflow/FirstRunDependencyDialog.tsx` (신규)
- **로직**:
  1. 앱 시작 시 `setting("first_run_dependency_check_done")` 플래그 검사. 미수행이면 다이얼로그 표시.
  2. 검사 항목: chub, code-review-graph. 각각 `available: bool, installer_command: String, requires: String` 반환.
  3. 다이얼로그: 항목별 체크박스 (기본 ON) + "건너뛰기" / "설치". 선택 시 `install_dependency(name)` invoke.
  4. install 명령:
     - chub: `npm install -g @aisuite/chub` (`Command::new("npm")`)
     - code-review-graph: `pip install code-review-graph` (`Command::new("pip")`)
  5. 결과 status 이벤트 `dependency:install_result` emit. 실패 시 안내 + 수동 설치 명령 표시.
- **INV-DEP-A** 충족: user consent 후에만 실행.
- **INV-DEP-B** 충족: 다이얼로그 닫아도 앱 진행 가능.

#### T5 — Settings → Runtime 에 *수동 설치 트리거* 버튼 추가
- **파일**: `src/components/tunaflow/settings/RuntimeSection.tsx` 의 `ContextHubPanel`, 그리고 CRG 섹션이 있다면 그곳에 동일 버튼.
- **변경**: `unavailable` 상태일 때 "Install via npm/pip" 버튼 표시 → T4 의 `install_dependency` invoke. 이미 설치된 사용자에겐 안 보임.
- **INV**: macOS UI 도 동일하게 보이지만 macOS 에선 이미 설치된 경우가 보통이라 버튼 자체가 숨겨짐 → 영향 0.

### Phase 3 — Bundled assets (P2)

#### T6 — vendor skills 를 NSIS installer 에 번들
- **파일**: `src-tauri/tauri.conf.json` (resources 항목), `src-tauri/src/bootstrap/services.rs` (first-run 시 unpack)
- **변경**:
  - build 시 `_research/_skills` 또는 `agents/_skills` 의 sn snapshot 을 installer resources 에 포함.
  - 첫 실행 시 `~/.tunaflow/skills/` 가 비어 있으면 번들된 snapshot 을 unpack. 이후 사용자가 publish-skills 로 갱신 가능.
- **INV**: macOS 빌드도 같은 resources 항목 사용 가능 (cross-platform). 다만 macOS 측 publish-skills.sh 는 그대로 유지 — 두 경로 공존.
- **사이즈 영향**: 238 skills × 평균 SKILL.md ~5~50KB = 약 5~15MB. 허용 가능.

#### T7 — Windows installer 후 reboot/relaunch 가이드
- **파일**: NSIS .nsi (또는 `tauri.conf.json` bundle 설정)
- **변경**: post-install 단계에서 PATH 갱신 안내 (npm/pip 으로 새로 설치된 binary 가 같은 세션에서 인식 안 될 수 있어 dev 모드 재시작 필요). 다이얼로그 또는 README 보강.

### Phase 4 — 추후 개선 (P3, 본 plan 외 axis)

- chub 정적 binary 번들 (npm 의존 제거) — chub 가 단일 binary release 를 제공하면 mode A 로 격상 가능. 현재 npm-only 라 Phase 2 의 mode B 유지.
- code-review-graph 의 PyInstaller 단일 실행 파일 번들 — Python 의존 제거 가능. 단 사이즈 크고 Python ABI 호환성 위험. 본 plan 범위 외.

## 5. 작업 분해 — developer 인계용

| Task | 파일 | 검증 명령 | 예상 LOC |
|---|---|---|---|
| **T1** | `src-tauri/src/agents/context_hub.rs` (+test) | `cargo test --lib agents::context_hub` | +30 / -0 |
| **T2** | `src-tauri/src/agents/crg.rs` (+test) | `cargo test --lib agents::crg` | +20 / -0 |
| **T3** | `README.md`, `README.ko.md`, `INSTALL.md` | docs only | +5 / -3 |
| **T4** | `src-tauri/src/commands/dependency_install.rs` (신규), `src/components/.../FirstRunDependencyDialog.tsx` (신규) | `cargo test`, `vitest run` | +150 / -0 |
| **T5** | `src/components/tunaflow/settings/RuntimeSection.tsx` | `vitest run` | +40 / -0 |
| **T6** | `src-tauri/tauri.conf.json`, `src-tauri/src/bootstrap/services.rs` | install + first run | +30 / -0 |
| **T7** | NSIS .nsi or tauri bundle config | install smoke | +10 / -0 |

각 Task → **별 commit + 별 PR + macOS+Windows CI ✓ 후 머지** (INV-2/4).

## 6. 회귀 가드 / 검증 시나리오

### 6.1 macOS 회귀 가드 (INV-1)
- T1/T2: macOS 환경에서 `cargo test --lib agents::{context_hub,crg}` baseline 카운트 동일.
- T3: docs only, code 무관.
- T4/T5: macOS 에서도 dialog/button 코드 컴파일/렌더 OK. 단 macOS 사용자에겐 *이미 설치돼 있음* 으로 invisible (UX 영향 0).
- T6: macOS 빌드 시 resources 포함 여부는 conf 설정에 따름. macOS 측 publish-skills.sh 동작 동일.

### 6.2 Windows 검증 (T1~T7 누적 후)
| ID | 시나리오 | 기대 결과 |
|---|---|---|
| W-1 | clean Windows VM 에 chub/crg 미설치 상태로 NSIS installer 설치 | 첫 실행 시 dialog → "설치" 선택 → chub + crg 글로벌 설치 → ready |
| W-2 | 같은 VM 에 chub 만 미리 설치된 상태로 dialog | crg 만 표시 (chub 항목 자동 hide) |
| W-3 | dialog "건너뛰기" → 앱 정상 진입, Settings → Runtime 의 두 카드 unavailable, 수동 설치 버튼 노출 |
| W-4 | npm install -g 권한 부족 (Roaming 쓰기 거부) → 실패 메시지 + 수동 명령 표시 |
| W-5 | 인터넷 차단 → npm/pip timeout 후 graceful 실패 메시지 |
| W-6 | 두 의존성 설치 후 dev 모드 재시작 → backend resolve_bin 즉시 인식, status `ready` |

### 6.3 회귀 카운트 baseline
- 본 plan 시작 시점: FE 381 / Rust 558 (Windows). FE/Rust 양쪽 +N (테스트 추가) 만 허용, 감소 금지.

## 7. 리뷰어(Codex) review 포인트

- **R-1** chub.cmd 인자 escape 회귀 (Rust 1.77+ CVE-2024-24576 영향) — `chub search "복합 query"` 같은 실 호출이 Windows 에서 정상 작동하는지.
- **R-2** Python 환경 가정 — 사용자가 `python3` 가 아닌 `python` 으로만 PATH 에 있을 때 `pip install` 호출 분기.
- **R-3** consent dialog 의 i18n — ko/en 양쪽 문구.
- **R-4** Settings install 버튼이 macOS 에서도 *동일 코드*로 렌더되지만 detection 결과 `available:true` 라 hidden 인지 (INV-1 안전성).
- **R-5** `dependency:install_result` 이벤트가 background 작업이라 hang 가능성 — timeout (예: npm 60s, pip 120s) 적용 여부.
- **R-6** README 표기 변경 (T3) 이 ko/en 양쪽 동일 의미 유지.

## 8. 오픈 질문 (architect → 사용자 / mac architect)

| Q | 결정 필요한 사항 |
|---|---|
| Q-1 | INV-DEP-C 통일 방향 — 문구를 *consent UX* 로 정정할지, 또는 silent install + opt-out 토글로 README 와 일치시킬지. |
| Q-2 | T6 (skills bundle) 우선순위 — 현재 publish-skills.sh 수동 publish 로도 동작하므로 P3 로 미룰 수 있음. NSIS 사이즈 영향 vs 사용자 편의 trade. |
| Q-3 | code-review-graph 가 Python 패키지라 venv 사용 여부 권장 — global pip install 이 사용자 환경에 영향이라 OS-wide 사이드이펙트. dialog 에 *"venv 사용 권장"* 안내 포함 여부. |
| Q-4 | T4 의 dialog 가 핸드오프 §B (startup race) 진단을 방해하지 않는지 — 첫 실행 시 dialog 표시가 "엔진/모델 감지" 단계 hang 과 별 axis 임을 명시. |

## 9. 진행 메모 (architect → developer)

- 본 plan 작성 직전, Windows architect 가 **수동으로** `npm install -g @aisuite/chub` (chub 0.1.4) 와 `pip install code-review-graph` (crg 2.3.2) 를 설치 완료. 따라서 T1/T2 검증은 이 머신에서 **즉시** 가능.
- T1~T2 만 머지해도 본 머신의 unavailable 표시는 ready 로 변경됨 (재시작 후). T4~T5 는 다른 Windows 사용자를 위한 일반 사용자 가치.
- 핸드오프 `windowsBetaHardeningArchitectHandoff_2026-04-29.md` 의 트랙 §B (startup race) / §C (DB path stale) / §D (watchdog compat) 와 axis 분리 — 본 plan 의 PR 은 별도로 머지.
- 머지 순서 권장: **T1 → T2 → T3 → T4 → T5 → (T6 → T7)**. 각 PR 사이 baseline 회귀 카운트 확인.
