---
title: onboarding codex 호출 stderr 표면화 — exit 1 root cause 즉시 진단 가능
status: ready-to-implement
priority: P1 (Plan B Task 02 진단 follow-up — observability 회복, fix 가설 검증 전제)
created_at: 2026-04-29
related:
  - src-tauri/src/commands/project_onboarding.rs   # 577~601 라인, codex spawn 영역
  - src-tauri/src/agents/codex.rs                  # 55~130, production codex 호출 (참조 패턴)
  - docs/plans/onboardingAnalysisFailureAndSkipUiPlan_2026-04-29.md  # Plan B Task 02 진단 SSOT
canonical: true
owners:
  - architect (본 plan 작성)
  - developer (구현)
---

# 배경 (Plan B Task 02 진단 인용, 2026-04-29)

batmania52 보고: onboarding 시 codex 가 exit 1 로 실패. 사용자/로그 어디에도 root cause 가 표면화되지 않음.

진단 결과 (Plan B Task 02):

| 항목 | `agents/codex.rs` (production) | `commands/project_onboarding.rs:576~601` (onboarding) |
|---|---|---|
| 인자 | `exec --json --skip-git-repo-check --color=never --full-auto [--model X] -i ... -` | `exec --full-auto - [--model X]` |
| **stderr** | `Stdio::piped()` + drain → 에러 메시지에 본문 포함 | **`Stdio::null()` (583줄) — 전부 버림** |
| current_dir | `resolve_cwd(input.project_path)` 설정 | **미설정** |

가설 H1~H4 (skip-git-repo-check 누락, cwd 미설정, 인자 순서, codex 신버전) 가 있지만 **root cause 식별 자체가 stderr 가 버려져서 불가능**. F1 (stderr piped) 단독으로 들어가야 다음 사용자 보고 시 가설 즉시 확정 + F2~F5 (가설 기반 fix) 의 검증 근거 확보.

# 진단 (Architect 사전 분석)

- F1 = `Stdio::null()` → `Stdio::piped()` + stderr drain (await_cli_with_cancel 의 에러 메시지에 본문 포함)
- 변경 라인 수 ~10 LoC. 위험 0 (출력 캡처만, 동작 변경 없음).
- F2~F5 (skip-git-repo-check, color=never, current_dir, 인자 순서) 는 **본 plan 비대상** — 가설 검증 후 별 PR 분리.

# Fix Scope

## Layer A — F1 stderr piped + drain

### A1. `commands/project_onboarding.rs:577~601` 수정
- `cmd.stderr(Stdio::null())` → `cmd.stderr(Stdio::piped())` 변경
- spawn 후 stderr handle 을 drain (background read or `wait_with_output()` 패턴)
- `await_cli_with_cancel` 또는 동등 wrapper 의 에러 메시지에 stderr 본문 포함 (`agents/codex.rs:55~130` 의 production drain 패턴 참조)
- exit code != 0 시: `Err(format!("codex exit {code}: {stderr_body}"))` 형태
- stderr 가 비어 있으면: `Err(format!("codex exit {code} (no stderr)"))` 로 fallback

### A2. 기존 production 패턴과 일치 확인
- `agents/codex.rs` 의 stderr drain helper 가 재사용 가능하면 그대로 사용 (DRY)
- 분리되어 있으면 onboarding 한정 inline drain (helper 추출은 본 plan 비대상 — Phase 2)

## Layer B — 회귀 가드 grep / unit test (선택)

### B1. unit test (선택)
- `cmd_with_invalid_exec_returns_stderr_in_error()` — fake codex binary (exit 1 with stderr "boom") 호출 → 에러 메시지에 "boom" 포함 검증
- 단순 fix 라 unit test 필수는 아님 — 작성 시 회귀 가드 됨

# Invariants

- INV-1: onboarding codex 호출이 exit != 0 일 때 사용자/로그 가시 에러 메시지에 stderr 본문 포함
- INV-2: stderr 가 비어 있을 때 graceful fallback (`(no stderr)` 메시지)
- INV-3: 정상 종료 (exit 0) path 동작 변경 0 — stderr 무시 그대로
- INV-4: production codex (`agents/codex.rs`) path 변경 0
- INV-5: F2~F5 가설 기반 fix 는 본 plan 비대상 (별 PR)

# 검증

## 자동
- `cd src-tauri && cargo check`
- `cd src-tauri && cargo test --lib` (Rust 559 baseline 동일 또는 +N)
- `npx tsc --noEmit`
- `npx vitest run` (FE 381 baseline 동일)

## 수동 smoke (가능한 한)
- onboarding 실행 → codex 의도적 fail (예: `--full-auto` + git repo 부재 환경) → 에러 메시지에 stderr 본문 보임 확인
- onboarding 정상 path → 동작 변경 0

# Developer 핸드오프 프롬프트

`docs/plans/onboardingCodexStderrSurfacePlan_2026-04-29.md` 의 Layer A 수행. 단순 fix (~10 LoC).

**작업 절차**

1. `src-tauri/src/commands/project_onboarding.rs:576~601` 영역 읽기
2. `agents/codex.rs:55~130` 의 production stderr drain 패턴 참조 (helper 재사용 가능 시 import, 아니면 inline)
3. `Stdio::null()` → `Stdio::piped()` + drain. exit != 0 시 에러 메시지에 stderr 본문 포함
4. 검증:
   - `cd src-tauri && cargo check`
   - `cd src-tauri && cargo test --lib`
   - `npx tsc --noEmit`
   - `npx vitest run`
5. 회귀 가드 grep:
   - `git diff src-tauri/src/agents/codex.rs` (변경 0 확인)
   - `git diff src-tauri/src/commands/project_onboarding.rs` (577~601 영역 한정 확인)
6. 커밋:
   - `fix(onboarding): pipe codex stderr for failure diagnostics (Plan B follow-up)`
   - trailer:
     ```
     Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
     ```
7. push + `gh pr create` (title: `fix(onboarding): surface codex stderr on exit != 0`)
8. **[CI 정책]** PR 직후 `gh pr merge --squash --delete-branch --admin` 즉시 머지. CI watch 불필요 (변경 영역 작고 코드 회귀 위험 0).
9. 반환값:
   - PR URL + 머지 commit
   - 변경 라인 수 + 핵심 diff (1~2줄)
   - Verification 결과 (4 검증)
   - production codex.rs 변경 0 grep 결과

**DO NOT**
- F2~F5 fix 시도 X (skip-git-repo-check / color=never / current_dir / 인자 순서) — 별 PR
- production `agents/codex.rs` 변경 X
- onboarding 의 다른 단계 (rawq, identity, scaffold 등) 변경 X
- 새 dependency 추가 X

# 셀프 이슈 본문 초안

> ## fix(onboarding): surface codex stderr on exit != 0
>
> Plan B Task 02 진단 follow-up. batmania52 보고 (codex exit 1 root cause 미식별) 의 1차 진단 가능성 회복.
>
> ### 진단
>
> `commands/project_onboarding.rs:583` 가 `Stdio::null()` 로 stderr 를 버림 → 에러 메시지에 본문 미포함 → 사용자/로그 어디서도 codex exit code 외 정보 미가시. 가설 H1~H4 검증 자체가 불가능.
>
> ### Plan
>
> `docs/plans/onboardingCodexStderrSurfacePlan_2026-04-29.md` Layer A — `Stdio::piped()` + drain. 약 10 LoC 변경. F2~F5 가설 기반 fix 는 별 PR.
