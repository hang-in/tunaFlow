---
title: Developer 핸드오프 — agent watchdog trailing kill 차단 + reviewer read tool guard
plan: docs/plans/watchdogAndReviewerReadGuardPlan_2026-04-29.md
created_at: 2026-04-29
---

# Developer 핸드오프 — watchdog trailing kill + reviewer read guard

## 0. 한 줄 요약

v0.1.4-beta publish 직전 마지막 hardening 2 건 — claude.rs watchdog RAII guard 추가 (Task 01) + REVIEWER_TEMPLATE 에 result.md read 금지 명시 (Task 02). 합본 PR 1 개로 처리.

## 1. 작업 개요 — 2 task, 합본 PR 권장

**Plan SSOT**: `docs/plans/watchdogAndReviewerReadGuardPlan_2026-04-29.md`. 작업 시작 전 Plan 의 §3 Subtasks 를 그대로 따를 것.

| Task | 파일 | 핵심 변경 | 우선 |
|---|---|---|---|
| 01 | `src-tauri/src/agents/claude.rs` | watchdog `done` AtomicBool + RAII WatchdogGuard | P1 |
| 02 | `src-tauri/src/commands/project_tools.rs` | REVIEWER_TEMPLATE 에 read 금지 라인 + 기존 항목 표현 강화 | P1 |

**브랜치**: `feat/agent-watchdog-and-reviewer-read-guard` 권장.
**PR title 예시**: `fix(agents,reviewer): watchdog trailing kill + reviewer read tool guard`.
**커밋 단위**: task 별 분리 권장. `fix(claude): RAII guard for agent timeout watchdog (Task 01)` / `fix(reviewer): forbid reading *-result.md from disk (Task 02)`.

## 2. 중요 — Task 01 의 사전 상태

Architect 가 이전 세션에서 Task 01 의 코드 변경을 **이미 worktree 에 적용**했고 commit 만 안 된 상태다. 즉 다음과 같이 보일 것:

```
$ git status -s
 M src-tauri/src/agents/claude.rs
```

**처리 방식 (권장)**:
1. `git diff src-tauri/src/agents/claude.rs` 로 Architect 의 변경분 검토.
2. Plan §3 Task 01 의 Change description 과 일치하는지 라인 단위 확인.
3. 일치하면 그대로 stage + commit.
4. 불일치하거나 의심스러우면 chat 으로 Architect 에게 escalate.

**거절 처리** (예: Architect 변경이 plan 의도와 다르다고 판단되면):
1. `git checkout -- src-tauri/src/agents/claude.rs` 로 revert.
2. Plan §3 Task 01 Change description 대로 새로 적용.
3. chat 으로 revert + 재적용 사유 보고.

## 3. DO — 반드시 지킬 것

1. **Plan §3 의 Verification 명령을 task 마다 실제로 실행** 하고 결과를 chat 보고.
2. **Task 01 → Task 02 순서**. Task 01 단독 PR 분리도 가능 (만약 Task 02 에 막히면).
3. **회귀 위험 가드** 각 task 의 "회귀 위험 가드" 섹션을 작업 전후로 확인. 특히:
   - Task 01: `claude.rs` 의 `stream_run` 외 함수 변경 금지. `idle_timeout=600s` 값 변경 금지. 다른 agent 파일 절대 수정 금지.
   - Task 02: `DEVELOPER_TEMPLATE` / `ARCHITECT_TEMPLATE` / 기타 상수 절대 변경 금지. REVIEWER_TEMPLATE 의 Critical Rules + What is NOT a fail reason **두 섹션 한정** 변경.
4. **PR description 에 Plan 링크 + 두 Verification 결과 + diff hash** 첨부.
5. **PR 머지 후** `CHANGELOG.md` 의 `[0.1.4-beta] - 2026-04-29` 섹션 `### Fixed` 영역에 Plan §6 의 두 entry 추가 (별도 PR 또는 같은 PR 의 추가 commit, 자유 선택).

## 4. DO NOT — 사이드 이펙트 차단

- ❌ 다른 agent 파일 (`agents/codex.rs`, `agents/gemini.rs`, `agents/ollama.rs`, `agents/lmstudio.rs`, `agents/claude_sdk_session.rs`) 변경.
- ❌ `DEVELOPER_TEMPLATE` / `ARCHITECT_TEMPLATE` / `META_TEMPLATE` 변경.
- ❌ REVIEWER_TEMPLATE 의 Role / Review Procedure / Verdict Format / Re-review 섹션 변경.
- ❌ `idle_timeout` 값 변경 (600s 그대로).
- ❌ watchdog `thread::spawn` 을 `JoinHandle.join()` 으로 변환.
- ❌ ContextPack 어셈블리 (`context_loading.rs`) 추가 변경 — PR #211 에서 이미 처리됨.
- ❌ 새 dependency 추가.
- ❌ `[0.1.5-beta]` 새 CHANGELOG 섹션 만들기 (cadence 고려, v0.1.4-beta 안에 묶음).

## 5. v0.1.4-beta release publish 체크리스트 (PR 머지 후)

PR 머지가 완료되면 사용자(또는 Architect 별도 트리거) 가 다음을 진행. **이 핸드오프 본 작업의 일부는 아니지만 컨텍스트 공유 차원에서 명시**.

```bash
# 1. main 동기화 + tag 확인
git fetch origin
git checkout main
git pull origin main
git log --oneline -5            # PR 211 + 본 PR + CHANGELOG 보강 commit 확인

# 2. CHANGELOG entry 검증
grep -A 5 "0.1.4-beta" CHANGELOG.md | head -40

# 3. 기존 v0.1.4-beta tag 가 있다면 force-update 위험 평가
git tag -l "v0.1.4-beta"
# tag 가 이미 있고 외부 release 가 publish 안 됐다면 (Draft 상태) 삭제 후 재생성:
# git tag -d v0.1.4-beta
# git push origin :refs/tags/v0.1.4-beta
# git tag -a v0.1.4-beta -m "v0.1.4-beta — emergency claude transport flip + result.md contamination + watchdog/reviewer hardening"
# git push origin v0.1.4-beta

# 4. release 자산 재빌드 (CI 또는 로컬)
# scripts/build-release.sh 또는 GitHub Actions 트리거

# 5. Draft release 자산 교체 후 Publish (사용자 직접 트리거 — gh release edit/upload)
```

**주의**: tag 강제 갱신 (`-d` + force push) 은 외부에 release 가 publish 안 된 Draft 상태에서만 권장. 이미 publish 된 tag 는 immutable 로 처리.

## 6. 변경 후 검증 (전체)

```bash
cd src-tauri && cargo check --message-format=short
cd src-tauri && cargo test --lib
npx tsc --noEmit
npx vitest run

# 회귀 grep
git diff src-tauri/src/agents/                    # claude.rs 만 변경되었는지
git diff src-tauri/src/commands/project_tools.rs  # REVIEWER_TEMPLATE 두 섹션만 변경되었는지
rg "result\.md" src-tauri/src/commands/project_tools.rs
```

테스트 카운트 baseline (PR #211 직후): **FE 381 / Rust 559**. 작업 후 동일 또는 +N (새 unit test 만큼). 감소 시 회귀.

## 7. CI 정책

- PR 직후 admin merge 즉시 가능 (CI watch 불필요). 자체 검증 §6 통과 후 self-merge.
- merge 후 main 에서 추가 회귀 발생 시 즉시 revert PR 생성.
- v0.1.4-beta release 자산 재빌드는 본 작업 scope 외, 사용자 직접 트리거.

## 8. 보고 포맷

작업 완료 시 chat 에:
- task 별 변경 라인 수 + diff 요약 1~2줄
- 각 Verification 결과 (PASS/FAIL + 핵심 출력)
- baseline 대비 테스트 카운트 (FE/Rust)
- PR URL + 머지 commit hash
- CHANGELOG entry 추가 commit hash (별도 PR 인 경우 그 PR URL)
- 회귀 위험 가드 위반 없음 확인 (특히 다른 agent 파일/다른 template 미변경 grep 결과 1줄)

## 9. 막히면

- Task 01 의 worktree diff 가 plan 의도와 다르다고 판단되면 코드 수정 전 chat 에서 escalate.
- Task 02 의 텍스트 표현이 어색하면 (한국어 영역 아니고 영문 시스템 프롬프트) 의미를 유지하면서 자연스럽게 다듬어도 무방. 단 핵심 키워드 "Never read", "filesystem", "*-result.md" 는 유지.
- CHANGELOG entry 표현은 Plan §6 와 의미만 일치하면 자유 — 정확한 한 자 한 자 그대로일 필요 없음.
