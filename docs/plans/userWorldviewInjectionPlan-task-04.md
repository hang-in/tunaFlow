# Subtask 04 — Background Job 스켈레톤 (enqueue API + UI status + pending cancel)

> 상위 plan: [userWorldviewInjectionPlan.md](./userWorldviewInjectionPlan.md)
>
> **Codex round-1 review 2026-04-23 반영** — 당초 설계의 worker 실행 경로 (`execute_job`, `stash_insight_from_job`, `record_trace_log_start/end`, `AppState` 등) 는 tunaFlow 실코드에 대응 helper 가 없어 구현 진입 blocker. 본 subtask 는 **스켈레톤만** 제공하고, 실제 worker 실행은 metaAgent 착수 시점의 별도 plan 으로 위임. 대신 enqueue / cancel / UI status 경로를 완전히 마감해 후속 plan 이 안전하게 얹을 기반을 제공.

## Scope

본 subtask 가 **제공하는 것**:
- `agent_jobs` 확장 컬럼 (`priority` / `dedupe_key` / `visibility`) 을 사용하는 `enqueue_job` helper
- `cancel_background_job` Tauri command — **pending cancel 은 보장**, running cancel 은 status 플래그만 변경 (best-effort, INV-4 축소 반영)
- `count_pending_background_jobs` Tauri command — UI polling 용
- Frontend StatusBar 컴포넌트 (polling + event listener)

본 subtask 가 **제공하지 않음** (별도 plan):
- Worker loop 실제 실행 경로 (`execute_job`)
- Insight stash 결과 연결 (`stash_insight_from_job`)
- `trace_log` 기록 통합 (기존 `insert_trace_log_with_context` 재사용 설계는 후속)
- Cooperative cancel token / subprocess kill
- 실제 trigger 측 — metaAgent 책임

## Changed files

- `src-tauri/src/commands/jobs.rs` — `enqueue_job` helper + `cancel_background_job` / `count_pending_background_jobs` Tauri commands.
- `src-tauri/src/lib.rs` — 신규 command 등록.
- `src/lib/api/backgroundJobs.ts` (신규) — FE API wrapper + event listener.
- `src/components/tunaflow/StatusBar.tsx` (또는 동등) — "background insight: N pending" 표시 + cancel 버튼.

## Change description

### 1. `enqueue_job` helper

```rust
// src-tauri/src/commands/jobs.rs
pub fn enqueue_job(
    conn: &Connection,
    conversation_id: &str,
    message_id: Option<&str>,
    engine: &str,
    kind: &str,
    priority: i64,           // 0=foreground, -1=background
    dedupe_key: Option<&str>,
    visibility: &str,        // "visible" (본 plan 에서는 항상 visible)
) -> Result<Option<String>, AppError> {
    // dedupe 체크 — 같은 key 의 pending/running job 있으면 skip
    if let Some(key) = dedupe_key {
        let exists: bool = conn.query_row(
            "SELECT 1 FROM agent_jobs WHERE dedupe_key = ?1 AND status IN ('pending','running') LIMIT 1",
            [key], |_| Ok(true),
        ).optional()?.unwrap_or(false);
        if exists { return Ok(None); }
    }
    let id = format!("job-{}", Uuid::new_v4());
    let now = now_epoch_ms();
    conn.execute(
        "INSERT INTO agent_jobs (id, conversation_id, message_id, engine, kind, status, started_at, updated_at, priority, dedupe_key, visibility)
         VALUES (?1,?2,?3,?4,?5,'pending',?6,?6,?7,?8,?9)",
        params![id, conversation_id, message_id, engine, kind, now, priority, dedupe_key, visibility],
    )?;
    Ok(Some(id))
}
```

Foreground 경로는 기존 `create_job` 유지 (priority=0 default). Background 는 신규 `enqueue_job` 사용. 본 subtask 는 `enqueue_job` 을 직접 호출하는 FE 경로를 제공하지 않는다 — 호출자는 후속 plan 이 추가.

### 2. `cancel_background_job` Tauri command (pending 보장, running best-effort)

```rust
#[tauri::command]
pub fn cancel_background_job(
    job_id: String,
    state: State<DbState>,
) -> Result<CancelResult, AppError> {
    let w = state.write.lock().map_err(|_| AppError::Lock)?;

    // pending → cancelled (보장)
    let pending_rows = w.execute(
        "UPDATE agent_jobs SET status='cancelled', updated_at=?1
          WHERE id=?2 AND status='pending'",
        params![now_epoch_ms(), job_id],
    )?;
    if pending_rows > 0 {
        return Ok(CancelResult::PendingCancelled);
    }

    // running → status 만 cancelled (best-effort)
    // INV-4 축소 반영: 실행 중 subprocess 는 본 plan 범위 밖. 후속 plan 에서 cooperative cancel token 추가.
    let running_rows = w.execute(
        "UPDATE agent_jobs SET status='cancelled', updated_at=?1
          WHERE id=?2 AND status='running'",
        params![now_epoch_ms(), job_id],
    )?;
    if running_rows > 0 {
        return Ok(CancelResult::RunningStatusChanged);
    }

    Ok(CancelResult::NotFound)
}

#[derive(serde::Serialize)]
pub enum CancelResult {
    PendingCancelled,      // 즉시 제거됨
    RunningStatusChanged,  // status 만 변경 (실제 종료는 후속 plan)
    NotFound,              // 이미 done/failed/cancelled 또는 없음
}
```

### 3. `count_pending_background_jobs` Tauri command

```rust
#[tauri::command]
pub fn count_pending_background_jobs(state: State<DbState>) -> Result<i64, AppError> {
    let r = state.read.lock().map_err(|_| AppError::Lock)?;
    let n: i64 = r.query_row(
        "SELECT COUNT(*) FROM agent_jobs WHERE priority < 0 AND status = 'pending'",
        [], |r| r.get(0),
    )?;
    Ok(n)
}
```

### 4. FE API wrapper

```ts
// src/lib/api/backgroundJobs.ts
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

export type CancelResult = 'PendingCancelled' | 'RunningStatusChanged' | 'NotFound';

export function cancelBackgroundJob(jobId: string): Promise<CancelResult> {
    return invoke('cancel_background_job', { jobId });
}

export function countPendingBackgroundJobs(): Promise<number> {
    return invoke('count_pending_background_jobs');
}

export type BackgroundProgress = {
    jobId: string;
    state: 'started' | 'done' | 'failed' | 'cancelled';
    kind?: string;
};

/** 후속 plan 에서 worker 가 emit 시 사용. 본 subtask 는 listener 만 노출. */
export function subscribeBackgroundProgress(
    handler: (p: BackgroundProgress) => void,
): Promise<() => void> {
    return listen<BackgroundProgress>('background_insight_progress', (e) => handler(e.payload))
        .then((un) => () => un());
}
```

### 5. Frontend StatusBar

```tsx
// src/components/tunaflow/StatusBar.tsx
export function BackgroundJobStatusItem() {
    const [pending, setPending] = useState(0);
    const [activeJob, setActiveJob] = useState<{ id: string; kind: string } | null>(null);

    // polling — 15초. 후속 plan 이 event-driven 으로 개선 가능.
    useEffect(() => {
        let cancelled = false;
        const tick = () => {
            if (cancelled) return;
            countPendingBackgroundJobs().then((n) => { if (!cancelled) setPending(n); });
        };
        tick();
        const interval = setInterval(tick, 15000);
        return () => { cancelled = true; clearInterval(interval); };
    }, []);

    // 후속 plan 에서 worker 가 emit 시작하면 activeJob 표시됨. 현재는 대기만.
    useEffect(() => {
        const unlisten = subscribeBackgroundProgress((p) => {
            if (p.state === 'started') setActiveJob({ id: p.jobId, kind: p.kind ?? 'unknown' });
            else setActiveJob(null);
        });
        return () => { unlisten.then(fn => fn()); };
    }, []);

    if (pending === 0 && !activeJob) return null;
    return (
        <div className="status-bar-item">
            <Spinner size="xs" />
            <span>
                Background insight: {activeJob?.kind ?? 'queued'}
                {pending > 0 && ` · ${pending} pending`}
            </span>
            {activeJob && (
                <button onClick={async () => {
                    const result = await cancelBackgroundJob(activeJob.id);
                    if (result === 'PendingCancelled' || result === 'RunningStatusChanged') {
                        setActiveJob(null);
                    }
                }}>
                    취소
                </button>
            )}
        </div>
    );
}
```

### 6. Enqueue 예제 (scope 외 — 주석 only)

```
// 후속 plan (metaAgent 또는 별도 background executor) 에서:
//   enqueue_job(conn, conv_id, None, "claude", "insight_background",
//               -1, Some("boredom-insight-2026-04-22T19"), "visible")
```

## Dependencies

depends_on: [02] — `agent_jobs` priority/dedupe_key/visibility 컬럼 필요.

## Verification

- `cargo test --lib commands::jobs::enqueue_dedupe`:
  - 같은 `dedupe_key` 로 연속 enqueue 시 두 번째는 `Ok(None)` 반환
  - 이전 job 이 `cancelled` / `failed` / `done` 이면 같은 key 재 enqueue 허용
- `cargo test --lib commands::jobs::cancel_background_job`:
  - pending → `PendingCancelled` + DB 에 `status='cancelled'`
  - running → `RunningStatusChanged` + DB status 변경 (실 실행 없는 환경에서 row 만 검증)
  - 이미 done/failed → `NotFound`
- `cargo test --lib commands::jobs::count_pending_background_jobs`:
  - priority=-1 AND status='pending' row 만 카운트
  - priority=0 row 는 제외 (foreground)
- `npx vitest run src/components/tunaflow/StatusBar.test.tsx`:
  - `countPendingBackgroundJobs` mock 이 값 반환 시 배지 표시
  - pending=0 + activeJob=null 시 컴포넌트 null 반환
  - 취소 버튼 클릭 → `cancelBackgroundJob` invoke 호출
- 수동 E2E:
  1. SQLite CLI 로 임의 `agent_jobs` row INSERT (`priority=-1, status='pending'`)
  2. StatusBar 에 "Background insight: queued · 1 pending" 표시 확인
  3. UI 취소 버튼 → row status='cancelled' 확인
  4. Worker 실 실행은 본 subtask 범위 밖 — 별도 plan 추가 후 재검증

## Risks

- **scope 축소의 함정**: 본 subtask 단독으로는 background job 이 실행되지 않으므로, metaAgent 측이 `enqueue_job` 을 호출하기 시작해도 pending 쌓이기만 하고 실행 안 됨. 후속 plan 순서를 반드시 "worker executor → metaAgent trigger" 로 맞춰야 사용자 체감 작동.
- **"queued N pending" UI 상태가 오래 유지**: 실행기 부재로 영구 pending 발생. 본 subtask 머지 시점에는 metaAgent 도 enqueue 안 하므로 pending=0 으로 StatusBar 가 렌더 안 됨 (문제 없음). 문제는 **executor 없이 metaAgent 만 먼저 머지** 되는 경우 — 본 plan 의 index 또는 metaAgentPlan 에 "executor subtask 의존성" 명시 필요. 본 subtask 의 Risks 섹션으로 보존.
- **cancel semantics 축소로 사용자 혼동**: "취소" 버튼 클릭 시 running job 은 즉시 종료되지 않음. UI 메시지 "취소 요청 — 현재 작업 종료 후 반영" 로 soft 표현 권장.
- **polling 15초 주기**: 응답성 낮지만 CPU 부담 작음. Event-driven 으로 개선 여지 — 후속.
- **INV-4 준수 검증**: scope 축소가 INV-4 를 깼는가? 아니다 — INV-4 를 "pending 보장 / running best-effort" 로 plan 본문에서 축소했고, 본 subtask 는 그 축소된 정의에 정확히 부합.
