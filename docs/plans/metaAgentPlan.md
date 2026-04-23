---
title: 메타에이전트(Meta-agent) — 프로세스 관리자 + 정체성 분석 trigger
status: planned (Phase 0 부분 구현 완료, Phase 1-4 pending)
priority: P0
created_at: 2026-04-12
updated_at: 2026-04-23
related:
  - src/lib/metaConversation.ts                                           # 이미 존재 (Phase 0 부분 구현)
  - src/lib/defaultPersonas.ts                                            # persona_meta 등록됨
  - src-tauri/src/db/migrations.rs                                        # v33 migration 적용됨
  - src-tauri/src/commands/project_onboarding.rs                          # 온보딩 분석 경로
  - docs/plans/metaAgentInitialSetupPlan_2026-04-16.md                    # Agent Profile 추천 세부 (Phase 1-C 확장)
  - docs/plans/metaAgentOnboardingPlan_2026-04-16.md                      # 온보딩 UX (엔진/모델 선택) 세부
  - docs/plans/projectIdentityAnalysisPlan.md                             # 정체성 분석 파이프라인 (Phase 3 = metaAgent 의 trigger 책임)
  - docs/plans/userWorldviewInjectionPlan.md                              # Phase 4 = background insight job 이관처
triggered_by:
  - 2026-04-12 최초 설계 — 프로세스 관리자 + 이슈 감지 + 우선순위 제안
  - 2026-04-23 Karma/Identity 파이프라인 분리 — metaAgent 에 "정체성 분석 trigger" 책임 추가
  - 2026-04-23 userWorldview 축소 — "background insight job" 을 metaAgent 로 이관
absorbs:
  - userWorldviewInjectionPlan 의 subtask-04 (background insight job skeleton)
  - projectIdentityAnalysisPlan 의 subtask-02 (identity analysis trigger — metaAgent 측 구현)
---

# 메타에이전트 통합 플랜

> "제안하되 결정하지 않는다" — 승인 게이트는 항상 사용자.

---

## TL;DR for Developer

1. **metaAgent 는 프로세스 관리자 역할 고정** — Architect(설계) / Developer(구현) / Reviewer(검수) 와 구분. 기술 설계 / 코드 수정 / 파괴적 action 금지.
2. **Phase 0~2** 는 **기존 설계** (상태 분석, 이슈 감지, 우선순위 제안, 온보딩). 일부 구현 완료 (persona_meta, metaConversation.ts, v33 migration).
3. **Phase 3 신규** — `projectIdentityAnalysisPlan` 의 trigger 주체. `plans.status='done' % 3 == 0 AND eligible_artifacts >= 10` 조건 감시 → `identity_analysis` job enqueue → 분석 prompt assembly → `identity_summary` artifact 생성 요청.
4. **Phase 4 신규** — 일반 background insight job 경로 (userWorldview subtask-04 에서 이관). `agent_jobs.priority=-1` 로 low-priority worker pool.
5. **모든 자동 action 은 Settings 토글** 로 OFF 가능. 사용자 주권 최우선. INV-1 으로 강제.

구현 순서: Phase 0 완결 → Phase 1-A/B (tool-request + suggestion UI) → Phase 1-C (온보딩 훅) → Phase 2 (상태 분석 심화) → Phase 3 (identity trigger) → Phase 4 (bg job worker). Phase 3 은 `projectIdentityAnalysisPlan` subtask-01 머지 후 착수 권장.

**하지 말 것**:
- metaAgent 가 LLM 호출을 직접 실행 (prompt assembly + artifact creation 요청까지만. 실제 LLM 요약 생성은 dedicated analyzer persona 또는 기존 engine)
- 사용자 승인 없이 data 수정 / 삭제 / 전송
- Main conversation 에 자동 plan 주입 (plan-proposal 마커 직접 생성 금지)

---

## 개요

메타에이전트는 **프로세스 관리자**다. Architect(설계 실행자)와 역할이 다르다.

| 역할 | 판단 범위 |
|---|---|
| **메타에이전트** | 프로젝트 상태 분석, 이슈 감지, 우선순위 제안, 설정 최적화, **정체성 분석 trigger**, **background job 관리** |
| **아키텍트** | 기술 설계, Plan 분해, subtask 구성 |
| **사용자** | 모든 결정 — 메타에이전트는 제안만 |

### 적용 범위
tunaFlow 자체뿐 아니라 **사용자의 모든 프로젝트**에서 훌륭한 어시스턴트 역할.

---

## 핵심 기능

### 1. 온보딩 (새 프로젝트 추가 시 자동 트리거)
- 프로젝트 기술 스택 감지 (rawq/파일시스템 스캔)
- context-hub 소스 추천 (감지된 스택 기반)
- 추천 스킬셋 제안
- CLAUDE.md 초안 생성 제안
- **세부**: `docs/plans/metaAgentOnboardingPlan_2026-04-16.md` + `metaAgentInitialSetupPlan_2026-04-16.md`

### 2. 에러/이슈 모니터링 (온디맨드)
- `agent_jobs` 에러 스캔
- `trace_log` 이상 패턴 (context 과부하, 반복 실패)
- `failure_lessons` 반복 패턴 감지
- → `insight_findings`에 기록 → `meta-suggestion` 카드 → 사용자 승인 → Architect 전달

### 3. 프로젝트 상태 분석 (온디맨드)
- 밀린 Plan 목록, rework 비율
- 최근 세션 흐름 요약
- 다음 우선순위 제안

### 4. 설정 최적화 (제안)
- 엔진 추천 (태스크 유형별)
- 스킬셋 최적화
- context-hub 소스 갱신

### 5. 정체성 분석 trigger (신규 2026-04-23)
- `plans.status='done'` 카운트 감시
- 3의 배수 도달 + eligible artifacts ≥ 10 시 `identity_analysis` job 자동 enqueue
- 상세: **Phase 3**

### 6. Background insight job 관리 (신규 2026-04-23)
- `agent_jobs.priority=-1` 로 등록된 low-priority job 의 dispatch / cancel / status
- 상세: **Phase 4**

---

## 트리거 설계

| 트리거 | 시점 | 방식 |
|---|---|---|
| **온보딩** | 새 프로젝트 추가 직후 | 알림 배너 표시 → 사용자 클릭 시 Meta 대화 이동 |
| **온디맨드 이슈 분석** | 사용자가 명시적으로 호출 | 사이드바 Meta 고정 항목 클릭 |
| **정체성 분석 (Phase 3)** | Plan done % 3 == 0 AND eligible ≥ 10 | 자동 enqueue (Settings 토글로 OFF 가능) |
| **Background insight (Phase 4)** | 외부 모듈이 `enqueue_job(priority=-1, ...)` 호출 | metaAgent worker 가 poll 해 실행 |

백그라운드 자동 개입은 Phase 3/4 에 한정. 두 경로 모두 **Settings 에서 disable 가능** + 실행 내역은 `trace_log` 에 기록.

---

## Invariants

- **[INV-1]** metaAgent 는 **사용자 승인 없이 파괴적 action 을 수행하지 않는다**. 파괴적 action = 데이터 수정 / 삭제 / 외부 전송 / 파일 쓰기 / agent 실행. 예외: **(a)** `meta-suggestion` 카드 생성, **(b)** `insight_findings` INSERT, **(c)** 자동 trigger 에 의한 `agent_jobs` INSERT (analysis / bg insight) 는 "제안 + 기록" 범주라 허용. 모든 자동 trigger 는 Settings 에서 OFF 가능해야 한다. **이유**: "제안하되 결정하지 않는다" 원칙 + 사용자 주권. **검증**: metaAgent 경로 grep 으로 `UPDATE messages` / `DELETE FROM` / 외부 HTTP POST 호출 부재 확인. Settings 토글 OFF 시 Phase 3/4 의 자동 발화 없음을 integration test.

- **[INV-2]** metaAgent 는 `plan-proposal` 마커를 **직접 생성하지 않는다**. 설계가 필요한 제안은 Architect 에게 위임 (`<!-- tunaflow:meta-to-architect:TOPIC -->` 마커로 전달). **이유**: 역할 분리 — metaAgent 는 "무엇이 필요한가" 를 감지, Architect 는 "어떻게 구현할까" 를 설계. **검증**: `grep "tunaflow:plan-proposal" agents/meta.md` + promptFragment 결과 0건. metaAgent 응답 integration test — plan-proposal 마커 생성 시 fail.

- **[INV-3]** `meta-suggestion` / `identity_summary` / `background_insight_job` 자동 생성은 **프로젝트별 Settings 로 OFF 가능** 하다. Phase 3 의 identity trigger 와 Phase 4 의 bg worker 는 사용자가 "끄고 싶다" 할 때 즉시 멈춰야 한다. **이유**: INV-1 의 실운영 보장. **검증**: Settings UI 에 `tunaflow.meta_agent.auto_identity_analysis` / `tunaflow.meta_agent.background_insight_enabled` 체크박스 존재 + 쿼리 시점에 값 확인.

- **[INV-4]** metaAgent 대화는 **프로젝트당 1개 싱글턴** (`conversations.type='meta'` + `projects.meta_conversation_id`). 중복 생성 방지를 위해 `Map<projectKey, Promise<string>>` 캐시 필요. **이유**: metaAgent 가 여러 대화로 분산되면 상태 파편화 + 혼란. **검증**: `metaConversation.ts::getOrCreateMetaConversation` 동시 호출 시 동일 ID 반환 test.

- **[INV-5]** Phase 3 (identity trigger) 의 LLM 호출 주체는 **metaAgent 자신 또는 dedicated analyzer persona**. metaAgent 직접 실행 시 `persona_meta.promptFragment` 를 따르되 섹션 강제 프롬프트를 별도 system 메시지로 append. 분석 LLM 호출과 metaAgent 의 user-facing 대화가 **동일 conversation 에서 뒤섞이지 않도록** 별도 conversation 또는 job 단위 격리. **이유**: 사용자가 meta 대화를 볼 때 분석 중간 trace 가 쏟아지면 UX 저해. **검증**: identity_analysis job 실행 중 meta conversation 의 messages 에 LLM raw output 이 append 되지 않음을 integration test.

- **[INV-6]** Phase 4 (background insight worker) 는 **foreground job 진행 중에는 양보**. `agent_jobs WHERE priority=0 AND status='running'` 존재 시 다음 iteration 까지 대기. 또한 job 실행은 **반드시 `trace_log` 기록** + `background_insight_progress` 이벤트 emit + Settings 의 "진행 상태" UI 노출. Silent 실행 금지. **이유**: tunaFlow 원칙 "숨은 동작 금지, trace 투명". **검증**: foreground job mock 중 bg job pick 안 함 + trace_log row 증가 + event emit 검증.

---

## Phase 0 — 핵심 인프라 (부분 구현 완료)

### P0-1. DB 마이그레이션 (v33) ✅ **구현됨**

**파일**: `src-tauri/src/db/migrations.rs`

```sql
-- projects 테이블 확장
ALTER TABLE projects ADD COLUMN meta_conversation_id TEXT;
ALTER TABLE projects ADD COLUMN onboarding_done INTEGER DEFAULT 0;
```

`add_column_if_missing` 헬퍼로 멱등성 보장.

### P0-2. 프론트엔드 타입 확장 ✅ **구현됨**

**파일**: `src/types/index.ts`

```typescript
type: "main" | "branch" | "discussion" | "scratchpad" | "meta";
```

**연쇄 영향**: `scratchpads = conversations.filter(c => c.type === "scratchpad")` 패턴은 meta를 자연히 제외하므로 안전. `type !== "scratchpad"` 부정 패턴 grep 필요.

### P0-3. `metaConversation.ts` 유틸리티 ✅ **구현됨**

**파일**: `src/lib/metaConversation.ts`

```typescript
async function getOrCreateMetaConversation(projectKey: string): Promise<string>
```

Map<projectKey, Promise<string>> 캐시로 중복 생성 방지.

### P0-4. 사이드바 Meta 고정 항목 ⚠️ **구현 확인 필요**

**파일**: `src/components/tunaflow/Sidebar.tsx`

프로젝트 선택기 바로 아래, 섹션들 위에 `MetaNavItem` 삽입.

### P0-5. 메타에이전트 시스템 프롬프트 ❌ **미구현**

**파일 (신규)**: `agents/meta.md` (tunaFlow 레포 내 번들 에이전트)

현재 `agents/` 폴더에 `architect.md` / `code-reviewer.md` / `developer.md` 만 존재. `meta.md` 추가 필요.

핵심 내용:
- **역할 한계 명시**: "You propose only. Every suggestion requires user approval."
- **금지 사항**: "Do NOT produce plan-proposal markers directly. Delegate to Architect after user confirms."
- **tool-request 사용법**: `jobs`, `trace`, `plans`, `lessons`, `rawq` 활용 지침
- **출력 형식**: `<!-- tunaflow:meta-suggestion:TYPE -->` 마커 사용법
- **아키텍트 위임**: `<!-- tunaflow:meta-to-architect:TOPIC -->` 마커

### P0-6. 메타에이전트 페르소나 추가 ✅ **구현됨**

**파일**: `src/lib/defaultPersonas.ts`

`persona_meta` 등록됨.

---

## Phase 1-A — tool-request 확장

### P1-A-1. ToolRequest 타입 확장

**파일**: `src/lib/planProposalParser.ts`

```typescript
type: "docs" | "rawq" | "graph" | "plans" | "memory" | "sessions"
    | "skills" | "artifacts" | "lessons" | "jobs" | "trace";
```

`plans` 타입 query 값 확장: `"pending"` | `"done"` | `"all"`

### P1-A-2. toolRequestHandler — `jobs`, `trace` 핸들러 추가

**파일**: `src/lib/toolRequestHandler.ts`

`jobs` 타입:
- `invoke("list_failed_jobs", { projectKey, limit: 20 })` 호출
- 최근 7일 에러를 마크다운 테이블로 포맷

`trace` 타입:
- `invoke("get_trace_anomalies", { projectKey, limit: 30 })` 호출
- context_truncated 건수, 반복 실패 패턴 포맷

### P1-A-3. 백엔드 신규 커맨드

**파일**: `src-tauri/src/commands/jobs.rs`

```rust
#[tauri::command]
pub fn list_failed_jobs(project_key: String, limit: Option<i64>, ...) -> Result<Vec<AgentJob>, AppError>
```

**파일**: `src-tauri/src/commands/tracing.rs`

```rust
#[tauri::command]
pub fn get_trace_anomalies(project_key: String, limit: Option<i64>, ...) -> Result<Vec<TraceAnomaly>, AppError>
```

---

## Phase 1-B — meta-suggestion 마커 & UI

### P1-B-1. meta-suggestion 파서

**파일**: `src/lib/planProposalParser.ts`

```
<!-- tunaflow:meta-suggestion:TYPE -->
...content...
<!-- /tunaflow:meta-suggestion:TYPE -->
```

TYPE: `"onboarding"` | `"issue"` | `"priority"` | `"config"`

```typescript
export interface ParsedMetaSuggestion {
  suggestionType: "onboarding" | "issue" | "priority" | "config";
  title: string;
  description: string;
  severity?: "critical" | "high" | "medium" | "low";
  actionLabel?: string;
  architorTopic?: string;
  raw: string;
}
```

### P1-B-2. MetaSuggestionCard 컴포넌트

**파일 (신규)**: `src/components/tunaflow/message/MetaSuggestionCard.tsx`

UI 구성:
- 헤더: `Bot` 아이콘 + suggestion type 배지 (issue=red, priority=blue, onboarding=green, config=yellow)
- severity 배지
- title + description
- 액션: "승인 → Architect에 전달" + "무시"

승인 흐름:
1. `insight_findings`에 자동 기록
2. `architorTopic` 있으면 Architect 대화로 이동 + topic 전송

### P1-B-3. MessageItem 통합

**파일**: `src/components/tunaflow/MessageItem.tsx`

meta 대화에서만 MetaSuggestionCard 렌더.

---

## Phase 1-C — 온보딩 트리거

### P1-C-1. 프로젝트 추가 훅

**파일**: `src/stores/slices/projectSlice.ts`

`createProject` 완료 후:
1. `getOrCreateMetaConversation(projectKey)` 호출
2. 알림 배너: "Meta 에이전트가 프로젝트 분석을 시작했습니다"
3. 자동 이동 없음 — UX 충격 방지

### P1-C-2. `get_meta_context` 커맨드

**파일 (신규)**: `src-tauri/src/commands/meta_context.rs`

```rust
#[derive(Serialize)]
pub struct MetaContext {
    pub failed_jobs: Vec<AgentJobSummary>,
    pub trace_anomalies: Vec<TraceAnomaly>,
    pub pending_plans: Vec<PlanSummary>,
    pub rework_ratio: f64,
    pub recent_lessons: Vec<FailureLessonSummary>,
    pub open_findings: Vec<InsightFindingSummary>,
}
```

### P1-C-3. `list_context_hub_sources` 커맨드

**파일**: `src-tauri/src/commands/context_hub.rs`

context-hub 내부 API로 현재 등록 소스 목록 조회.

**세부 확장**: 온보딩 UX 는 `metaAgentOnboardingPlan_2026-04-16.md` + `metaAgentInitialSetupPlan_2026-04-16.md` 에 별도 상세. Agent Profile 추천 / Workflow 기본값 등.

---

## Phase 2 — 프로젝트 상태 분석 & 설정 최적화

P1 완료 후 메타에이전트가 대화를 통해 자연스럽게 수행. 대부분 **시스템 프롬프트 개선**과 **tool-request 활용**.

### P2-1. `agents/meta.md` 프롬프트 확장
- 온보딩 흐름 상세화 (rawq 스캔 → 스택 감지 → 소스 추천 → CLAUDE.md artifact)
- 상태 분석 흐름 (jobs → trace → plans → lessons 순차 조회)

### P2-2. `plans` tool-request 확장
- `"pending"` 쿼리: 진행 중 Plan + rework 비율 반환
- `"all"` 쿼리: 전체 요약

---

## Phase 3 — Identity analysis trigger (신규 2026-04-23)

> `projectIdentityAnalysisPlan` 의 **subtask-02** 에 해당. 본 phase 는 metaAgent 측 구현 책임.

### P3-1. Trigger 감시 로직

**파일 (신규)**: `src-tauri/src/commands/meta_agent/identity_trigger.rs`

```rust
pub fn evaluate_identity_trigger(
    conn: &Connection, project_key: &str,
) -> Result<IdentityTriggerDecision, AppError>
```

조건 (INV-2 반영):
- `done_count % 3 == 0` (`plans WHERE status='done' AND project_key=?`)
- 이전 `identity_summary` 이후 eligible artifacts ≥ threshold (default 10, Settings 로 튜닝)

전체 명세: `docs/plans/projectIdentityAnalysisPlan-task-02.md` 참조.

### P3-2. Trigger 훅 연결

`plans.rs::complete_plan` 말미 + PR 머지 훅 + 수동 "지금 분석" 버튼.

Fire-and-forget `tokio::task::spawn_blocking` — plan 완료 흐름 영향 없음.

### P3-3. Analysis job enqueue

`agent_jobs` 재사용:

```rust
pub fn enqueue_identity_analysis_job(
    app: &AppHandle, project_key: &str, decision: IdentityTriggerDecision,
) {
    let job_id = format!("id-analysis-{}-{}", project_key, now_epoch_ms());
    // dedupe_key = `identity-analysis-{project}-{period_start}` — INV-6 의 race 방어
    // kind = "identity_analysis"
    // priority = -1 (background)
    // 참조: userWorldview subtask-04 의 enqueue 패턴
}
```

실제 분석 실행 (prompt assembly + LLM 호출 + identity_summary 생성) 은 `projectIdentityAnalysisPlan` 의 subtask-03 에 명세. metaAgent 는 **job enqueue + kick-off 이벤트 emit** 까지.

### P3-4. Settings UI — `IdentityAnalysisSettings`

- `tunaflow.meta_agent.auto_identity_analysis` 토글 (default ON, INV-3)
- `tunaflow.identity_analysis.min_artifacts` threshold (default 10, 3~50 범위)
- "지금 분석 실행" + "강제 실행 (threshold 무시)" 버튼
- TriggerStatus 표시 — 현재 done_count / eligible_count / threshold / 마지막 분석 시각

세부 UI: `projectIdentityAnalysisPlan-task-02.md` §5.

---

## Phase 4 — Background insight worker (신규 2026-04-23)

> `userWorldviewInjectionPlan` 의 subtask-04 에서 이관. 스켈레톤만 제공된 상태에서 metaAgent 측으로 흡수 — executor 까지 완성.

### P4-1. `agent_jobs` 컬럼 확장 (migration v47 or later)

- `priority INTEGER DEFAULT 0` (0=foreground, -1=background)
- `dedupe_key TEXT`
- `visibility TEXT DEFAULT 'visible'`

**userWorldview subtask-04 의 스키마 그대로** 유지. migration 은 별도 plan 으로 (userWorldview subtask-04 가 archive 됐으므로 metaAgent subtask 로 재편).

### P4-2. `enqueue_job` helper + `cancel_background_job` command

userWorldview subtask-04 의 설계 그대로 재활용:
- `enqueue_job(conn, conv_id?, engine, kind, priority, dedupe_key?, visibility)` — dedupe 체크 후 INSERT
- `cancel_background_job(job_id)` — pending → cancelled 보장 / running → status 변경 best-effort
- `count_pending_background_jobs()` — UI polling 용

### P4-3. Background worker loop

**파일 (신규)**: `src-tauri/src/commands/meta_agent/background_worker.rs`

```rust
pub fn spawn_background_worker(app: AppHandle, state: Arc<AppState>) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;

            // INV-6: foreground busy 면 양보
            if has_foreground_running(&state) { continue; }

            // Pick 1 pending bg job
            let Some(job) = pick_background_job(&state) else { continue; };

            // mark running + emit
            mark_job_running(&state, &job.id);
            app.emit("background_insight_progress",
                json!({"jobId": job.id, "state": "started", "kind": job.kind})
            ).ok();
            record_trace_log_start(&state, &job);        // INV-6: trace 필수

            // dispatch by kind
            let result = match job.kind.as_str() {
                "identity_analysis" => run_identity_analysis_job(&app, &state, &job).await,
                "insight_background" => run_insight_background_job(&app, &state, &job).await,
                _ => Err(AppError::BadRequest(format!("unknown bg kind: {}", job.kind))),
            };

            let status = if result.is_ok() { "done" } else { "failed" };
            mark_job_status(&state, &job.id, status);
            record_trace_log_end(&state, &job, result.as_ref().err().map(|e| e.to_string()));
            app.emit("background_insight_progress",
                json!({"jobId": job.id, "state": status})
            ).ok();
        }
    });
}
```

**concurrency cap = 1** (loop 구조로 자연 보장).

### P4-4. FE StatusBar 표시

**파일**: `src/components/tunaflow/StatusBar.tsx` (또는 상태 표시 컴포넌트)

- pending 수 polling (15s) → "Background: N pending"
- 진행 중 job 표시 (kind + 취소 버튼)
- INV-6 의 "진행 상태 UI 노출" 요구 충족

userWorldview subtask-04 설계 그대로 구현.

### P4-5. Settings 토글

- `tunaflow.meta_agent.background_insight_enabled` (default ON, INV-3)
- OFF 시 worker loop 가 pick 하지 않음 (continue)

---

## 구현 방식 (요약)

- 새 엔진 타입 불필요 — **특별한 페르소나 + `conversations.type = "meta"` 대화**
- 기존 tool-request 시스템 활용
- 신규 tool-request 타입: `jobs` (agent_jobs 에러), `trace` (trace 이상)
- 출력 마커: `<!-- tunaflow:meta-suggestion:TYPE -->` → MetaSuggestionCard UI
- 프로젝트별 Meta 대화 싱글턴 (1 프로젝트 = 1 Meta 대화)
- **Phase 3/4 는 background daemon** — Tauri app 시작 시 worker loop spawn

---

## Rationale

### 왜 metaAgent 에 Phase 3/4 를 흡수하는가

**Phase 3 (identity trigger)**:
- `projectIdentityAnalysisPlan` 의 trigger 조건 (`plans done count + artifact volume`) 은 프로젝트 전반 상태 감시 성격. 이는 metaAgent 의 "상태 분석 / 우선순위 제안" 역할과 정합.
- 별도 에이전트로 분리하면 "누가 언제 도는가" 관리 비용 증가. metaAgent 가 이미 온디맨드 상태 분석을 수행하므로 자동 trigger 감시도 자연스러움.
- INV-1 의 "사용자 승인 없이 파괴적 action 금지" 는 유지 — identity analysis 는 새 artifact 생성까지만, 기존 데이터 수정 X.

**Phase 4 (bg insight worker)**:
- userWorldview subtask-04 가 "스켈레톤만" 으로 머지되면 dead feature. executor 까지 붙여야 의미.
- Background worker 는 단일 dispatcher 가 여러 job kind 를 처리하는 구조가 효율. 각 plan 마다 worker 를 따로 만들면 동시성 제어 / priority 관리가 파편화.
- metaAgent 는 이미 "프로세스 관리자" 포지션 — bg job 관리자 역할이 자연스럽다.

### 왜 단일 worker (concurrency=1) 인가

- tunaFlow 는 단일 사용자 desktop 앱. 동시 실행 이점 거의 없음.
- Foreground agent 와의 경합 방지 (INV-6) 가 단순 loop 로 충분.
- 향후 다중 worker 필요 시 확장 가능 (kind 별 pool).

### 기존 InitialSetup / Onboarding plan 과의 관계

- `metaAgentOnboardingPlan_2026-04-16.md` — Phase 1-C-1 온보딩 UX 의 상세 확장 (엔진 선택 / 모델 드롭다운). 독립 plan 으로 유지.
- `metaAgentInitialSetupPlan_2026-04-16.md` — Agent Profile 추천 / Skill 활성화 / Workflow 기본값. Phase 1-C 확장 + Phase 2 의 "설정 최적화" 와 겹침. 본 상위 plan 이 뼈대 + 세부 plan 2개가 살. 흡수하지 않고 crosslink 유지.

### 대안 비교

| 대안 | 판정 | 사유 |
|---|---|---|
| 정체성 분석을 별도 "analyst agent" 로 분리 | 기각 | metaAgent 가 이미 프로세스 관리자 — 중복 |
| Background worker 를 분산 (kind 별 단위) | 기각 | 단일 사용자 환경에서 overkill |
| metaAgent 가 파괴적 action 직접 수행 | 기각 (INV-1) | 사용자 주권 원칙 위배 |
| **채택** (metaAgent 가 Phase 3/4 흡수 + 단일 worker + INV 제약) | ✅ | 역할 일관성 + 최소 침습 |

### Open questions

1. **Q-1 Phase 3/4 머지 순서**: Phase 3 (identity trigger) 는 `projectIdentityAnalysisPlan` subtask-01 (artifact 자동 생성) 머지 후 착수 자연. Phase 4 (bg worker) 는 Phase 3 의 identity_analysis job 실행을 위해 선행 또는 동반 필요. 실제 구현 순서는?

2. **Q-2 metaAgent 의 LLM 엔진 선택**: Phase 3 의 identity_summary 생성 LLM 호출은 어느 엔진? `persona_meta.engine` 이 claude 기본이면 분석도 claude 로. 사용자가 토글할 수 있게? (비용 / 품질 trade-off)

3. **Q-3 analysis 중 메타 대화 UX**: identity_analysis job 실행 중 사용자가 meta 대화 열면 trace/progress 가 보이는가, 보이지 않고 job panel 에서만? INV-5 는 "뒤섞이지 않도록" 만 요구, 구체 UX 는 미정.

4. **Q-4 세부 plan 의 상태**: `metaAgentInitialSetupPlan` / `metaAgentOnboardingPlan` 이 언제 구현에 착수되는지 (본 상위 plan Phase 1-C 완료 시점). 세부 plan 의 subtask 분해가 필요한지.

5. **Q-5 CLAUDE.md 자동 갱신**: 온보딩에서 CLAUDE.md 초안 생성한다 했는데, 이후 세션별 업데이트는 metaAgent 가 자동 제안? 사용자가 명시 호출?

---

## 파일 변경 범위 요약

### 신규 생성 (Phase 0-2 만)
| 파일 | 규모 |
|------|------|
| `agents/meta.md` | ~120줄 |
| `src/lib/metaConversation.ts` | ~60줄 (구현됨) |
| `src/components/tunaflow/message/MetaSuggestionCard.tsx` | ~150줄 |
| `src-tauri/src/commands/meta_context.rs` | ~120줄 |

### 신규 생성 (Phase 3-4 추가)
| 파일 | 규모 |
|------|------|
| `src-tauri/src/commands/meta_agent/identity_trigger.rs` | ~80줄 |
| `src-tauri/src/commands/meta_agent/background_worker.rs` | ~150줄 |
| `src/components/tunaflow/settings/IdentityAnalysisSettings.tsx` | ~120줄 |

### 수정 (Phase 0-2)
| 파일 | 변경 내용 |
|------|-----------|
| `src/types/index.ts` | `"meta"` 타입 추가 (완료) |
| `src-tauri/src/db/migrations.rs` | v33 (완료), Phase 4 용 v47+ 추가 |
| `src/lib/planProposalParser.ts` | meta-suggestion 파서, ToolRequest 타입 확장 |
| `src/lib/toolRequestHandler.ts` | `jobs`, `trace` 핸들러, `plans` 확장 |
| `src/lib/defaultPersonas.ts` | `persona_meta` (완료) |
| `src/components/tunaflow/Sidebar.tsx` | MetaNavItem 삽입 |
| `src/components/tunaflow/MessageItem.tsx` | MetaSuggestionCard 통합 |
| `src/stores/slices/projectSlice.ts` | 온보딩 트리거 |

### 수정 (Phase 3-4)
| 파일 | 변경 내용 |
|------|-----------|
| `src-tauri/src/commands/plans.rs` | `complete_plan` 말미 identity trigger 훅 |
| `src-tauri/src/lib.rs` | background worker spawn at startup |
| `src/components/tunaflow/StatusBar.tsx` | background job progress 표시 |

---

## 구현 순서 (의존성 기반)

```
[기존 설계 — Phase 0-2]
P0-1 (DB migration) ✅
  └─ P0-2 (타입 확장) ✅
       └─ P0-3 (metaConversation.ts) ✅
            ├─ P0-4 (사이드바 Meta 항목) ⚠️
            └─ P0-6 (페르소나 추가) ✅
P0-5 (agents/meta.md) ❌ 독립

P1-A-3 (백엔드 신규 커맨드) ❌
  └─ P1-A-1/2 (toolRequestHandler 확장) ❌
       └─ P1-B-1 (파서 확장) ❌
            └─ P1-B-2 (MetaSuggestionCard) ❌
                 └─ P1-B-3 (MessageItem 통합) ❌

P1-C-2 (get_meta_context) ❌
  └─ P1-C-1 (온보딩 트리거) ❌

P1-C-3 (list_context_hub_sources) ❌ 독립

P2 — P1 전체 완료 후

[신규 2026-04-23 — Phase 3-4]
Phase 3 prerequisite:
  projectIdentityAnalysisPlan subtask-01 (artifact 자동 생성) 머지

Phase 3:
P3-1 (identity_trigger.rs)
  └─ P3-2 (complete_plan 훅)
       └─ P3-3 (analysis job enqueue)
            └─ P3-4 (Settings UI)

Phase 4:
P4-1 (v47 migration, agent_jobs 컬럼 확장)
  └─ P4-2 (enqueue_job / cancel_background_job)
       └─ P4-3 (background_worker.rs) ← Phase 3 의 identity_analysis job 실행 필요
            └─ P4-4 (StatusBar)
                 └─ P4-5 (Settings 토글)
```

Phase 3 과 Phase 4 는 상호 의존 — Phase 4 의 worker 가 Phase 3 의 `identity_analysis` kind 를 실행. 따라서 **Phase 4 먼저 구현** (worker skeleton) → **Phase 3 구현** (trigger + kind dispatch) 이 자연스러움.

---

## 리스크 & 주의사항

1. **Meta 대화 싱글턴 경쟁 조건**: `Map<projectKey, Promise<string>>` 캐시로 중복 생성 방지 (INV-4)
2. **`agents/meta.md` 경로**: 사용자 프로젝트에 없음 → P0는 `persona_meta.promptFragment`, 후속에서 번들 에이전트 경로 지원
3. **meta-suggestion 마커 오용**: `MessageItem`에서 `conversation.type === "meta"` 조건으로만 파싱
4. **온보딩 UX 충격**: 자동 이동 대신 알림 배너 방식 사용
5. **`type !== "scratchpad"` 부정 패턴**: meta 포함 여부 grep 검토 필요
6. **Phase 4 worker 의 startup 타이밍**: Tauri app ready 시점에 worker spawn. main thread 블로킹 안 되게 `tokio::spawn`
7. **Phase 3 trigger 빈도**: threshold 10 이 실사용에서 과소/과다인지 3~6개월 운영 후 재조정 (Open question Q-1 참조)
8. **LLM 비용**: Phase 3 의 identity_summary 생성이 월 수 회 반복. 사용자 구독 범위 내 설계 — 추가 과금 경로 최소화 (Token Policy 참조)

---

## 총 규모 추정

| 구분 | LOC |
|------|-----|
| Rust 백엔드 (Phase 0-2) | ~330 |
| Rust 백엔드 (Phase 3-4 추가) | ~250 |
| TypeScript 프론트엔드 (Phase 0-2) | ~500 |
| TypeScript 프론트엔드 (Phase 3-4 추가) | ~180 |
| 에이전트 프롬프트 | ~150 (meta.md + persona extension) |
| **합계** | **~1,410** |

Phase 0~2 만으로는 ~950 LOC. Phase 3~4 흡수로 +460 LOC.

---

## 관련 문서

- 상위 Plan: 본 문서 (metaAgent 전체)
- 온보딩 UX 세부: `metaAgentOnboardingPlan_2026-04-16.md`
- 초기 구성 자동화: `metaAgentInitialSetupPlan_2026-04-16.md`
- 정체성 분석 파이프라인: `projectIdentityAnalysisPlan.md` (Phase 3 bridge)
- Background job skeleton: `userWorldviewInjectionPlan-task-04.md` (superseded, 본 plan Phase 4 로 이관)
- Token Policy: `docs/reference/tokenPolicyReference.md`
- harness 규약: `docs/plans/harnessVerificationGapPlan.md`
