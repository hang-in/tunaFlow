# Changelog

All notable changes to tunaFlow are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

## [0.1.4-beta] - 2026-04-29

🚨 **긴급 패치** — claude CLI 2.1.121 (2026-04-28 자동 update) 의 `--sdk-url`
정책 변경으로 tunaFlow sdk-session 모드 영구 차단. 모든 사용자 환경에서 claude
응답이 30s timeout 으로 중단되는 회귀 발생. CLI `-p --resume` path 로 transport
전환 (Anthropic 공식 사용자 path).

### Fixed

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
