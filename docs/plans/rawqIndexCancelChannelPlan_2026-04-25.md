---
title: rawq index build cancel 채널 추가 (subprocess kill 대안)
status: implemented (Option A — 2026-04-25)
priority: P3 (영향 범위 작음 — UI freeze 까진 안 가는 것으로 추정)
created_at: 2026-04-25
updated_at: 2026-04-25
related:
  - docs/reference/asyncCancelPipelineAudit_2026-04-25.md  # 항목 5
  - src-tauri/src/agents/rawq.rs  # ensure_index_cancellable, rebuild_index_cancellable
  - src-tauri/src/commands/project_tools.rs  # cancel_rawq_index (이번 PR), rebuild_rawq_index
  - src-tauri/src/commands/projects.rs       # RawqIndexing { active, cancels }
  - src/lib/bootstrap/project.ts             # teardownPreviousProject — auto cancel
canonical: true
owners:
  - architect (본 plan 작성)
  - developer (구현)
---

# 배경

`asyncCancelPipelineAudit_2026-04-25` 의 항목 5 — rawq index build 중 명시적 cancel 채널 부재.

현재 `ensure_index` 는 `Command::wait_with_output` 으로 동기 대기 (`rawq.rs:348`). subprocess kill 외 cancel 경로 없음. 사용자가 프로젝트를 닫아도 rawq subprocess 가 끝까지 진행 → CPU/메모리 일시 점유.

# 현실적 영향

- **UI freeze 까진 안 감** — rawq 는 별 thread 에서 spawn 되어 main loop 블로킹 안 함
- **CPU 점유 일시** — 큰 프로젝트 (Rust target/ 같은 큰 디렉터리 — 다만 #180 fix 로 exclude 됨) 는 여전히 수 분 진행 가능
- **사용자 경험** — "왜 indexing 이 끊기지 않고 끝까지 가나" 의문. 명시적 stop/cancel UI 없음

# 우선순위 낮은 이유

- 영향 범위가 작음 (백그라운드만)
- 현실적 trigger 가 적음 (사용자가 프로젝트 닫고 다른 거 열 때 정도)
- onboarding / branch adopt / plan generation 과 달리 데이터 일관성 위험 없음 (rawq DB 는 자체 idempotent)

# 수정 방향 가설

## Option A — subprocess kill on Drop (간단)

`start_rawq_index` 가 spawn 한 thread / Child 의 reference 를 보관. 사용자가 프로젝트 닫으면 (또는 새 프로젝트 열면) `Child::kill()` 호출.

장점: rawq upstream 변경 없음. tunaFlow 단 수정만.
단점: 즉시 종료 (graceful shutdown 없음). 부분 인덱스 남을 수 있음 (다음 build 에서 재생성하니 영향 작음).

## Option B — rawq daemon 모드 활용 (현재 daemon 모드 운영 중이라면)

rawq daemon 에 cancel 신호를 socket 으로 전송. graceful shutdown.

장점: 깔끔. 부분 인덱스 정리 가능.
단점: rawq upstream 에 cancel 신호 채널 추가 필요. PR 작성 비용.

## Option C — rawq upstream 에 cancel flag 추가 PR

`rawq index build` 에 `--cancel-on-signal SIGTERM` 같은 옵션. tunaFlow 가 그 신호 활용.

장점: 다른 rawq 사용자도 혜택.
단점: 시간 큼.

# 권장: Option A

가장 빠르고 수용 가능한 절충. subprocess kill 은 OS 레벨 기본 메커니즘이라 안정적. graceful shutdown 못 하는 단점은 "부분 인덱스 → 다음 build 시 재생성" 으로 흡수.

# Invariants

- **[INV-1]** start_rawq_index 호출자가 cancel 가능 (실시간 또는 lifecycle hook)
- **[INV-2]** cancel 후 부분 인덱스가 남아도 다음 ensure_index 호출이 정상 작동 (idempotent / 자동 재빌드)
- **[INV-3]** 동시 진행 중인 다른 프로젝트의 rawq build 에 영향 없음

# Developer 핸드오프 프롬프트

```
[작업] rawq index build cancel 채널 추가 (Plan rawqIndexCancelChannel / asyncCancel audit #5)

[SSOT] docs/plans/rawqIndexCancelChannelPlan_2026-04-25.md + docs/reference/asyncCancelPipelineAudit_2026-04-25.md

[배경 3줄]
- rawq subprocess 가 사용자 프로젝트 dismiss 후에도 끝까지 진행 → CPU 점유
- UI freeze 까진 안 가지만 사용자 경험상 의문 발생
- 우선순위 낮은 cleanup — 다른 audit 항목 (onboarding/branch/plan) 처리 후 진행

[수정 범위 — Option A 권장]

1) src-tauri/src/agents/rawq.rs:
   - ensure_index 시그니처에 Optional<Arc<AtomicBool>> cancel flag 추가
   - 또는 spawn 한 Child 의 reference 를 caller 가 보관할 수 있게 변경
   - graceful 종료 시 Child::kill() 호출

2) src-tauri/src/commands/project_tools.rs:
   - start_rawq_index 가 Child reference 를 RawqIndexing State 에 저장
   - 신규 cancel_rawq_index command (또는 기존 lifecycle 에 통합)
   - 사용자가 프로젝트 dismiss 시 자동 cancel

3) src/lib/api/rawq.ts:
   - cancelRawqIndex 함수 (필요 시)

4) UI hook:
   - 프로젝트 dismiss 시 invoke("cancel_rawq_index", { projectPath }) 호출
   - 또는 자동 cleanup (lifecycle hook 으로)

[검증]
- cargo check / cargo test
- 수동: 큰 프로젝트 indexing 진행 중 다른 프로젝트 열기 → 첫 프로젝트 indexing 즉시 중단 확인 (Activity Monitor)

[커밋 분리]
- feat(rawq): cancel-aware ensure_index + Child reference保管
- feat(rawq): cancel_rawq_index command + state 통합
- feat(ui): auto-cancel rawq on project dismiss

[셀프 이슈]
"feat: rawq index build cancel channel (audit follow-up, low priority)"
```

# 구현 결과 (2026-04-25)

Option A 채택. 구현 매핑:

- `agents/rawq.rs` — `RawqError::Cancelled` 추가, `ensure_index_cancellable(path, Option<Arc<AtomicBool>>)` / `rebuild_index_cancellable(...)` 신설. 100 ms poll loop 에서 flag 감시 후 `Child::kill() + wait()` (좀비 방지). 기존 `ensure_index` / `rebuild_index` 는 `None` cancel 로 위임하는 호환 wrapper 로 유지 (jobs.rs 등 변경 영향 0).
- `commands/projects.rs` — `RawqIndexing` 이 `active: HashSet<String>` + `cancels: HashMap<String, Arc<AtomicBool>>` 두 필드를 갖도록 확장. `RawqIndexing::new()` 추가.
- `commands/project_tools.rs` — `register_indexing()` helper 로 단일 lock scope 에서 duplicate guard + cancel flag 등록. `start_rawq_index`/`rebuild_rawq_index` 가 cancel-aware 변형 호출. `Cancelled` 결과는 `rawq:cancelled` 이벤트로 emit (success/error 와 분리). 신규 `cancel_rawq_index` command (idempotent — 알 수 없는 path 는 false 반환 no-op).
- `bootstrap/services.rs` — `RawqIndexing` 생성을 `RawqIndexing::new()` 으로 단순화.
- `lib.rs` — `cancel_rawq_index` 등록.
- `src/lib/bootstrap/project.ts` — `activeRawqProjectPath` module 변수로 진행 중 path 추적. `teardownPreviousProject()` 가 idempotent cancel invoke. fs-watcher debounce 호출 시에도 path 갱신. 신규 `rawq:cancelled` 이벤트 listener 추가 — 사용자에게 error 가 아니라 `ready` + "indexing cancelled" 메시지로 표시.

# Invariants — 검증

- INV-1 ✅ — `cancel_rawq_index(projectPath)` 가 신규 command 로 노출. `teardownPreviousProject` 가 lifecycle hook 으로 자동 호출.
- INV-2 ✅ — Cancel 후 부분 인덱스가 남아도 다음 `ensure_index_cancellable` 의 `is_indexed` 체크가 동작하며, false 면 다시 build (rawq DB 자체 idempotent).
- INV-3 ✅ — `RawqIndexing.cancels` 가 path-keyed HashMap 이라 다른 프로젝트의 build 와 격리. 각 호출이 own `Child` 를 보유.

테스트 커버리지: `agents/rawq.rs::cancel_tests` 5 케이스 (`cancel_is_set` 분기 3 + `ensure_index_cancellable` / `rebuild_index_cancellable` 의 pre-set flag 단축경로 2). 501 cargo lib tests + 344 vitest 통과.

# 후속 — Upstream PR (선택)

Option A 머지 후 안정 운영 검증되면, **Option B/C 로 rawq upstream 에 cancel 신호 PR 제안 가능**. rawq upstream owner (auyelbekov) 가 active 한 maintainer 라 (#12 이슈 응답 1-2 일 내 약속) 협조 가능성 높음.

# 알려진 미구현

- 사용자가 명시적 "cancel" 버튼을 누르는 UI 는 추가하지 않음. plan 우선순위 P3 + 핸드오프의 주된 요구사항(프로젝트 dismiss 시 자동 cancel) 만 충족. RuntimeSection 의 `재빌드 중…` 버튼은 그대로 disabled 유지. 명시적 cancel UI 가 필요하면 별 plan 으로 분리.

# 관련 기록

- `asyncCancelPipelineAudit_2026-04-25` 항목 5
- rawq #12 (upstream gitignore 이슈) 와 별 트랙 — 이 plan 은 tunaFlow 단 수정 우선
