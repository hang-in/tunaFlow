# Changelog

All notable changes to tunaFlow are recorded here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [SemVer](https://semver.org/spec/v2.0.0.html).

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

[0.1.2-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.1-beta...v0.1.2-beta
[0.1.1-beta]: https://github.com/hang-in/tunaFlow/compare/v0.1.0-beta...v0.1.1-beta
[0.1.0-beta]: https://github.com/hang-in/tunaFlow/releases/tag/v0.1.0-beta
