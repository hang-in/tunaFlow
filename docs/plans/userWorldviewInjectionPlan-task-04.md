# Subtask 04 — Low-priority background insight worker (visible + cancelable)

> 상위 plan: [userWorldviewInjectionPlan.md](./userWorldviewInjectionPlan.md)

## Changed files

- `src-tauri/src/commands/jobs/background_worker.rs` (신규) — polling loop + job executor.
- `src-tauri/src/commands/jobs.rs` — job enqueue helper 확장 (priority + dedupe_key).
- `src-tauri/src/lib.rs` — 앱 startup 에 worker task spawn.
- `src/lib/api/backgroundJobs.ts` (신규) — FE API wrapper + event listener.
- `src/components/tunaflow/StatusBar.tsx` (또는 동등) — "background insight: N pending" 표시.

## Change description

### 1. Enqueue helper

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
    // dedupe 체크 — 같은 key 의 pending job 있으면 skip
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

Foreground 경로는 기존 `create_job` 유지 (priority=0 default). Background 는 신규 `enqueue_job` 사용.

### 2. Worker loop

```rust
// src-tauri/src/commands/jobs/background_worker.rs
pub fn spawn_background_worker(app: AppHandle, state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;

            // Foreground busy 면 skip
            if is_foreground_busy(&state) { continue; }

            // pick 1 pending background job (priority ASC = -1 먼저)
            let job: Option<JobRecord> = {
                let r = state.db.read.lock().ok()
                    .and_then(|conn| {
                        conn.prepare(
                            "SELECT id, conversation_id, message_id, engine, kind, priority, dedupe_key
                             FROM agent_jobs WHERE status='pending' AND priority < 0
                             ORDER BY priority ASC, updated_at ASC LIMIT 1"
                        ).ok()
                         .and_then(|mut s| s.query_map([], row_to_job).ok()
                             .and_then(|mut it| it.next().transpose().ok().flatten()))
                    })
            };
            let Some(job) = job else { continue; };

            // mark running + emit start
            mark_job_status(&state, &job.id, "running").ok();
            app.emit("background_insight_progress",
                json!({"jobId": job.id, "state": "started", "kind": job.kind})
            ).ok();

            // trace_log 에 시작 기록 — INV-4
            record_trace_log_start(&state, &job);

            // 실행 (기존 agent 실행 경로 재활용)
            let result = execute_job(&app, &state, &job).await;

            // 완료
            let status = if result.is_ok() { "done" } else { "failed" };
            mark_job_status(&state, &job.id, status).ok();
            app.emit("background_insight_progress",
                json!({"jobId": job.id, "state": status})
            ).ok();

            // trace_log 종료 기록
            record_trace_log_end(&state, &job, result.as_ref().err().map(|e| e.to_string()));

            // Insight stash: 성공 시 기존 insightOrchestration 파이프라인 호출
            if let Ok(output) = result {
                crate::commands::insight_extract::stash_insight_from_job(&state, &job, output).await.ok();
            }
        }
    });
}

fn is_foreground_busy(state: &AppState) -> bool {
    // agent_jobs 에 priority=0 AND status='running' 1건 이상 있으면 busy
    let r = state.db.read.lock();
    if let Ok(c) = r {
        let cnt: i64 = c.query_row(
            "SELECT COUNT(*) FROM agent_jobs WHERE priority = 0 AND status = 'running'",
            [], |r| r.get(0),
        ).unwrap_or(0);
        return cnt > 0;
    }
    false
}
```

**concurrency cap = 1** 은 loop 구조로 자연스럽게 보장 (1 iteration 당 1 job).

### 3. Cancel 경로

```rust
#[tauri::command]
pub fn cancel_background_job(
    job_id: String,
    state: State<DbState>,
) -> Result<(), AppError> {
    let w = state.write.lock().map_err(|_| AppError::Lock)?;
    w.execute(
        "UPDATE agent_jobs SET status='cancelled', updated_at=?1
          WHERE id=?2 AND status IN ('pending','running')",
        params![now_epoch_ms(), job_id],
    )?;
    // 이미 running 중이면 child process kill 은 별도 (추후 확장)
    Ok(())
}
```

### 4. FE status bar

```tsx
// src/components/tunaflow/StatusBar.tsx
export function StatusBar() {
    const [pending, setPending] = useState(0);
    const [activeJob, setActiveJob] = useState<{ id: string; kind: string } | null>(null);

    useEffect(() => {
        const poll = setInterval(async () => {
            const count = await invoke<number>('count_pending_background_jobs');
            setPending(count);
        }, 15000);
        return () => clearInterval(poll);
    }, []);

    useEffect(() => {
        const unlisten = listen<BackgroundProgress>('background_insight_progress', (e) => {
            if (e.payload.state === 'started') {
                setActiveJob({ id: e.payload.jobId, kind: e.payload.kind });
            } else {
                setActiveJob(null);
            }
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
                <button onClick={() => invoke('cancel_background_job', { jobId: activeJob.id })}>
                    취소
                </button>
            )}
        </div>
    );
}
```

### 5. Enqueue 예제 (본 plan 의 scope 밖 — 주석으로만 남김)

```
// 향후 metaAgent 가 "사용자 지루함 감지" 시:
//   enqueue_job(..., priority=-1, kind="insight_background",
//               dedupe_key="boredom-insight-2026-04-22T19")
```

본 subtask 는 enqueue 경로와 worker 만 제공. 실제 트리거는 metaAgent 가 담당 (Q-5 참조).

## Dependencies

depends_on: [02] — agent_jobs priority/dedupe_key/visibility 컬럼 필요.

## Verification

- `cargo test --lib commands::jobs::tests::enqueue_dedupe`:
  - 같은 dedupe_key 로 연속 enqueue 시 두 번째는 Ok(None) 반환
  - status='cancelled' / 'failed' 이후 같은 key 는 재 enqueue 허용
- `cargo test --lib commands::jobs::background_worker::tests`:
  - Foreground job running 시 background pick 안 함
  - Foreground 없을 때 priority -1 pending 을 하나 pick
  - 완료 시 status='done' + trace_log 기록 + event emit
  - INV-4: trace_log 에 해당 job_id 관련 2건 이상 (start/end)
- `cargo test --lib commands::jobs::cancel_background_job`:
  - pending → cancelled
  - running → cancelled (child kill 은 별도 확장 시점까지 best-effort)
  - 이미 done/failed 은 no-op
- `npx vitest run src/components/tunaflow/StatusBar.test.tsx`:
  - pending > 0 시 배지 표시
  - `background_insight_progress { state: 'started' }` 이벤트 시 activeJob 설정
- 수동 E2E:
  1. `enqueue_job` 을 임의로 호출 (테스트용 Tauri command)
  2. StatusBar 에 "Background insight: ..." 표시 확인
  3. 취소 버튼 → 즉시 사라짐
  4. `trace_log` 에 해당 job 의 start/end 기록 확인

## Risks

- **Polling 30초 주기**: 응답성은 낮지만 CPU 부담 작음. event-driven 으로 개선은 Q-4 follow-up.
- **Foreground busy 판정**: `priority = 0 AND status = 'running'` 기준. 그러나 streaming 중 job 이 pending → running 전이가 빠르면 race. 1 iteration 건너뛰는 정도의 tolerance 는 허용.
- **child kill 미구현**: cancel 이 running job 에 대해 status 만 변경. 실제 subprocess 종료는 별도 확장. 본 plan 의 background job 은 일반적으로 1~2 분 이내 종료 — 죽을 때까지 기다려도 큰 문제 없음. 장시간 job 도입 시 확장 필요.
- **Worker crash 시 job 상태**: worker task 가 panic 하면 running job 이 영원히 running 상태. App restart 시 `bootstrap/db.rs:19` 의 stale cleanup 이 정리. 단 같은 session 내 recovery 는 없음 — 수용.
- **`count_pending_background_jobs` 명령 누락**: Tauri command 로 추가 필요 (SELECT COUNT(*) WHERE priority<0 AND status='pending'). FE polling 용.
- **dedupe_key 네임스페이스**: 사용자 프로젝트 간 dedupe 가 교차하면 곤란. 컨벤션: `<project_key>:<kind>:<timestamp_bucket>` 형태 권장 — 실제 kind 별로 enqueue 호출자가 결정.
- **Concurrency cap=1 한계**: 대용량 indexing 과 insight 가 경쟁. MVP 충분. 확장 시 per-kind cap 으로.
