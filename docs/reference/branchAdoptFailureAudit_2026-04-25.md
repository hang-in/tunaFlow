---
title: Branch adopt 실패 매트릭스 audit
canonical: false
status: audit-snapshot
created_at: 2026-04-25
related:
  - docs/plans/branchAdoptRollbackPlan_2026-04-25.md
  - docs/reference/asyncCancelPipelineAudit_2026-04-25.md
  - src-tauri/src/commands/branches.rs
  - src/stores/slices/branchSlice.ts
---

# 동기

`branchAdoptRollbackPlan_2026-04-25` 의 audit 단계 산출물. plan 수립 시점엔 본문
미분석으로 LLM 호출 / cancel 채널 / 부분 적용 위험을 가설로 둔 상태였으나, 실제
구현을 읽고 보면 **현재 adopt 경로는 LLM 비호출 + 단일 mutex-locked sync 명령**
이라 plan 의 Layer B/C/D 가 적용되지 않는다. 본 audit 은 그 격차를 명시한다.

# Audit 결과 — plan 가정 vs 실제

## 가정 1 — "LLM summary 생성 단계가 있다"

**틀림**.

`src-tauri/src/commands/branches.rs::adopt_branch` (line 448~593) 본문에 **LLM
호출이 없다**. summary_body 는 다음 두 경로 중 하나로 결정된다:

1. **RT 브랜치**: `memos` 테이블에서 `roundtable_brief` 타입 메모를 조회 후
   "Key Positions" 섹션 추출 (line 468~498)
2. **일반 브랜치**: branch 의 마지막 assistant 메시지에서 300자 미리보기 추출
   (line 501~521)

둘 다 **순수 SQLite SELECT** 으로 끝나고, 외부 호출 / 네트워크 / async 가 없다.
plan 의 Layer B (LLM retry/abort) 는 **현재 아키텍처에 적용 대상이 없다**.

## 가정 2 — "adopting 같은 중간 상태가 있다"

**틀림**.

`branches.status` 컬럼은 `'active' | 'adopted' | 'archived'` 만 사용한다. adopt
중에 `'adopting'` 같은 중간 상태로 마킹되지 않는다 (`UPDATE branches SET status =
'adopted'` 한 번만 실행, line 530~533). plan 의 Layer D (앱 기동 시 stuck
"adopting" row 복구 UX) 는 **현재 데이터 모델에 해당 row 가 발생할 수 없다**.

## 가정 3 — "background async task 가 있어 cancel 가능"

**틀림**.

`adopt_branch` 는 `#[tauri::command] pub fn` 이고 (async 아님) sync 컨텍스트에서
실행된다. invoke 시점에 mutex 잠그고 returned 시점에 unlock — 중간에 cancel 할
수 있는 await 지점이 없다. UI 가 드로어를 닫아도 rust 작업은 이미 끝났거나 mutex
가 해제 대기 중일 뿐이다. plan 의 Layer C (드로어 dismiss → cancel command) 는
**cancel 할 task 가 없으므로 적용 불가**.

## 가정 4 — "DB write 가 부분 적용될 수 있다"

**맞음 — 이 plan 의 유일한 실제 위험**.

`adopt_branch` 는 단일 mutex-locked Connection 위에서 다음 4 개 write 를 순차
실행한다 (모두 별개 statement, transaction 미사용):

| # | 위치 | 동작 |
|---|---|---|
| 1 | line 530~533 | `UPDATE branches SET status='adopted' WHERE id=?` |
| 2 | line 536~545 | `UPDATE branches SET status='archived' WHERE id IN (descendants)` (recursive CTE) |
| 3 | line 565~569 | `INSERT INTO messages (...summary message...)` |
| 4 | line 575~578 | `UPDATE branches SET adopted_message_id=? WHERE id=?` |

**부분 적용 시나리오**:

- Step 1 성공 → step 2 실패 (예: descendants 가 plan FK 와 충돌하지 않으나 다른
  trigger 가 fail): branch 는 `adopted`, descendants 는 여전히 `active`, parent
  conversation 에 summary 메시지 없음 → INV-1 위반
- Step 1+2 성공 → step 3 실패 (예: parent conversation 이 이 사이에 삭제됨, FK
  위반): branch 만 `adopted` 로 마킹되고 summary 메시지는 없음 → INV-1 위반
- Step 1+2+3 성공 → step 4 실패: summary 는 들어갔지만
  `branches.adopted_message_id` 는 NULL → mobile δ-Branch 상세 조회 시 "어떤
  turn 으로 흡수됐는지" 매핑 실패 (regression)
- 가장 흔한 트리거: **process crash / power loss** 도중. WAL mode 라 마지막
  COMMIT 전엔 statement 만 page cache 에 머물 수 있고, recovery 시 일부 statement
  만 살아남을 가능성

**즉시 수정 필요**: 4 개 write 를 `BEGIN; ... COMMIT;` 한 트랜잭션으로 묶어
모두-적용-or-모두-롤백 보장.

## 가정 5 — "s25 의 'adopt 중 스트리밍 메시지 소멸' 수정이 같은 카테고리"

**부분 일치, 그러나 별개 문제**.

git log 에서 s25 시기 (2026-04-12) 직접 fix 커밋은 보이지 않으나, 현재
`branchSlice.ts:124` 의 adopt 후처리 코드 (line 132~135) 에 다음 패턴이 있다:

```ts
// Preserve in-memory streaming messages not yet saved to DB
const streamingMsgs = get().messages.filter((m) => m.status === "streaming");
const dbIds = new Set(freshMessages.map((m) => m.id));
const messages = [...freshMessages, ...streamingMsgs.filter((m) => !dbIds.has(m.id))];
```

이는 "adopt 직후 `list_messages` 결과로 store 를 덮어쓰면 아직 DB 에 저장 안 된
streaming 메시지가 사라진다" 를 막는 fix 이다. **DB 부분 적용** 과는 다른 layer
(store ↔ DB sync) 의 race condition. 본 plan 의 범위 (Rust DB transaction 보장)
와 직교한다 — 이미 fix 되어 있으므로 retain.

# 수정 매트릭스

| Plan Layer | 가설 | 실제 적용성 | 본 PR 범위 |
|---|---|---|---|
| A — DB transaction | step 3+4 | **유효 + 확장 (step 1~4 전체)** | ✅ 포함 |
| B — LLM retry/abort | step 2 | LLM 호출 없음 — N/A | ❌ 불필요 |
| C — UI dismiss cancel | long-running task | sync command — N/A | ❌ 불필요 |
| D — partial recovery UX | "adopting" row | 해당 status 미존재 — N/A | ❌ 불필요 |

# Invariants — 본 PR 후 보장

- **[INV-1]** adopt 가 Ok(Message) 를 반환했다면 다음이 모두 보장된다:
  - branch.status = 'adopted'
  - 모든 descendants.status = 'archived'
  - parent conversation 에 summary message 존재
  - branch.adopted_message_id = summary message id
- **[INV-2]** adopt 가 Err 를 반환했다면 위 4 가지 중 어느 것도 적용되지 않는다
  (transaction rollback). 단 process crash / power loss 시 SQLite WAL 의 atomic
  commit 보장에 의존
- **[INV-3]** 동일 branch 를 두 번 adopt 호출 — 첫 번째 호출에서 status 가
  'adopted' 로 바뀌므로 두 번째는 line 455~461 의 `WHERE status = 'active'`
  쿼리에서 fail (NotFound). 부작용 없음 (idempotent — 두 번째 호출은 no-op)

# 본 audit 의 한계

- HTTP API 경로 (`http_api/conversations.rs::adopt_branch`) 는 별도 구현이며, 본
  audit 은 desktop Tauri command 만 다룬다. HTTP 측은 bytes count 만 다른 동일
  로직을 호출하므로 같은 transaction fix 가 자동 적용된다고 추정 (검증 필요 시
  follow-up)
- branchSlice.ts 의 adoptBranch 는 backend 호출 + 후처리만 하므로 본 PR 의 Rust
  변경에 영향받지 않는다

# 후속 plan 정리

- `branchAdoptRollbackPlan_2026-04-25.md` 의 Layer B/C/D 는 **현재 아키텍처와
  무관** 한 가설이었으므로 별도 plan 으로 승격할 필요 없다
- 만약 향후 adopt 에 LLM-generated 요약이 도입되면 (e.g. RT brief 가 없는 일반
  branch 에 대해 한 줄 요약을 모델에 요청하는 경우), 그 시점에 Layer B/C 다시
  검토
