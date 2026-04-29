---
title: agent watchdog trailing kill 차단 + reviewer read tool guard
status: ready
phase: planning
owner: developer-handoff
created_at: 2026-04-29
updated_at: 2026-04-29
canonical: true
context: v0.1.4-beta publish 직전 마지막 hardening. PR #211 후속.
related:
  - docs/prompts/watchdogAndReviewerReadGuardDeveloperHandoff_2026-04-29.md
  - docs/plans/resultMdContaminationFixPlan_2026-04-29.md
  - src-tauri/src/agents/claude.rs
  - src-tauri/src/commands/project_tools.rs
---

# watchdog trailing kill 차단 + reviewer read tool guard

## 1. Context

v0.1.4-beta 자산 publish 직전 발견된 두 작은 hardening 항목을 묶음. 둘 다
재현 빈도 낮지만 영향이 명확하고, 같은 release 묶음에 포함시키는 게 안전.

1. **claude.rs agent watchdog 가 reader 종료 후에도 살아남아 trailing kill 발생**
   - `[guardrail] status=ok duration=350692ms` 정상 종료 후에도 watchdog 30s
     loop 가 누적 idle 카운팅 → 이미 reap 된 PID 에 `kill -9` 송출. PID 재사용
     시 엉뚱한 프로세스 kill 위험 (확률 낮지만 0 아님).
   - 실제 로그: `[agent-timeout] No output for 623s, killing pid 10195` 와
     `[guardrail] status=ok duration=350692ms` 같은 사이클에 동시 출현.

2. **Codex reviewer 가 `*-result.md` 를 자체 read tool 로 직접 열람**
   - PR #211 (resultMdContaminationFix) 가 ContextPack 입력에서 result.md 를
     제거했음에도, Codex 가 `sed -n / nl -ba ... docs/plans/...-result.md`
     같은 line-oriented read 로 직접 파일 열람 → 잘림 패턴을 verdict 근거로 사용.
   - Codex 본인이 정책 위반 인정함 (Q1 답: "(b) 자체 read 도구로 직접").
   - REVIEWER_TEMPLATE 의 "Never judge result.md" 규칙은 *judge* 만 다룸,
     *read* 행위 자체는 명시 금지 안 됨.

## 2. Goals / Non-goals

### Goals
- (G1) reader loop 종료 / cancel / early return 모든 경로에서 watchdog 30s
  sleep 누적이 trailing kill 로 이어지지 않도록 RAII guard 적용.
- (G2) REVIEWER_TEMPLATE 에 "Never *read* `*-result.md` via filesystem tools"
  규칙 명시. read 행위 자체를 정책 위반으로 정의.

### Non-goals
- ❌ Codex CLI 측의 read tool 제거/제한 (Anthropic 영역, 우리 통제 외).
- ❌ filesystem 권한 sandbox 적용 (과한 변경, 다른 합법 read 영향).
- ❌ watchdog 의 idle_timeout 값 조정 (600s 그대로).
- ❌ 다른 agent (codex/gemini/ollama/lmstudio) 의 watchdog 변경 — 같은 패턴이
  다른 곳에 없음을 이미 확인 (s40 grep 결과: claude.rs 단독).

## 3. Subtasks

### Task 01 — claude.rs watchdog 에 done flag + RAII guard 추가 [P1]

**상태**: Architect 가 working tree 에 이미 적용한 변경분이 존재 (이전 세션
의사결정으로 Architect 가 직접 코드 수정함, commit 안 됨). Developer 는
**diff 검토 후 그대로 commit** 또는 **revert 후 plan 대로 새로 적용**.
권장: diff 검토 + 그대로 commit (변경 내용이 plan 의 의도와 일치).

**Changed files**: `src-tauri/src/agents/claude.rs`

**Change description** (worktree 에 이미 적용된 내용과 동일):
- `idle_timeout` 블록 (현재 라인 ~216) 에 다음 추가:
  - `let watchdog_done = Arc<AtomicBool::new(false)>` 새 flag.
  - watchdog `thread::spawn` loop 내부 `thread::sleep(30s)` 직후 `if done_flag.load(SeqCst) { break }` 체크 추가.
  - 함수 scope 끝까지 살아있는 RAII guard:
    ```rust
    struct WatchdogGuard(Arc<AtomicBool>);
    impl Drop for WatchdogGuard {
        fn drop(&mut self) { self.0.store(true, SeqCst); }
    }
    let _watchdog_guard = WatchdogGuard(Arc::clone(&watchdog_done));
    ```
  - 기존 `timed_out` flag 와 reader 루프 / 정상 종료 분기는 변경 없음.

**Verification**:
- `cd src-tauri && cargo check --message-format=short` 통과
- `cd src-tauri && cargo test --lib agents::claude` 또는 `cargo test --lib`
  전체 통과 — baseline 카운트 (PR #211 직후 559) 와 동일.
- diff 가 plan 의도와 일치하는지 chat 에 diff 첨부 (`git diff src-tauri/src/agents/claude.rs`).

**회귀 위험 가드**:
- `claude.rs` 의 다른 함수 (`stream_run` 외) 변경 금지.
- `idle_timeout` 값 (600s) 변경 금지.
- 다른 agent 파일 (`codex.rs`, `gemini.rs`, `ollama.rs`, `lmstudio.rs`,
  `claude_sdk_session.rs`) 절대 손대지 말 것.
- watchdog `thread::spawn` 의 detached 패턴 유지 — `JoinHandle.join()` 으로
  바꾸면 함수 종료가 30s 까지 지연됨.

---

### Task 02 — REVIEWER_TEMPLATE 에 read tool 사용 금지 명시 [P1]

**Changed files**: `src-tauri/src/commands/project_tools.rs`

**Change description**:
- REVIEWER_TEMPLATE 상수 (line ~649-707) 의 "Critical Rules" 섹션
  (line ~700-707) 에 한 줄 추가:
  ```
  - **Do NOT read `*-result.md` from disk**: Even with sed/cat/nl/read tools,
    accessing the result report file is the same policy violation as judging
    it. The result report is auto-generated and not part of the review contract.
  ```
- 또한 "What is NOT a fail reason" 섹션 (line ~684-691) 의 result report 항목
  표현 강화:
  ```
  - Result report quality, content, structure, OR existence — it is auto-generated
    by tunaFlow, not the Developer's work. Do not read or judge `*-result.md`.
  ```
- 두 곳 모두 line wrap 80~100자 유지. 기존 다른 rule 항목 변경 금지.

**Verification**:
- `cd src-tauri && cargo check`
- `rg "result.md" src-tauri/src/commands/project_tools.rs` 로 두 신규 라인이
  포함되어 있는지 확인 (3 라인 이상 매치 예상: 기존 + 신규 2).
- 기존 DEVELOPER_TEMPLATE / 다른 상수 영역에 변경이 없는지 `git diff` 로 확인.

**회귀 위험 가드**:
- DEVELOPER_TEMPLATE 절대 변경 금지.
- ARCHITECT_TEMPLATE / META_TEMPLATE 등 다른 상수 변경 금지.
- 변경 라인은 REVIEWER_TEMPLATE 의 Critical Rules + What is NOT a fail reason
  섹션 한정. 다른 섹션 (Role, Review Procedure, Verdict Format, Re-review)
  건드리지 말 것.
- 이번 변경 후 reviewer 가 ContextPack 입력에서 받은 result.md (PR #211 로
  이미 차단됨) 도 똑같이 판정 근거로 못 쓴다는 의미라 redundant 하지 않음 —
  텍스트 명시가 read 행위 차단의 핵심.

## 4. Cross-cutting risks

| 위험 | 대응 |
|---|---|
| Codex 가 새 규칙도 무시 (행동 확정성 없음) | Q4 에서 본인이 명시적 준수 약속함. 향후 review 라운드에서 같은 위반 재발 시 강한 조치 (REVIEWER_TEMPLATE 에 "If you read result.md, your verdict is automatically void" 같은 escalation) 추가 plan. |
| Task 01 RAII guard 가 panic-safe 인지 | `Drop` impl 은 panic 시에도 호출됨 (rust 표준 동작). 다만 `AtomicBool::store` 자체는 panic 안 함. OK. |
| watchdog thread 가 30s sleep 중 함수 exit 시 process 종료까지 늦어지는가 | thread 가 detached 라 main 흐름 차단 안 함. 다만 process 가 살아있는 동안 thread 도 idle. main 종료 시 OS 가 정리. 이슈 없음. |

## 5. Rollback

- Task 01 단독 revert 가능 (`git revert <commit>`).
- Task 02 단독 revert 가능.
- 두 task 가 독립이라 합본 PR 도 한 commit 단위 revert 로 처리 가능.

## 6. Release notes (v0.1.4-beta entry 보강)

PR 머지 후 `CHANGELOG.md` 의 `[0.1.4-beta] - 2026-04-29` 섹션 `### Fixed`
영역에 다음 두 항목 추가:

```md
- **Reviewer 정책 위반 차단** (PR #211 + 후속) — Codex Reviewer 가
  `*-result.md` 를 자체 read tool 로 직접 열람 후 잘림 패턴을 verdict 근거로
  사용하던 정책 위반 패턴 확인. ContextPack 입력 차단 (PR #211, root cause)
  에 더해 REVIEWER_TEMPLATE 에 "Never read `*-result.md`" 규칙 명시 추가
  (이 plan). reportSync 의 truncation 도 UTF-8 boundary-safe 8k/2k 상한 +
  잘림 마커 + sentinel 기반 self-include guard 로 강화.
- **claude agent watchdog trailing kill 차단** — reader loop 정상 종료 후
  watchdog 30s sleep 누적이 이미 reap 된 PID 에 `kill -9` 송출하던 race.
  PID 재사용 시 엉뚱한 프로세스 kill 위험 0 으로 차단. RAII guard 패턴.
```

PR #211 entry 와 watchdog entry 는 같은 v0.1.4-beta 안에 묶임 — separate
`[0.1.5-beta]` 섹션 만들지 말 것 (cadence 고려).

## 7. Out of plan

- v0.1.4-beta 자산 재빌드 + GitHub release Draft → Publish 절차는 이 plan
  scope 외 (release ops 영역). 핸드오프 §5 에 체크리스트 별도 정리.
- REVIEWER_TEMPLATE 에 "verdict void if you read result.md" 같은 escalation
  은 후속 (재발 관측 후). 지금은 명시 추가만.
