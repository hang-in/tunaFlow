---
title: User Worldview Injection — Identity/Interface/Continuity 3축 번들 (E1 통합)
status: planned
priority: P1
created_at: 2026-04-22
related:
  - src-tauri/src/commands/agents_helpers/send_common/prompt_assembly.rs     # ContextPack identity 섹션
  - src-tauri/src/commands/jobs.rs                                           # agent_jobs 확장 대상
  - src-tauri/src/commands/project_tools.rs                                  # rawq background pattern 참조
  - src/lib/toolRequestHandler.ts                                            # tool-request follow-up 파이프라인
  - src/lib/insightOrchestration.ts                                          # Insight stash 경로
  - docs/plans/designReviewGatePlan.md                                       # 유사 Architect↔reviewer gate 패턴
  - docs/plans/metaAgentPlan.md                                              # P0 메타에이전트와 교차
  - docs/plans/harnessVerificationGapPlan.md                                 # §5 proposer 규약
triggered_by:
  - 2026-04-22 세션 사용자 철학적 프롬프트 (Identity/Interface/Continuity 3질문)
  - Gemini 답변 ("거부권", "자기 관찰 도구" 두 insight 기여)
  - 검토 세션 피드백 (rule-first stance check, event+snapshot first, low-priority visible background)
---

# User Worldview Injection — E1 통합 번들

> 에이전트가 사용자의 실존적 의도(세계관, Vibe 변천, 업 karma)와 동기화되어 "말은 있으되 달리지 못하는" 상태를 벗어나게 한다. 세 층을 한 plan 에 묶는 이유: 각각 독립이 아니라 상호 강화 (worldview 없이는 stance-conflict 판정 불가, preference_timeline 없이는 conflict 대조 불가, background insight job 없이는 proactive 제안 불가).

> **적용 범위**: 본 plan 의 모든 로직 (worldview 주입 / preference_timeline / stance-conflict / background insight job) 은 **sdk-session 경로 (Branch chat) 한정**. RT (`-p` one-shot) 는 매 turn full ContextPack 을 재주입하는 것이 정상 동작이며 본 plan 대상 아님. INV-1 이하의 모든 invariant, ContextPack 주입 변경, tool-request 핸들링은 `claude_sdk_session::spawn_session` 경로에만 영향을 준다. RT 경로 (`roundtable_helpers/*`, `agents/claude.rs::run_one_shot`) 는 본 plan 이 건드리지 않는다.

---

## TL;DR for Developer

1. **`user_worldview.md` 주입** — `~/.tunaflow/user_worldview.md` 사용자 철학 stance. ContextPack 조립 시 **identity 섹션보다 앞에** 삽입 (`prompt_assembly.rs`). 존재하지 않으면 기본 placeholder. 사용자가 Settings 에서 편집.
2. **`preference_events` + `preference_snapshots` 테이블 신설** (migration v46). vector-first 아님 — event-log + snapshot 2단 구조. Embedding 은 별도 선택 subtask 로 분리 (본 plan 범위 밖).
3. **Stance-conflict 감지는 rule-first** — 현재 요청을 recent preference_snapshots (최근 3건) 와 대조. Rule precheck 가 결정적 (conflict/no-conflict/ambiguous) 이면 모델 호출 스킵. Ambiguous 한정 Haiku/Flash 로 verify → 결과 compact 요약 후 Opus 프롬프트에 주입.
4. **Silent tool-request 금지** — 대신 `agent_jobs` 에 `priority`, `dedupe_key`, `visibility` 컬럼 + `kind='insight_background'` 추가. 본 plan 의 Subtask 04 는 **enqueue API + UI status surface + pending cancel 까지만** (Codex round-1/2 review 반영 — scope 축소). Worker 실행 경로 / trace_log 통합 / insight stash 연결은 별도 plan (metaAgent 착수 시점) 위임.
5. **Stance-conflict confirmation modal** — agent 가 `<!-- tunaflow:stance-conflict:<memory_name>:<field>:<rationale> -->` 마커 fire 시 사용자 modal. 두 composite key 는 `preference_snapshots` PK 와 정합. 선택지: (a) 의도 변경 확정 → 새 변곡점 기록, (b) 기존 선호 유지 → 요청 수정, (c) 무시 → agent 가 그대로 진행 (timeline 미기록).

구현 순서: 01 (worldview) → 02 (preference_events/snapshots) → 03 (stance-conflict rule+modal) → 04 (background insight job). 01 은 독립 ROI 최고. 02~03 은 의존 체인. 04 는 01~03 없이도 가능하나 "지루함 감지 후 제안" 플로우는 01 필요.

**하지 말 것**:
- Vector embedding 을 preference_timeline 에 **기본** 도입 (Gemini 초안의 과잉)
- Silent 동작 — 사용자 모르게 cost/자원 소비 금지 (tunaFlow 원칙 위배)
- Stance-conflict 를 Opus inline 으로 매번 호출 (비용 폭증)
- Agent 거부권을 "도덕적 판단" 으로 구현 — 기계적 rule 매칭 + 사용자 confirmation 으로 충분

---

## Specification

### 1. `user_worldview.md` 주입

**파일 위치**: `~/.tunaflow/user_worldview.md` (global) + `<project>/.tunaflow/user_worldview.md` override 허용 (후자가 있으면 우선).

**기본 템플릿** (Settings UI 에서 "기본 문구 로드" 버튼):
```markdown
# User Worldview

## Ontology
(사용자가 기본으로 채워 넣을 철학 stance. 예시는 tunaFlow 가 제공하지 않음 — user-authored.)

## Engagement preference
- 대화 bandwidth 가 자연스럽게 저하될 때 agent 의 대응 방식
- 과거의 업(業)과 모순되는 요청에 대한 agent 의 응답 원칙
```

**주입 경로**: `prompt_assembly.rs::assemble_prompt()` 가 ContextPack 을 조립할 때, **identity_fragment 보다 먼저** `worldview_fragment` 를 삽입. 우선순위:

```
[project]            ← 기존
[platform]           ← 기존
[agent-role]         ← 기존
...
[worldview]          ← 신규, identity 바로 앞
[identity]           ← 기존
[skills]
[recent_context]
...
[user_prompt]
```

**토큰 상한**: worldview 는 최대 500 tokens. 초과 시 앞부터 자르지 않고 사용자에게 "worldview 너무 깁니다 (Settings 에서 편집 권장)" toast + 자동 truncate.

**토글**: Settings 에 "Worldview 주입 활성화" 체크박스. 기본 ON. 끄면 fragment 완전 생략.

### 2. `preference_events` + `preference_snapshots` 스키마 (migration v46)

초안 수정 (검토 세션 피드백 반영): event-log + snapshot 2단 구조. Embedding 은 **별도 후속 subtask/plan** 으로 분리.

```sql
-- 매 변곡점 (stance 전환 사건) 을 append-only 기록
CREATE TABLE preference_events (
    id              TEXT PRIMARY KEY,
    memory_name     TEXT NOT NULL,       -- 예: "engine_preference"
    field           TEXT NOT NULL,       -- 예: "cli_vs_sdk"
    stance_from     TEXT,                -- 직전 값 (없으면 NULL = 최초 선호)
    stance_to       TEXT NOT NULL,       -- 새 값
    reason_text     TEXT,                -- 자유 서술 (사용자 또는 agent 요약)
    reason_tags     TEXT,                -- JSON array ["cost", "reliability", ...]
    confidence      REAL DEFAULT 1.0,    -- agent 자동 감지 시 낮음
    source          TEXT NOT NULL,       -- "user" | "agent_inferred"
    changed_at      INTEGER NOT NULL     -- epoch ms
);
CREATE INDEX idx_preference_events_field ON preference_events(memory_name, field, changed_at DESC);

-- 현재 활성 stance 의 caching. events 에서 파생되지만 session resume 시 빠른 load 용.
CREATE TABLE preference_snapshots (
    memory_name     TEXT NOT NULL,
    field           TEXT NOT NULL,
    current_stance  TEXT NOT NULL,
    last_event_id   TEXT NOT NULL REFERENCES preference_events(id),
    updated_at      INTEGER NOT NULL,
    PRIMARY KEY (memory_name, field)
);
```

**Embedding 은 선택적**: 차후 필요 시 별도 subtask 로 `preference_embeddings` 테이블 추가. 본 plan 은 여기까지 다루지 않음. INV-3 로 강제.

**Write path**:
- 사용자가 Settings 에서 직접 변경 시 → event INSERT + snapshot UPSERT, `source='user'`, `confidence=1.0`
- Agent 가 대화에서 stance 변경 감지 시 (후속 subtask 04 의 background job 이 수행) → event INSERT, `source='agent_inferred'`, `confidence<1.0`. 이 경우 사용자에게 confirmation modal.

**Read path** (세션 resume):
- `SELECT memory_name, field, current_stance FROM preference_snapshots` — 전체 snapshot load. 작음.
- ContextPack 조립 시 최근 3 변곡점만 추가 컨텍스트: `SELECT ... FROM preference_events ORDER BY changed_at DESC LIMIT 3`.

### 3. Stance-conflict detection (rule-first)

**흐름**:

```
User request arrives
    ↓
[A] Rule precheck (Rust, no model)
    - Extract candidate preferences from request (regex / keyword / named entity)
    - Compare to current snapshots + recent 3 events
    - Output: { conflict: bool | "ambiguous", matched_events: [...] }
    ↓
  ┌─ conflict=false → skip, inject compact "no conflict" summary
  │
  ├─ conflict=true → emit stance-conflict marker, show modal
  │
  └─ conflict="ambiguous"
         ↓
     [B] Model verify (Haiku/Flash, short prompt)
         - Input: user request + ambiguous snapshot
         - Output: {conflict: bool, reason: string}
         ↓
       (위와 동일 분기)
    ↓
[C] Opus 메인 프롬프트에 compact result 주입
    - "No conflict" or "Conflict detected — user confirmed Y"
    - 500 tokens 이내 요약
```

**Rule precheck 구현 위치**: `src-tauri/src/commands/agents_helpers/send_common/stance_check.rs` (신규).

**Model verify**: 기존 `agents/claude.rs::run_one_shot` 패턴 재활용, `-p haiku` 또는 Gemini Flash. Timeout 10s, 실패 시 fallback = **`Unknown`** 상태 (Codex round-1 review 반영). UI 는 Unknown 을 modal 이 아닌 **inline warning (soft-confirm)** 으로 렌더. modal 발생은 **결정적 Conflict 에만** 국한해 사용자 피로도 폭증 방지.

**Confirmation modal 마커** (composite key PK 와 정합, 하이픈 안전 non-greedy 파싱):

```html
<!-- tunaflow:stance-conflict:<memory_name>:<field>:<short_rationale> -->
```

`<memory_name>` 과 `<field>` 는 `preference_snapshots` 의 composite primary key. Modal 이 `get_preference_snapshot(memory_name, field)` Tauri command 호출로 단일 snapshot 조회. `<short_rationale>` 은 하이픈/개행 포함 가능.

**규약**: `memory_name` 과 `field` 값은 **`:` 문자를 포함하지 않는다**. marker parser 구분자 충돌 방지. Subtask 02 의 `validate_pref_identifier` 가 write 시점에 강제.

Modal 선택지:
- **의도 변경 확정** → 신규 preference_event INSERT (`source='user'`), snapshot UPSERT, agent 이 원래 요청을 그대로 수행
- **기존 선호 유지** → agent 에게 "사용자가 기존 선호 유지를 선택함. 요청을 재해석하라" 재주입 후 새 turn
- **무시** → timeline 미기록, agent 가 그대로 진행 (원 요청 수행). modal 재표시 방지를 위해 해당 conflict 는 이 session 동안 mute.

### 4. Low-priority background insight job (agent_jobs 확장)

**agent_jobs 테이블 확장** (migration v46 에 포함):

```sql
ALTER TABLE agent_jobs ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
    -- 0 = foreground (현재 default), -1 = low-priority background
ALTER TABLE agent_jobs ADD COLUMN dedupe_key TEXT;
    -- 같은 key 의 pending job 있으면 신규 INSERT 생략
ALTER TABLE agent_jobs ADD COLUMN visibility TEXT NOT NULL DEFAULT 'visible';
    -- 'visible' | 'hidden' (향후 확장). 본 plan 은 'visible' 만 사용.
CREATE INDEX idx_agent_jobs_queue ON agent_jobs(priority, status, updated_at);
```

**Subtask 04 scope** (Codex round-1/2 review 2026-04-23 반영 — 축소):

본 plan 의 Subtask 04 는 **스켈레톤만** 제공:
- `enqueue_job` helper (priority / dedupe_key / visibility 수용)
- `cancel_background_job` Tauri command — pending 보장, running best-effort (status 플래그만)
- `count_pending_background_jobs` Tauri command
- Frontend StatusBar 컴포넌트 (polling + event listener — worker 가 emit 시 표시)

**제공하지 않음** (별도 plan, metaAgent 착수 시점):
- Worker loop 실제 실행 경로 (`execute_job`, polling scheduler)
- trace_log 기록 통합 (기존 `insert_trace_log_with_context` 재사용 여부 후속 결정)
- Insight stash 연결 (`insightOrchestration` 파이프라인 통합)
- Cooperative cancel token / subprocess kill
- 실제 trigger 측 (metaAgent 가 enqueue 호출)

**본 plan 머지 직후 동작**: `enqueue_job(priority=-1, ...)` 호출 시 row 가 `status='pending'` 으로 머물고, 실행될 executor 는 없음. StatusBar 는 "N pending" 표시 + 사용자 cancel 가능. 후속 plan (metaAgentPlan 의 subtask 또는 별도 worker executor plan) 이 worker 를 얹어야 실행. 이는 축소된 INV-4 정의 (pending 보장 / running best-effort) 와 정합.

---

## Invariants

- **[INV-1]** ContextPack 조립 시 `user_worldview` fragment 는 반드시 `identity_fragment` 보다 **앞** 에 위치한다. 순서 역전은 agent 가 "역할 (identity)" 을 먼저 정의하고 사용자 OS (worldview) 를 context 로 취급하게 만들어 본 plan 의 전제를 무효화. **검증**: `prompt_assembly.rs::assemble_prompt` 단위 테스트 — 주어진 ContextPackMeta 의 sections 순서 assert.

- **[INV-2]** Stance-conflict 감지는 **rule precheck 가 결정적** 인 경우 모델을 호출하지 않는다. Model verify 는 `conflict=ambiguous` 분기에서만 발화. **이유**: 매 turn 모델 호출 시 비용 폭증 + latency 지연. **검증**: `stance_check.rs::tests` — clear conflict 케이스 (현재 snapshot 과 정반대 키워드) 와 clear no-conflict 케이스 (무관한 요청) 에서 model 호출 mock 이 0 회 호출되는지 assert.

- **[INV-3]** `preference_timeline` 관련 테이블은 본 plan 에서 `preference_events` + `preference_snapshots` **2개만** 도입한다. Embedding 관련 테이블 (`preference_embeddings`, vector index 등) 은 **별도 후속 plan** 으로 분리된다. **이유**: 기존 vector 층 (bge-m3, sqlite-vec) 과 경쟁하면 retrieval 우선순위가 혼란. 필요 증명된 이후에 추가. **검증**: migration v46 DDL grep — `embedding` / `vec0` 키워드 부재 확인.

- **[INV-4]** 모든 `priority < 0` background job 은 (a) UI 에 진행/대기 상태 노출 (visibility), (b) **pending 상태의 cancel 은 보장**, running 상태의 cancel 은 best-effort (status 플래그만 변경, subprocess kill 은 후속) 를 만족해야 한다. Silent 실행 금지. **이유**: tunaFlow 원칙 "숨은 동작 금지, trace 투명". 본 plan 의 Subtask 04 는 enqueue + UI surface + pending cancel 까지만 커버 (Codex round-1/2 review 2026-04-23 반영). Worker 실행, `trace_log` 기록, `background_insight_progress` 이벤트 emit 은 후속 plan 이 추가할 때 본 INV 를 확장 준수해야 한다. **검증**: Integration test — `enqueue_job` 으로 pending row 생성 → `cancel_background_job` → `status='cancelled'` 확인 + 이후 pending 쿼리에서 제외.

- **[INV-5]** Stance-conflict confirmation modal 의 "무시" 선택은 **timeline 에 어떠한 event 도 기록하지 않는다**. 사용자의 침묵을 stance 변경으로 해석하지 않음. **이유**: 업(業) 의 기록은 사용자 명시 승인 없이 누적되면 "내 선호가 내 모르게 바뀌었다" 라는 karma 오염. **검증**: UI 테스트 — modal 에서 "무시" 클릭 후 `SELECT COUNT(*) FROM preference_events WHERE id = ?new_event_id` 가 0.

- **[INV-6]** Agent 응답에 stance-conflict 마커가 있을 때 UI 는 그 마커를 **사용자에게 렌더하지 않는다** (HTML comment 형식 유지). Modal 로만 노출. **이유**: 마커가 raw text 로 보이면 사용자 혼란 + 신뢰 하락. **검증**: react-markdown 렌더 테스트 — 마커가 final DOM 에 없음을 확인.

---

## Rationale (reviewer-only)

### 검토 세션 피드백 반영

초안 (2026-04-22 Gemini 답 + 내 블루프린트 합산) 에서 다음 3 가지가 검토 세션에 의해 교정됨. 본 plan 은 교정본:

| 초안 | 교정 이유 | 본 plan |
|---|---|---|
| stance-conflict 를 Opus inline 으로 매 turn 체크 | 비용 폭증 | rule-first + ambiguous 에만 small model verify (§3) |
| `preference_timeline` 에 reason vectorization 기본 포함 | tunaFlow 이미 vector 층 있음. 새 memory source 추가 시 retrieval 경쟁 | event+snapshot 2단, embedding 은 선택적 후속 (INV-3) |
| Silent tool-request 로 부재중 stash | tunaFlow "trace 투명" 원칙 위배 | background + low-priority + visible + cancelable 로 재정의 (§4, INV-4) |

### Gemini 기여 (수용)

- **거부권 개념** → stance-conflict marker + modal 로 구현. "도덕적 거부" 가 아니라 "기계적 conflict detection + 사용자 confirmation" 로 기술 번역.
- **자기 관찰 도구 (Vipassana) 격상** → preference_events timeline 은 Insight 탭의 subview 로 노출 (별도 UI plan 에서 정리). 본 plan 은 backend 만.

### 대안 비교

| 대안 | 판정 | 사유 |
|---|---|---|
| 모든 turn Opus inline stance check | 기각 | 비용 |
| preference_timeline vector-first | 기각 | retrieval 경쟁 |
| Silent tool-request | 기각 | 원칙 위배 |
| 세 축을 3개 독립 plan 으로 분리 | 기각 | 서로 강화 관계. worldview 없이는 stance-conflict 판정 문맥 부재 |
| **채택** (3축 번들 + rule-first + event/snapshot + visible background) | ✅ | 최소 침습 + tunaFlow 원칙 정합 |

### Open questions

1. **Q-1 (worldview 템플릿 기본값)**: 사용자가 Settings 에서 "기본 문구 로드" 를 선택했을 때 제공할 minimal 템플릿 내용. 본 plan 은 "section header 와 빈 본문" 권장 (placeholder). 사용자 철학은 user-authored — tunaFlow 가 제시하는 순간 bias 주입. Developer/사용자 협의 후 확정.

2. **Q-2 (stance-conflict rule precheck 알고리즘)**: 초기 구현은 어떤 rule 패턴이 적절한가. 키워드 리스트? 간단한 regex? named entity (engine 이름 / 기능 이름) 추출? 본 plan 은 "간단한 토큰 매칭부터 시작 + 실제 oversight/undersight 측정 후 개선" 을 권장. 최초 릴리스는 **false positive 허용 (model verify 가 catch), false negative 회피** 튜닝.

3. **Q-3 (small verify model 선정)**: Haiku vs Gemini Flash vs 로컬 LMStudio? 본 plan 은 "기존 설정된 model" 우선 + Haiku default. Developer 구현 시 실측 latency 기반 결정.

4. **Q-4 (background worker polling vs event-driven)**: §4 의 worker 가 30 초 polling 으로 기술됐으나 event-driven 이 더 반응적. Polling 은 구현 단순, event-driven 은 복잡성 증가. 본 plan 은 MVP 로 polling 채택, 후속 refactor 여지 남김.

5. **Q-5 (metaAgent 와의 trigger 경로)**: 본 plan 은 background job 등록 **경로** 만 제공. 실제 "사용자 지루함 감지" trigger 는 `metaAgentPlan.md` (P0) 의 책임. metaAgent 구현 후 본 plan 의 `priority=-1` job 을 호출하는 식. 현재는 수동 테스트로 job 등록 경로 검증.

---

## Subtask 구조

| # | 파일 | 범위 | 의존 |
|---|---|---|---|
| 01 | [-task-01.md](./userWorldviewInjectionPlan-task-01.md) | `user_worldview.md` 파일 + ContextPack 주입 + Settings UI 편집기 | — |
| 02 | [-task-02.md](./userWorldviewInjectionPlan-task-02.md) | migration v46 (preference_events + preference_snapshots + agent_jobs 컬럼 확장) + write path helper | — |
| 03 | [-task-03.md](./userWorldviewInjectionPlan-task-03.md) | Stance-conflict detection (rule precheck + small model verify + confirmation modal) | 02 |
| 04 | [-task-04.md](./userWorldviewInjectionPlan-task-04.md) | Background insight worker (low-priority + visible + cancelable) | 02 |

4 subtask. 01 독립 (가장 작고 ROI 최고). 02 가 03/04 공통 의존. 03/04 는 02 후 독립 병렬.

---

## 관련 문서

- Gemini 답변 원문: 본 세션 2026-04-22 대화 (검토 세션 공유)
- 검토 세션 피드백: 본 세션 2026-04-22 대화 (rule-first, event+snapshot, visible background)
- 사용자 철학 원문: 본 세션 2026-04-22 "바이브 코더 / 2인 3각" 프롬프트
- 설계 유사 gate 패턴: `docs/plans/designReviewGatePlan.md`
- P0 metaAgent 교차: `docs/plans/metaAgentPlan.md`
