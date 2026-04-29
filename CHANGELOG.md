# Changelog

All notable changes to tunaFlow are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

## [0.1.5-beta] - 2026-04-29 (예정)

🛡️ **claude transport flip hardening** — v0.1.4-beta 의 transport flip
(`-p --resume`) 후 발견된 stale resume_token 부작용 차단. 외부 사용자
v0.1.4-beta 업그레이드 직후 첫 send 좌절 시나리오 해소.

### Fixed

- **stale resume_token 자동 회복**
  ([claudeTransportFlipHardeningPlan_2026-04-29.md](docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md))
  - 한동안 미사용 conversation 의 resume_token 이 (a) 과거 sdk-url 시점
    session id 이거나 (b) Anthropic 측 TTL 만료 → `--resume <id>` 시도가
    "out of extra usage" 형태로 거부되던 문제. 사용자 액션 0 자동 회복.
  - **T1**: claude.rs `stream_run` 이 `rate_limit_event` line 을 parse →
    `RunOutput.last_rate_limit` 로 노출 (RuntimeStatusBar indicator 데이터
    소스).
  - **T2**: `stream_run` wrapper 가 result.is_error 의 keyword 패턴 detect
    → `--resume` 제거 후 1회 retry. false positive 차단 (정상 인증 실패 /
    한도 초과 / 네트워크 에러는 retry 트리거 X).
  - **T3**: retry 성공 시 `session_freshness::clear_delivered_key` →
    다음 send 부터 `is_session_continuation=false` → ContextPack revival
    자동 (full mode + anchor 2 turns). frontend 에 `claude:fresh_fallback`
    이벤트 emit.
  - **T4**: 사용자 가시화 — fresh_fallback toast 1회 + RuntimeStatusBar 의
    Claude rate_limit indicator (정상 시 hide, approaching/limit_reached/
    overage_disabled 상태별 색상). claude.ai/settings/usage 링크.
  - **T5**: DB migration v49 — 7일+ idle conversation 의 stale claude
    resume_token 일괄 NULL. idempotent (schema_version 가드). 활성 사용
    conversation 영향 0.
  - **T6**: Conversation 우클릭 메뉴에 "Claude 세션 재시작" 항목 추가.
    backend `restart_sdk_session` 호출 + 토스트.
  - **T7**: claude API 에러 6 종 분류 (\`stale_resume_token\` /
    \`auth_failure\` / \`rate_limited\` / \`quota_exceeded\` /
    \`model_unavailable\` / \`unknown\`). agent:error event 의 errorKind
    payload 에 노출.

### Anthropic billing 안내

tunaFlow 가 `claude -p` headless mode 사용 — Pro/Max plan 의 5시간 rolling
한도 + overage 정책 동일 적용. 한동안 미사용 conversation 의 resume_token
이 stale 일 수 있음 — v0.1.5-beta 가 자동 fallback. 수동 재시작은
conversation 우클릭 메뉴 (T6). claude.ai/settings/usage 에서 한도 / overage
/ "extra usage" 옵션 확인 권장.

## [0.1.4-beta] - 2026-04-29

🚨 **긴급 패치** — claude CLI 2.1.121 (2026-04-28 자동 update) 의 `--sdk-url`
정책 변경으로 tunaFlow sdk-session 모드 영구 차단. 모든 사용자 환경에서 claude
응답이 30s timeout 으로 중단되는 회귀 발생. CLI `-p --resume` path 로 transport
전환 (Anthropic 공식 사용자 path).

### Fixed

- **Reviewer 정책 위반 차단** (PR #211 + 후속) — Codex Reviewer 가
  `*-result.md` 를 자체 read tool 로 직접 열람 후 잘림 패턴을 verdict 근거로
  사용하던 정책 위반 패턴 확인. ContextPack 입력 차단 (PR #211, root cause)
  에 더해 REVIEWER_TEMPLATE 에 "Never read `*-result.md`" 규칙 명시 추가
  (이 plan). reportSync 의 truncation 도 UTF-8 boundary-safe 8k/2k 상한 +
  잘림 마커 + sentinel 기반 self-include guard 로 강화.
- **claude agent watchdog trailing kill 차단** — reader loop 정상 종료 후
  watchdog 30s sleep 누적이 이미 reap 된 PID 에 `kill -9` 송출하던 race.
  PID 재사용 시 엉뚱한 프로세스 kill 위험 0 으로 차단. RAII guard 패턴.
- **claude transport 영구 차단 회귀** (claude CLI 2.1.121 정책 변경):
  - claude 2.1.121 가 `--sdk-url` 의 host 를 `api.anthropic.com` 등 5 도메인만
    허용하도록 hardcoded whitelist 도입. tunaFlow 의 localhost WS 서버 차단
    → 모든 send 가 30s timeout. 사용자 가시 메시지 "claude did not connect within 30s"
  - **fix**: dispatch default 를 `-p --session-id`/`--resume` path 로 flip.
    claude internal session store 가 history 보관, 매 send 마다 fresh spawn
    (~2.5s) + cache hit 으로 빠른 reload (cache_read_input_tokens ~36k 확인)
  - manual 검증: Step 1 `--session-id "remember 42"` → "OK" / Step 2
    `--resume "what number?"` → "42" (stateful conversation 정상)
  - SSOT: `docs/plans/claudeResumeSessionTransitionPlan_2026-04-29.md`

### Changed

- **`resolve_claude_mode` default flip** — `cli` (resume-session) 가 default,
  `sdk-url` 은 `TUNAFLOW_USE_SDK_URL=1` 환경변수 명시 시만 활성화 (Anthropic 정책
  우회 path 발견 시 즉시 재활성화 가능). 기존 `TUNAFLOW_DISABLE_SDK_URL` env 는
  의미 반대 변경됨 — 사용자 환경에 set 됐다면 unset 또는 새 변수로 마이그레이션.
- **`restart_sdk_session` 명령 의미 확장** — sdk-url path 는 기존대로 process
  kill + RESUME_IDS clear + DB clear, cli (resume-session) path 는 DB resume_token
  NULL 처리 (다음 send 가 신규 session 으로 시작). engine / model 변경 시 같은
  명령으로 통일.

### Notes

- **sdk-session 코드 유지** (`src-tauri/src/agents/claude_sdk_session.rs`) —
  Anthropic 정책 우회 path 발견 시 `TUNAFLOW_USE_SDK_URL=1` 으로 즉시 재활성화.
  본 release 에서 deprecate 만, 코드는 그대로.
- **검증된 우회 path 후보** (모두 production 부적합): `/etc/hosts` + self-signed
  TLS (system-wide 침범), binary patch (ToS 회색), desktop app 빈틈 (cloud 사용),
  PTY 회귀 (parsing 불안), Anthropic 공식 RC 등록 (가능성 낮음).

### Windows-specific changes

- **첫 실행 동의 dialog + Settings 수동 설치 버튼** (PR #227 / #229 — T4/T5)
  — `chub` (`@aisuite/chub`) 와 `code-review-graph` 가 Windows 미설치 상태로
  unavailable 표기되던 회귀 차단. 첫 실행 시 consent dialog 노출, 사용자
  동의 시 npm/pip 으로 글로벌 설치 (timeout npm 60s / pip 120s, 활성 venv
  자동 활용). dismiss 시 graceful fallback + Settings → Runtime 카드의
  "npm/pip 으로 설치" 버튼 노출. silent global install 금지 (INV-DEP-A).
  SSOT: `docs/plans/windowsDependencyBootstrapPlan_2026-04-29.md`.
- **`context_hub` / `crg` `resolve_bin` Windows path 인식** (PR #221 / #223
  — T1/T2) — `%APPDATA%\npm\chub.cmd` 와 `<python>\Scripts\code-review-graph.exe`
  를 Windows native process `Command::new` 가 정상 spawn 하도록 cfg 분기 +
  PATH fallback 보강.
- **Windows 타이틀바 통합** (PR #237 — T-WT-1/2/3) — `decorations: false` +
  자체 `WindowControls.tsx` (Min / Maximize-Restore / Close 사각 46×32) +
  좌측 정렬 통일. 기존 *3 라인 헤더* (native title bar + TitleBar.tsx +
  콘텐츠 헤더) → *1 라인 통합*. mac 도 같은 좌측 정렬 적용 (시각 회귀 0).
  SSOT: `docs/plans/windowsTitlebarUnificationPlan_2026-04-29.md`.
- **claude watchdog `taskkill` 분기** (PR #231 — §D) — `Command::new("kill")`
  은 Unix-only 라 Windows 에서 idle_timeout (600s) 시 child `claude.exe` 가
  zombie 잔존 위험. `cfg(unix)` 는 `kill -9`, `cfg(windows)` 는
  `taskkill /F /PID` 분기.
- **`kill_orphan_sdk_processes` Windows no-op stub** (PR #235) — Unix-only
  `pgrep`/`ps` 가 Windows 에서 silently no-op 였음을 explicit 화. 실제
  orphan 처리는 `windowsOrphanProcessHardeningPlan` (P3, post-beta) 후속.
- **conventions `@import` path separator 정규화** (PR #213) — `Path::display()`
  Windows backslash 출력으로 Claude Code `@path` syntax 깨지던 회귀 fix.
- **`commands/files` 테스트 path-separator 정규화** (PR #226 — R-W-7
  hotfix) — `flatten_md_paths` test helper 정규화. escalate-1~4 + 동일 패턴
  3건 일괄 처리, production 영향 0.
- **DB project path stale fallback** (PR #234 — Track 3) — mac 동기화 DB 의
  `projects.path` 가 Windows 에서 invalid 일 때 file IO timeout hang 차단.
  startup load 시점 validate + UI fallback, DB row 보존.
- **claude SDK 세션 stderr surface** (PR #233 — Track 2 진단 도구) —
  `Stdio::null()` → `Stdio::piped()` + `[sdk-session-stderr]` 라인 forward.
  PR #222 (codex stderr surface) 동등 패턴.

### Internal / housekeeping (Windows)

- **Rust warning silence** (PR #230) — `unused_imports` 1건 + `dead_code` 2건
  (test-only `InvokeClaude::Empty/Stub` + `NotificationAuthStatus` stub
  variants). 동작 변경 0.
- **Plan / handoff docs**: `windowsDependencyBootstrapPlan` (#214/#236),
  `windowsCiPipelinePlan` (#224, mac+win cross-OS regression detection
  정책), `windowsTitlebarUnificationPlan` (#228/#236), 그리고 status 갱신
  (#232 `complete`).

### Known issues (Windows)

- **첫 실행 후 첫 메시지 ~30초 지연** — Microsoft Defender 의 first-scan
  영향으로 추정 (정적 분석 결과 가설 (b) 가장 유력). **1분 후 다시 보내면
  정상 동작**. Track 2 진단 도구 (PR #233) 가 다음 cold start 시 backend
  stderr 에 root cause 를 노출 → fix axis 는 v0.1.5 정식 release 또는 캡처
  후 별 PR. SSOT: `docs/plans/windowsBetaHardeningPlan_2026-04-26.md` §B.
- **Windows 11 snap layouts overlay 미표시** — Maximize 버튼 hover 시 Win11
  22H2+ 의 snap layouts 가 안 뜸. `decorations: false` 와 잠재 충돌 진단 후
  v0.1.5 에서 fix (T-WT-5 / Q-WT-3). SSOT: `windowsTitlebarUnificationPlan`.

## [0.1.3-beta] - 2026-04-26

Beta 사용자 보고 follow-up. 첫 외부 사용자 환경에서 두 건 보고 — rawq sidecar 가
앱 번들에서 영구 미인식 (Tauri 가 sidecar 번들 시 triple suffix strip 하는데
코드는 `rawq-{triple}` 이름만 검색) + 채팅/로그 single newline 이 한 줄로
collapse. 둘 다 v0.1.0~v0.1.2 사용자 모두 영향이라 hotfix.

### Fixed

- **rawq sidecar resolution** (#210) — `sidecar_strip_name()` + `resolve_diagnostics()`
  추가 (`src-tauri/src/agents/rawq.rs`). Tauri 가 번들 시 triple suffix 를 strip
  해서 `Contents/MacOS/rawq` 로 들어가는데 코드는 `rawq-aarch64-apple-darwin`
  으로만 검색하던 영구 mismatch. v0.1.0-beta 부터 모든 macOS 사용자에게 영향.
  drag-install 시 quarantine (`xattr`) 부착으로 sidecar 가 SIGKILL 되는 케이스도
  같이 정리. CI 의 `build-tauri-lite` 에 staged + built bundle 양쪽 verify step
  추가로 회귀 차단.
- **`get_rawq_status` unavailable 메시지** — 다음 단계 액션 (`xattr -cr` 후
  재시도, README 링크) 포함하도록 명료화.

### Added

- **`remark-breaks` 마크다운 플러그인** (#209) — 채팅/로그 paste 시 single
  newline 이 visible line break 으로 표시됨. CommonMark spec 상 paragraph 안
  single `\n` 은 공백으로 collapse 되는 게 정상이지만, 채팅·로그 컨텍스트엔
  부적합. `src/lib/markdownPlugins.ts` SSOT 모듈 신규 + 11 사용처 통일 +
  회귀 테스트 13건 (single newline → `<br>` / paragraph break / list / code
  block / table / strikethrough 보존).
- **INSTALL.md drag-install 안내** — `xattr -cr /Applications/tunaFlow.app`
  필요성 + 문제 해결 표 + smoke checklist 4 단계.

### Changed

- **README / README.ko Known Constraints** — "rawq is a bundled sidecar"
  명시 + drag-install quarantine 영향 보강. 시스템 PATH 의 `rawq` 는 영향 없음.

### Notes

- `docs/reference/rawqSidecarReleaseAudit_2026-04-26.md` — Layer A1 audit 결과
  (DMG mount + `xattr` + `file` 출력 인용). 진단 분기 근거 SSOT.
- 이번 fix 머지 + 신 release 까지 필요. 기존 v0.1.x 사용자가 `xattr -cr` 만
  실행해도 코드측 mismatch 가 별도라 rawq 인식 안 됨.

## [0.1.2-beta] - 2026-04-26

Windows build support + fragility audit hardening. First Windows release
(NSIS installer for x64). Followup audit on yesterday's UTF-8 panic cascade
yields atomic-transaction wraps for `delete_branch` / `update_plan_status` /
`delete_conversation`, plus production-path panic / unwrap audit confirming
zero remaining fragility in the same category.

### Added

- **Windows x64 build** via NSIS installer (`tunaFlow_*_x64-setup.exe`).
  CI matrix extended to `windows-latest` for `rawq` sidecar + Tauri Lite
  bundle. Same `v*.*.*` Release as macOS — single asset listing per release.
  Plan: `docs/plans/windowsBuildPlan_2026-04-24.md`.
- **`basename(path, fallback)` utility** (`src/lib/utils.ts`) — supports both
  `/` (Unix) and `\` (Windows) separators. Replaces 5 hardcoded
  `path.split("/").pop()` sites.
- **`scripts/build-rawq.ps1`** — PowerShell mirror of `build-rawq.sh` for
  Windows local sidecar builds.
- **`NoConsole` trait** (`src-tauri/src/no_console.rs`) — Windows
  `CREATE_NO_WINDOW` flag applied to all subprocess spawns. Stops the cmd
  window flicker that was happening on every CLI agent / git / model
  discovery call (50 spawn sites across 17 files patched).
- **Splash UI on app init** (`AppShell.tsx`) — spinner + stepwise loading
  text ("환경 설정 로드 중..." / "프로젝트 목록 로드 중..." / "엔진 / 모델
  감지 중..." / "프로젝트 열기: {name}..."). Replaces the blank sidebar-color
  box that left users wondering if the app had hung. `setLoaded(true)` moved
  to `finally` so `selectProject` failure no longer traps users on the splash.

### Changed

- **`bundle.targets`** narrowed from `"all"` to explicit list `["app", "dmg",
  "appimage", "deb", "rpm", "nsis"]` — MSI excluded. MSI rejects prerelease
  identifiers (`-beta`); NSIS has no such restriction. Beta-window decision;
  may revisit MSI when `-beta` is dropped.
- **`bundle.macOS.signingIdentity = "-"`** moved from CI `--config` override
  to permanent `tauri.conf.json` setting. Windows shell-escape of multiline
  `--config '{...}'` JSON kept breaking; permanent config sidesteps it.
- **CI workflow_dispatch behavior** — version falls back to `package.json`
  default (smoke-test mode), `tagName=''` so no draft release is generated.
  Tag-push path unchanged — release flow identical to v0.1.1-beta.
- **Tauri icons regenerated** via `npx tauri icon` — old `icon.ico` was
  actually a PNG with `.ico` extension, which Windows `RC.EXE` rejected. New
  ICO is proper multi-resolution Windows icon resource.
- **`INSTALL.md`** — Windows installation section + Gatekeeper / SmartScreen /
  antivirus guidance split into 3 axes. VirusTotal verification note added.
  Release body in `build.yml` mirrors the same 3-axis structure.

### Fixed

- **UTF-8 char boundary panic** (`identity_analyzer.rs:96`) — `i + 1` byte
  index split a multi-byte CJK character (`'지'` mid-bytes) → panic →
  `Lock poisoned` cascade across `bg-worker` / vector indexing until app
  restart. Replaced with `i + c.len_utf8()` and proper char-count tracking.
  Same fix applied to `project_onboarding.rs:203` (`&content[..3000]`).
- **`delete_branch`** (`branches.rs:387`) — 8 sequential DELETE/UPDATE
  statements wrapped in a single transaction. Mid-statement failure (FK
  constraint, lock contention) no longer leaves partial state with child
  branches deleted but parent intact.
- **`update_plan_status`** (`plans.rs:319`) — status / phase / branch-archive
  3 statements wrapped in a transaction. Removes the "status='done' but
  phase='active' stuck" partial-commit window.
- **`delete_conversation`** (`conversations.rs:127`) — 4 + N×5 + 1 statements
  (including shadow-branch conversations) wrapped in a transaction.

### Removed

- **MSI bundle target** (Windows) — see Changed.

### Notes

- Production unwrap / expect / panic / unreachable / todo / unimplemented
  audit: zero remaining in non-test paths after this release.
- `failure_lessons.rs:63 create_failure_lessons_batch` loop multi-execute
  is intentional partial-commit (failed lesson skipped, others kept) —
  out of scope.

## [0.1.1-beta] - 2026-04-25

First post-launch maintenance release. Triages public-beta community reports
(#175 / #176 / #178 / #180), recovers brand-session intent that drifted during
the s36 PTY → sdk-url WS transition, and lands a stack of plan-driven fixes for
multi-Developer collisions, brand cancel semantics, and layout cascading bugs.

### Added

- **Custom endpoint config UI for Ollama / LM Studio** (#175) — base URL override
  per engine, no more rebuild-to-switch.
- **Manual verification gate (B-19)** between impl-complete and review (#176) —
  optional fail-reason field with placeholder fallback.
- **rawq cancel channel** for in-flight index builds (#197 / audit #5).
- **`rebuild_rawq_index` command + Settings UI button** for stale-index recovery.
- **User intent SSOT surfacing** — Architect ContextPack now anchors on conversation
  intent extracted from raw turns (#199).
- **Brand inherits main CLI session** — `session_key_for(conv_id)` normalizes
  `branch:*` → root conversation; brand sends skip ContextPack to reuse main
  session continuity (#198).
- **Multi-Developer active-plan isolation** — brand-aware plan slot + ContextPack
  sender Developer ID (#204).
- **`flexboxConventions.md` SSOT** — `flex-col + flex-1` requires `min-h-0` on
  every parent; documented after #191 / #201 cascade chain.
- **CHANGELOG.md** — this file.

### Changed

- **CI self-trust trigger** — main-push trigger removed; only external PRs and
  release tags (`v*.*.*`) run CI. Cuts cognitive context fragmentation for solo
  dev. See `docs/plans/selfTrustCiTriggerOptimizationPlan_2026-04-25.md`.
- **install.sh** — fallback to `sudo` when `/usr/local/bin` is root-owned;
  `/releases` (not `/releases/latest`) for prerelease tag support; DMG matched by
  arch tag (`aarch64` / `x64`) instead of Rust triple.
- **Cargo / npm manifest metadata** — license / author / repository / description
  populated on both crates and root package.
- **README** — embed 6-minute demo video via GitHub user-attachments CDN; sync
  README.ko with English; correct 4-engine → 5-engine parity; refresh stale
  DB/test counters.
- **Cancel semantics on brand** — stream-abort token only; `restart_sdk_session`
  remains the explicit session-kill path (#202).

### Fixed

- **#178** — Claude `--dangerously-skip-permissions` flag added at all 3 call
  sites (`claude.rs:162`, `claude.rs:380`, `claude_sdk_session.rs:381`); fixes
  infinite hang on fs permission prompts.
- **#180** — rawq excludes build-artifact dirs (`target/**`, `node_modules/**`,
  `.venv/**`, `dist/**`, `build/**`, 14 patterns total) to prevent OOM.
- **#191** — `min-h-0` on main flex parent so long drawer content cannot stretch
  the viewport.
- **#201** — `min-h-0` cascade fix for ChatPanel plan→dev phase footer drift
  (3 nested flex children).
- **#188** — tool-steps finalize running status on stream completion; non-streaming
  UI fallback path.
- **#190** — onboarding Skip cancels the Rust analysis task instead of leaking;
  unified error-state buttons.
- **#193** — `startReviewRT` entry failure rollback + retry UX.
- **#194** — Codex / Gemini meta-agent analysis no longer biased to Claude's
  output format; `parse_output` accepts engine-native shapes.
- **#195** — plan generation atomic DB transaction with file-write rollback.
- **#196** — branch adopt wraps DB writes in a single transaction.
- **#186** — DB v47 migration: `agent_jobs.conversation_id` nullable for
  detached jobs.
- **C-2 / B-16** — tunaflow marker scrubbing consolidated across result / insight
  export paths.
- **brand cancel** — was no-op (or worse, killed main session) post-PR #198;
  now stream-abort only, session preserved (#202).

### Removed

- Stale `.tunaflow/outbox/*.md` artifacts from the polling-deprecated era
  (post-9295062 cleanup) + `.tunaflow/outbox/` added to `.gitignore` (#200).
- Unused experimental README ack entries (DINKIssTyle-Markdown-Browser).

### Docs

- `docs/reference/branchCancelAudit_2026-04-25.md` — audit feeding #202.
- `docs/reference/flexboxAuditResult_2026-04-25.md` — repo-wide `flex-1` survey.
- `docs/reference/multiDeveloperIsolationDecision_2026-04-25.md` — A+B option
  rationale.
- `docs/plans/selfTrustCiTriggerOptimizationPlan_2026-04-25.md` — CI trigger
  policy SSOT.
- `docs/plans/branchInheritsMainSessionPlan_2026-04-25.md` — Task A intent
  recovery + 4-layer fix.
- 7 additional plans in `docs/plans/` (today's user reports + sibling work).

## [0.1.0-beta] - 2026-04-23

Public beta launch. See README and `docs/reference/sessionHistory.md` for the
full backstory; this entry only marks the cut.

[0.1.4-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.3-beta...v0.1.4-beta
[0.1.3-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.2-beta...v0.1.3-beta
[0.1.2-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.1-beta...v0.1.2-beta
[0.1.1-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.0-beta...v0.1.1-beta
[0.1.0-beta]: https://github.com/hang-in/tunaFlow/releases/tag/v0.1.0-beta
