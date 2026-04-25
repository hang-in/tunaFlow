---
title: Plan 생성 실패 매트릭스 audit (planGenerationRollback 후속)
canonical: false
status: audit-snapshot
created_at: 2026-04-25
related:
  - docs/plans/planGenerationRollbackPlan_2026-04-25.md
  - docs/reference/asyncCancelPipelineAudit_2026-04-25.md  # 항목 4
---

# 동기

`planGenerationRollbackPlan_2026-04-25` 의 §"Developer 핸드오프 프롬프트 (1) Audit" 산출물.
Plan 의 1차 가설은 "Plan 생성 = LLM call (Tauri command 위임)" 이었음 — Rust 단 LLM 호출 / DB write / 파일 write 의 단계별 실패 매트릭스 + UI catch 체인 검증.

# 결론 요약 (먼저)

- **tunaFlow 의 Plan 생성 경로에 Rust 단 LLM 호출은 존재하지 않는다.**
- Plan 본문은 Architect agent 가 채팅 marker (`<!-- tunaflow:plan -->`) 로 생성 → UI 가 `PlanProposalCard` 로 노출 → 사용자가 promote/overwrite 시 frontend 가 `create_plan` / `replace_plan_subtasks` 등 **순수 DB command** 를 호출.
- `generate_plan_document` (plans.rs:677) 는 LLM 호출이 아니라 **DB → markdown 렌더 + 파일 write** 만 수행.
- 따라서 plan 의 Layer B (LLM 응답 파싱 강건화) / Layer C (invoke timeout + cancel) / Layer D (stale "generating" 상태 복구) 는 **tunaFlow 의 현재 아키텍처에 적용되지 않는다.**
  - 단, **Layer A (atomic DB + file write)** 는 실재 위험으로 확인됨 — 본 audit 의 핵심 발견.

# 실제 Plan 생성 흐름 (tunaFlow)

```
[Architect agent (채팅 응답)]
   └─ <!-- tunaflow:plan ... --> marker 생성 (LLM 응답 본문 안)
        ↓ (frontend 가 message 스트리밍 받음)
[planProposalParser.ts] marker 파싱 → ParsedPlanProposal
        ↓
[PlanProposalCard.tsx] 사용자에게 promote/overwrite 버튼 노출
        ↓ (사용자 클릭)
[planApi (plans.ts) → Tauri invoke]
   ├─ create_plan         (신규)        — plans.rs:107
   ├─ replace_plan_subtasks (덮어쓰기) — plans.rs:392
   ├─ updatePlanMeta / linkPlanBranch / updatePlanPhase ...
   └─ syncPlanDocument (fire-and-forget)
        ↓
[generate_plan_document]  — plans.rs:677
   ├─ DB read (plan + subtasks + events)
   ├─ build_plan_markdown(...)
   └─ std::fs::write(file_path, &md)
```

핵심 포인트:
- **LLM 호출은 Architect 응답 단계에서 끝난다.** Plan DB insert 시점에는 이미 marker payload 가 메모리에 있고, 파싱 실패 시 `PlanProposalCard` 가 promote 버튼을 노출하지 않는다 (UI 단 차단).
- DB insert 는 frontend 에서 multi-step 으로 호출 (create_plan + replace_plan_subtasks + updatePlanMeta + linkPlanBranch + ...) — 각 단계가 독립 Tauri command. 중간 실패 시 부분 적용 가능.
- 파일 write 실패는 `syncPlanDocument` 가 catch 후 `console.warn` 만 — 사용자에게 비가시.

# 단계별 실패 매트릭스

## Step 1 — Architect agent LLM 응답

| 실패 모드 | 현재 처리 | 위험도 |
|---|---|---|
| LLM timeout / network | engine 단 retry/error → 채팅 영역에 에러 표시 | 낮음 (UI 가시화 됨) |
| 응답에 marker 없음 | `PlanProposalCard` 미노출 | 낮음 (사용자가 재요청 가능) |
| marker 형식 깨짐 | `planProposalParser` 가 partial 추출 시도 → 부족하면 `PlanProposalCard` 가 비노출 | 낮음 |

→ **tunaFlow 단에서 별도 조치 불필요** (LLM 단 timeout / cancel 은 engine 별 핸들러 책임).

## Step 2 — `create_plan` (plans.rs:107)

```rust
let conn = state.write.lock().map_err(|_| AppError::Lock)?;
// ...
conn.execute("INSERT INTO plans ...", ...)?;          // (a)
for (i, st) in input.subtasks.iter().enumerate() {
    conn.execute("INSERT INTO plan_subtasks ...", ...)?;  // (b) loop
}
```

| 실패 모드 | 현재 처리 | 위험도 |
|---|---|---|
| (a) plan INSERT 실패 (FK / unique) | 즉시 return Err | 낮음 (전무) |
| (b) subtask 중간 INSERT 실패 | **plan 은 남고 일부 subtask 만 들어간 상태** | **중간** — 부분 적용 |
| disk lock / SQLITE_BUSY | rusqlite 가 자동 retry (busy_timeout 설정시) | 낮음 |

→ **위험 확인**: subtask loop 중 실패 시 부분 상태. WAL 모드에서도 transaction 으로 묶어야 atomic.

## Step 3 — `replace_plan_subtasks` (plans.rs:392)

```rust
let conn = state.write.lock().map_err(|_| AppError::Lock)?;
conn.execute("DELETE FROM plan_subtasks WHERE plan_id = ?1", ...)?;  // (a)
conn.execute("UPDATE plans SET revision = revision + 1 ...", ...)?;  // (b)
for (i, st) in subtasks.iter().enumerate() {
    conn.execute("INSERT INTO plan_subtasks ...", ...)?;  // (c) loop
}
```

| 실패 모드 | 현재 처리 | 위험도 |
|---|---|---|
| (a) DELETE 후 (c) 중간 실패 | **subtask 가 일부만 남거나 전부 사라진 상태** | **높음** — 사용자가 작성한 plan body 가 부분 손실 가능 |
| (b) UPDATE 만 성공 후 (c) 실패 | revision 만 bump 된 상태 (논리적 불일치) | 중간 |

→ **위험 확인**: 가장 위험한 경로. PlanProposalCard 의 overwrite / autoMerge / DraftingActions / MergeBranchButton / PlanCard 모두에서 호출.

## Step 4 — `generate_plan_document` (plans.rs:677)

```rust
// DB read (lock release 후)
std::fs::create_dir_all(&dir).map_err(...)?;   // (a)
let file_path = dir.join(format!("{}.md", slug));
if !file_path.exists() {
    std::fs::write(&file_path, &md).map_err(...)?;  // (b)
}
```

| 실패 모드 | 현재 처리 | 위험도 |
|---|---|---|
| (b) 파일 write 중 disk full / 권한 | std::fs::write 는 atomic 이 **아님** (chunked write 가능) → 부분 .md 가능 | 중간 |
| 파일이 이미 존재 → skip | 의도된 동작 (Architect 가 직접 작성한 경우 보존) | OK |
| `syncPlanDocument` (frontend) 가 invoke 실패 | `console.warn` 만, 사용자 비가시 | 낮음 (재생성 가능 — DB 가 SSOT) |

→ **위험 확인**: write 가 atomic 이 아닌 점은 LL 위험. tempfile + rename 패턴이 권장.

# Plan 의 Layer A~D 적용 가능성 재평가

| Layer | Plan 가설 | tunaFlow 실제 | 결론 |
|---|---|---|---|
| A. Rust transaction + atomic write | DB + file 한 묶음 | DB 단 transaction 적용 가능. file 쪽은 tempfile+rename 으로 atomic write 보강 가능 | **적용 — 본 PR 에서 구현** |
| B. LLM 응답 파싱 강건화 | retry / raw 노출 | tunaFlow Rust 단 LLM 없음 (Architect agent 가 채팅으로 처리) | **N/A** — 별도 plan 불필요 (engine 단 책임) |
| C. invoke timeout + cancel | invoke N초 timeout | `generate_plan_document` 는 < 100ms (DB read + 짧은 file write). LLM 호출이 없음 | **N/A** — 시급성 없음 |
| D. stale "generating" 상태 복구 | 부팅 시 stale plan 검사 | tunaFlow 에 plan.status="generating" 상태가 없음 (status: draft/active/done/abandoned). | **N/A** — 스키마 미해당 |

# 본 PR 구현 범위 (Layer A 만)

1. `create_plan`: 트랜잭션으로 plan + subtask 일괄. 중간 실패 시 전부 롤백.
2. `replace_plan_subtasks`: DELETE + UPDATE + INSERT loop 트랜잭션화. 부분 적용 방지.
3. `generate_plan_document`: 파일 write 를 tempfile + rename 으로 atomic 화. 부분 .md 방지.
4. 회귀 테스트: 트랜잭션 롤백 동작 확인 (subtask 중간 실패 시 plan 도 없어야 함).

# 미반영 (의도적)

- Layer B/C/D — 위 표 참조. 적용 가능 시점에 별도 plan 으로 분리.
- 다른 plan 관련 multi-step (frontend 에서 createPlan + replaceSubtasks + linkBranch 를 sequential 로 호출하는 PlanProposalCard 의 `handleOverwrite`) 의 multi-command 부분 적용 위험. — 본 PR 범위 밖. 향후 별 plan 으로 다룰 것 (예: `planMultiStepRollbackPlan`).

# 검증 plan

- `cargo test` 신규 테스트:
  - `create_plan_rollback_on_subtask_failure` — subtask insert 가 실패하면 plan 도 없어야 함.
  - `replace_plan_subtasks_rollback_on_failure` — INSERT 중간 실패 시 기존 subtask 가 보존되어야 함.
- 수동 (선택): `chmod -w docs/plans` 로 디렉터리 readonly → `generate_plan_document` 호출 → 부분 파일 없음 확인.

# 한계

- 본 audit 는 **Rust 단 plan 경로만** 감사. PlanProposalCard 의 frontend multi-step (overwrite 시 6+ 개 invoke 순차 호출 — 중간 실패 시 partial state) 은 동일 카테고리 위험이지만 별도 plan 후보로 분리.
- 트랜잭션 도입 후에도 **Tauri command 단위 atomicity** 만 보장. 여러 command 를 frontend 에서 순차 호출하는 경우는 여전히 부분 적용 가능.
