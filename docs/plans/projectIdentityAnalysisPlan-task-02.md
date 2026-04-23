# Subtask 02 — metaAgent trigger (2 조건 AND) + analysis job

> 상위 plan: [projectIdentityAnalysisPlan.md](./projectIdentityAnalysisPlan.md)
>
> **선행 의존**: `docs/plans/metaAgentPlan.md` (P0) 의 metaAgent 기본 구조. metaAgent 가 아직 구현 전이면 본 subtask 는 metaAgent 의 첫 실사용 예제로도 기능.

## Changed files

- `src-tauri/src/commands/meta_agent.rs` (신규 또는 확장) — trigger 감시 + analysis job enqueue.
- `src-tauri/src/commands/meta_agent/identity_trigger.rs` (신규) — `should_trigger_identity_analysis` 함수 + 관련 쿼리.
- `src-tauri/src/commands/agent_jobs.rs` (또는 jobs.rs) — `kind='identity_analysis'` 지원.
- `src-tauri/src/commands/plans.rs` — `complete_plan` 말미에서 metaAgent trigger 체크 훅 추가.
- `src/components/tunaflow/settings/IdentityAnalysisSettings.tsx` (신규) — threshold 튜닝 UI.
- `src-tauri/src/db/migrations.rs` — 변경 **없음** (artifacts 재사용, 새 테이블 X).

## Change description

### 1. Trigger 함수

```rust
// src-tauri/src/commands/meta_agent/identity_trigger.rs

pub struct IdentityTriggerDecision {
    pub should_run: bool,
    pub done_plan_count: i64,
    pub eligible_artifact_count: i64,
    pub threshold: i64,           // current Settings value
    pub reason: &'static str,     // debug trail: "count_mod3" / "volume_min" / "ok"
}

pub fn evaluate_identity_trigger(
    conn: &Connection,
    project_key: &str,
) -> Result<IdentityTriggerDecision, AppError> {
    // 조건 A: plan done count % 3 == 0
    let done_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM plans WHERE status='done' AND project_key=?1",
        [project_key], |r| r.get(0),
    )?;
    if done_count == 0 || done_count % 3 != 0 {
        return Ok(IdentityTriggerDecision {
            should_run: false, done_plan_count: done_count,
            eligible_artifact_count: 0, threshold: load_threshold(conn)?,
            reason: "count_mod3",
        });
    }

    // 이전 identity_summary 시점
    let last_summary_at: i64 = conn.query_row(
        "SELECT COALESCE(MAX(created_at), 0) FROM artifacts
          WHERE type='identity_summary'
            AND conversation_id IN (SELECT id FROM conversations WHERE project_key=?1)",
        [project_key], |r| r.get(0),
    )?;

    // 조건 B: eligible artifact volume
    let eligible_count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM artifacts
          WHERE type IN ('decision','review_outcome','rework_reason',
                         'finding_success','finding_failure','workflow_milestone')
            AND created_at > ?1
            AND conversation_id IN (SELECT id FROM conversations WHERE project_key=?2)",
        params![last_summary_at, project_key], |r| r.get(0),
    )?;
    let threshold = load_threshold(conn)?;
    if eligible_count < threshold {
        return Ok(IdentityTriggerDecision {
            should_run: false, done_plan_count: done_count,
            eligible_artifact_count: eligible_count, threshold,
            reason: "volume_min",
        });
    }

    Ok(IdentityTriggerDecision {
        should_run: true, done_plan_count: done_count,
        eligible_artifact_count: eligible_count, threshold,
        reason: "ok",
    })
}

fn load_threshold(conn: &Connection) -> Result<i64, AppError> {
    // 우선 env var override (dev/test 용)
    if let Ok(v) = std::env::var("TUNAFLOW_IDENTITY_ANALYSIS_THRESHOLD") {
        if let Ok(n) = v.parse::<i64>() { return Ok(n); }
    }
    // DB 설정 (Settings UI 에서 관리)
    conn.query_row(
        "SELECT value FROM app_settings WHERE key='identity_analysis.min_artifacts'",
        [], |r| r.get::<_, String>(0),
    ).ok()
     .and_then(|s| s.parse::<i64>().ok())
     .map(Ok)
     .unwrap_or(Ok(10))   // default = 10
}
```

**`app_settings` 테이블 선행 필요**: 현재 tunaFlow 에 app_settings 테이블이 없으면 별도 plan 또는 본 subtask 에 migration 추가. 일단 본 subtask 는 env var 또는 상수 default=10 으로 진행 가능. Settings UI 는 localStorage + FE 에서 rust 측 set command 호출 패턴도 허용.

### 2. Trigger 훅 연결

`complete_plan` 말미 (plan.status='done' 전이 성공 후):

```rust
// src-tauri/src/commands/plans.rs
pub fn complete_plan(...) -> Result<(), AppError> {
    // ... 기존 transition 로직 ...

    // Fire-and-forget trigger check. 실패해도 plan 완료에는 영향 없음.
    let project_key = plan.project_key.clone();
    let app_handle = app.clone();
    tokio::task::spawn_blocking(move || {
        let conn = state.read.lock().ok()?;
        let decision = evaluate_identity_trigger(&conn, &project_key).ok()?;
        if decision.should_run {
            enqueue_identity_analysis_job(&app_handle, &project_key, decision);
        }
        Some(())
    });

    Ok(())
}
```

### 3. Analysis job enqueue

`agent_jobs` 재사용. kind 확장:

```rust
pub fn enqueue_identity_analysis_job(
    app: &AppHandle,
    project_key: &str,
    decision: IdentityTriggerDecision,
) {
    let job_id = format!("id-analysis-{}-{}", project_key, now_epoch_ms());

    // agent_jobs INSERT (기존 스키마 재사용, priority=-1 background)
    // metadata: project_key + since_ts + until_ts (decision.done_plan_count 기준)
    // subtask-03 의 분석 에이전트가 이 job 을 pick up

    app.emit("identity_analysis_triggered", serde_json::json!({
        "jobId": job_id, "projectKey": project_key,
        "donePlanCount": decision.done_plan_count,
        "eligibleArtifacts": decision.eligible_artifact_count,
    })).ok();
}
```

### 4. 수동 트리거 command

Settings UI 에서 "지금 분석" 버튼이 호출:

```rust
#[tauri::command]
pub async fn trigger_identity_analysis_now(
    project_key: String,
    force: bool,                // true 면 threshold 무시
    state: State<'_, DbState>,
    app: AppHandle,
) -> Result<IdentityTriggerDecision, AppError> {
    let conn = state.read.lock().map_err(|_| AppError::Lock)?;
    let mut decision = evaluate_identity_trigger(&conn, &project_key)?;
    if force { decision.should_run = true; decision.reason = "forced"; }
    if decision.should_run {
        drop(conn);
        enqueue_identity_analysis_job(&app, &project_key, decision.clone());
    }
    Ok(decision)
}
```

Settings UI 에서 decision 반환값 렌더 — threshold 미달 시 `done_plan_count`, `eligible_artifact_count`, `threshold` 숫자 표시해 사용자가 "왜 안 돌지?" 이해.

### 5. Settings UI (IdentityAnalysisSettings.tsx)

```tsx
export function IdentityAnalysisSettings() {
    const [threshold, setThreshold] = useState(10);
    const [latest, setLatest] = useState<IdentityTriggerDecision | null>(null);
    const projectKey = useChatStore((s) => s.selectedProjectKey);

    useEffect(() => { loadThreshold().then(setThreshold); }, []);

    const runNow = async (force: boolean) => {
        const decision = await invoke<IdentityTriggerDecision>(
            'trigger_identity_analysis_now', { projectKey, force }
        );
        setLatest(decision);
    };

    return (
        <section>
            <h3>Identity Analysis</h3>
            <div className="setting-row">
                <label>최소 artifact 수 (threshold)</label>
                <input type="number" value={threshold} min={3} max={50}
                    onChange={(e) => {
                        const v = Number(e.target.value);
                        setThreshold(v);
                        saveThreshold(v);
                    }} />
                <p className="hint">
                    이전 분석 이후 누적된 eligible artifact 수가 이 값 이상이고
                    Plan done count 가 3의 배수일 때 분석이 실행됩니다. 기본 10.
                </p>
            </div>
            <div className="setting-row">
                <button onClick={() => runNow(false)}>지금 확인</button>
                <button onClick={() => runNow(true)} variant="ghost">강제 실행 (threshold 무시)</button>
            </div>
            {latest && (
                <div className="trigger-status">
                    <div>Plans done: {latest.done_plan_count} ({latest.done_plan_count % 3 === 0 ? "OK" : `need ${3 - latest.done_plan_count % 3} more`})</div>
                    <div>Eligible artifacts: {latest.eligible_artifact_count} / {latest.threshold}</div>
                    <div>Status: {latest.should_run ? "analysis enqueued" : `skipped (${latest.reason})`}</div>
                </div>
            )}
        </section>
    );
}
```

## Dependencies

depends_on: [01] — 자동 생성되는 artifact 가 있어야 trigger 조건 B 가 의미 있음.

## Verification

- `cargo test --lib commands::meta_agent::identity_trigger`:
  - `count_mod3` fail: done_count=1 → should_run=false
  - `count_mod3` fail: done_count=2 → should_run=false
  - `count_mod3` OK but `volume_min` fail: done_count=3, eligible=5 → should_run=false
  - All OK: done_count=3, eligible=10 → should_run=true
  - `volume_min` threshold 설정 변경 반영: env var / DB 값
- Integration: `complete_plan` 호출 후 trigger 훅 발화 mock 으로 캡쳐
- Force mode: `trigger_identity_analysis_now(force=true)` 에서 threshold 조건 bypass 확인
- `cargo test --lib commands::agent_jobs` — 새 kind='identity_analysis' enqueue 성공
- `npx vitest run src/components/tunaflow/settings/IdentityAnalysisSettings.test.tsx`

## Risks

- **threshold 초기값 적정성**: 10 이 적절한지 실측 불가. 초기 3~6개월 후 평균 분석 횟수 / 분석 결과 품질 rating 수집해 조정 (Open question Q-1).
- **app_settings 테이블 부재**: 현재 없을 수 있음. 우선 env var + 상수 default. 후속 plan 에서 kv 테이블 도입 시 이식.
- **Spawn blocking in complete_plan**: plan 완료 경로에서 async 훅을 spawn. panic 하면 plan 완료 로그에는 영향 없으나 silent fail. `eprintln!` / trace 로 감지 가능하게.
- **race condition**: 2 플랜이 동시 complete 하면 trigger 훅이 같은 순간 2 번 → analysis job 2 개 enqueue 가능. `agent_jobs.dedupe_key='identity-analysis-{project}-{period_start}'` 로 보호 (이미 subtask-04 범위의 기본 장치 활용).
- **metaAgent plan 미구현 시**: 본 subtask 가 metaAgent 의 first concrete use case 로 기능하나, metaAgent plan 의 governance / authorization 구조가 없으면 "metaAgent 가 자기 판단으로 실행" 이 사용자 통제 밖으로 느껴질 수 있음. Settings 토글 (`identity_analysis.enabled` default=true) 로 사용자가 끌 수 있게.
