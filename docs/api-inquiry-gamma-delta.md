# tunaFlow Desktop API 실사 응답 — Mobile γ/δ 전제 (2026-04-20)

> 배경: `tunaflow-mobile` 팀이 γ (Workflow/PlanDetail), δ (Meta + Branch) 플랜 착수 전에 데스크톱 HTTP API (port 19840) + WS (`/ws/events`) 계약 범위 확인 요청. 본 문서는 **실제 코드 전수 조사 결과** + 필요한 확장 제안.

**한 줄 결론**: 모바일이 요청한 endpoint 상당수가 **이미 구현**되어 있음 (특히 Branch adopt, Plan detail, Plan events, RT run). 남은 공백은 **스키마 범위 축소(응답 필드 축약)** + **Meta 인박스 HTTP 노출 부재** + **WS 이벤트 브릿지 좁음**. γ 작업은 거의 필드 추가, δ 작업은 Meta 쪽 HTTP 래퍼 5개 + WS 이벤트 확장이 핵심.

---

## 현재 라우팅 전수 (참조)

`src-tauri/src/http_api/mod.rs:157-204` 기준:

```
# State / Project
GET    /api/health
GET    /api/projects
POST   /api/projects
POST   /api/projects/{key}/documents/index
POST   /api/projects/{key}/documents/search
GET    /api/projects/{key}/documents/graph
GET    /api/projects/{key}/documents/orphans
GET    /api/projects/{key}/documents/status

# Conversation / Message
GET    /api/conversations?projectKey=X
POST   /api/conversations
GET    /api/conversations/{id}/messages
POST   /api/conversations/{id}/delete
POST   /api/conversations/{id}/send
GET    /api/conversations/{id}/memory/status
POST   /api/conversations/{id}/memory/compress
GET    /api/conversations/{id}/session-links
POST   /api/conversations/{id}/session-links/refresh
POST   /api/conversations/{id}/chunks/index
POST   /api/conversations/{id}/chunks/search
GET    /api/conversations/{id}/traces

# Branch
GET    /api/conversations/{id}/branches
POST   /api/branches
DELETE /api/branches/{id}
POST   /api/branches/{id}/archive
POST   /api/branches/{id}/adopt
POST   /api/branches/{id}/rename

# Plan / Artifact
GET    /api/plans?conversationId=X
GET    /api/plans/{id}
GET    /api/plans/{id}/events
POST   /api/plans/{id}/approve
POST   /api/plans/{id}/reject
GET    /api/artifacts?conversationId=X

# Agents / Roundtable
GET    /api/agents/status
POST   /api/roundtables/run
POST   /api/roundtables/{id}/cancel

# WebSocket
GET    /ws/events
```

Auth: 단일 Bearer 토큰 (`~/.tunaflow/api-token`), WS 는 헤더 또는 `?token=` 쿼리. `/api/health` 만 예외.

---

## A. 공통 전제

| # | 답 | 근거 |
|---|---|---|
| A1 | 버저닝 미도입. `/api/...` prefix만, `Accept` 무시. (d) 재정의 필요. 원하면 `/api/v1/` 도입 저비용 | `http_api/mod.rs:157` |
| A2 | 단일 Bearer 토큰, 스코프/롤 없음. `/api/health` skip. WS token 헤더 또는 `?token=`. (a) 이미 있음 | `auth.rs:23`, `ws.rs:26` |
| A3 | 대역폭 충분. γ 2~3개, δ Meta 3~5개 추가 이번 사이클 수용 가능. (b) 확장 가능 | — |
| A4 | `serde_json::json!` manual serialization → **필드 add-only 확장 자유**. (a) 이미 있음 | plans.rs, conversations.rs |
| A5 | Mock-first → 실 API 교체 전략 허용. 권장 | — |

---

## B. Plans / PlanDetail (γ)

**핵심**: `GET /api/plans` 와 `GET /api/plans/{id}` 둘 다 현재 `{id, conversationId, title, status, phase}` 5필드만 반환 (`plans.rs:24, 57`). **DB 스키마엔 더 많은 필드**: `description`, `expected_outcome`, `architect_engine`, `developer_engine`, `reviewer_engines`, `slug`, `revision`, `version_major`, `version_minor`, `implementation_branch_id`, `review_branch_id`, `updated_at`, `branch_id`.

| # | 답 | 조치 |
|---|---|---|
| B1 | **독립 필드**. Canonical: `PlanStatus = "draft"\|"active"\|"done"\|"abandoned"`, `PlanPhase = "drafting"\|"subtask_review"\|"approval"\|"implementation"\|"review"\|"done"\|"rework"`. 모바일이 제안한 `pending_approval/implementing/reviewing/rejected`는 없음 — 매핑 필요 | `src/types/index.ts:300-301`, `commands/plans.rs:436-440` |
| B2 | **혼합: (a) DB 에 있음 + (b) 응답 확장 필요**. `subtasks[]` 는 `plan_subtasks` join. `events[]` 는 이미 `GET /api/plans/{id}/events` 존재 | 기존 `GET /api/plans/{id}` 응답 확장 + `?include=subtasks,events` 쿼리 추가 |
| B3 | Subtask action: Tauri 커맨드 존재, **HTTP 없음**. (b) 확장 가능 | `POST /api/plans/{id}/subtasks/{sid}/status` 신규 |
| B4 | Revision 은 DB 에 bumped major/minor 로 기록, **별도 이력 리스트는 없음**. (d) "최신만 표시, 이력은 desktop" 정책 수용 권장 | `commands/plans.rs:94` |
| B5 | RT 결과는 messages 에 저장, plan 과 `implementation_branch_id/review_branch_id` 로 간접 연결. (a) 있음 + PlanDetail 에 RT 요약 파생 필드 추가 가능 (b) | `agents.rs:191` |
| B6 | conv 당 여러 plan 가능. `get_active_plan_phase` 로직 존재. `GET /api/conversations/{id}/active-plan` 신규 권장 (b) | `commands/plans.rs:228-240` |

### γ 권장 endpoint 계획

1. `GET /api/plans/{id}` 응답 확장 (canonical 필드 전부, add-only)
2. `?include=subtasks,events` 쿼리 파라미터
3. `POST /api/plans/{id}/subtasks/{sid}/status` 신규
4. `GET /api/conversations/{id}/active-plan` 신규 (옵션)

---

## C. Meta 인박스 (δ)

**핵심**: `meta_notifications` DB 테이블 존재 (v38), Tauri 커맨드 6개 존재, **HTTP endpoint 0개**. 유일한 진짜 신규 작업 영역.

| # | 답 | 조치 |
|---|---|---|
| C1 | 현재 정의: "**(a) 사용자 팔로업 필요 알림**" 강조. `kind` 값 (`review_passed`, `review_failed`, `doom_loop_warning`, `doom_loop_escalated`, `architect_redesign_requested`, `plan_completed`, `plan_promoted`, `tool_request_failed`, `insight_detected`, `generic`) 코드에 정의됨 | `src/lib/metaNotifications.ts:15-25` |
| C2 | 읽음/dismiss/route 이동 Tauri 레벨 존재. `route_json` 필드 (tab/stage/planId/branchId/messageId). clear 는 soft delete. (b) HTTP 노출 필요 | `commands/meta_notifications.rs:100+` |
| C3 | DB 테이블 + 실시간 dispatch. 모바일은 초기엔 REST polling 으로 충분, 후속 WS subscribe. (b) 신규 endpoint + WS bridge | 아래 계획 |
| C4 | 페이지네이션 `limit` 지원 (default 50, cap 200), `project_key` 필터. 초기 "최근 30 · 프로젝트 전체" 허용. (b) | `commands/meta_notifications.rs:68` |
| C5 | Meta 와 Branch 분할 허용. Branch detail 이 desktop 완성도 높음. Meta 만 뒤로 미루고 Branch 먼저 가능. (a) | — |

### δ Meta 권장 신규 endpoint

```
GET    /api/meta-notifications?projectKey=X&limit=N
POST   /api/meta-notifications/{id}/read
POST   /api/meta-notifications/mark-all-read  (body: {projectKey})
POST   /api/meta-notifications/{id}/dismiss
POST   /api/meta-notifications/clear  (body: {projectKey})
```

Tauri 커맨드 래핑만 하면 됨. 예상 작업량: 1시간 미만.

---

## D. Branch 스크린 (δ)

**핵심**: 모바일 팀 질문 중 **대부분 이미 존재**. `api.branches()` 가 리스트만 쓰는 건 스키마가 얇아서임 (detail endpoint 부재).

| # | 답 | 조치 |
|---|---|---|
| D1 | **Branch detail endpoint 부재**. 리스트는 id/label/customLabel/status/checkpointId/mode/parentBranchId/createdAt 반환 (`conversations.rs:130-141`). `adopted_message_id` 는 저장 안 됨 (adopt 가 parent conv 에 insert 하지만 branch row 기록 없음). (b) 신설 + DB 컬럼 add 권장 | — |
| D2 | Rounds 는 **별도 테이블 없음**. "round" 는 system 헤더 메시지로 마킹 (`agents.rs:227`). 모바일 예상 구조는 (d) 재정의 필요: 서버 파생 뷰로 제공 | (b) `GET /api/branches/{id}/rounds` aggregate 제공 |
| D3 | Participants 는 `conversations.rt_config` JSON. branch 응답에 미포함. (b) 쉽게 추가 | — |
| D4 | **`POST /api/branches/{id}/adopt` 존재** (`conversations.rs:219-260`). body `{conversationId}`. 전체 병합만 지원, source_message_id 옵션 없음. (a) 있음, (b) 확장 가능 | `?source_message_id=...` 옵션 추가 |
| D5 | **`POST /api/branches` 존재** (`conversations.rs:153`). body `{conversationId, label?, mode?, checkpointId?}`. `participants/mode` 는 rt_config 세팅 미연결. (a) 기본, (b) 확장 | create_branch 에 participants 옵션 추가 |
| D6 | Branch-of-branch **공식 지원 안 함**. parent_branch_id 컬럼 있으나 UI/HTTP 경로 없음. `list_branches` 가 conv_id 필터라 shadow conv id 넘기면 sub-branches 반환. (d) 이번 사이클 "1-depth only" 선언 | — |

### δ Branch 권장 신규/확장

```
GET    /api/branches/{id}                      # 신규 (detail + rt_config + rounds aggregate)
GET    /api/branches/{id}/rounds               # 신규 (system header 파싱)
POST   /api/branches                           # 기존 (participants 옵션 확장)
POST   /api/branches/{id}/adopt                # 기존 (source_message_id 옵션 확장)
```

---

## E. WebSocket 이벤트

현재 bridge 하드코딩 리스트 (`ws.rs:50`):

```rust
["agent:completed", "agent:error", "roundtable:progress", "roundtable:participant_status"]
```

`send_message` / RT 핸들러가 직접 broadcast: `message:new`, `agent:completed`, `agent:error`, `roundtable:participant_status`.

| # | 답 | 조치 |
|---|---|---|
| E1 | 공식 목록 문서 없음. `bridge_tauri_events` 배열이 사실상 정의. 모바일 `WsEvent` 타입은 loose. (d) 재정의 필요 | 본 문서에 공식 목록 명시 (아래) |
| E2 | `plan.*` 전무, `branch.*` 전무, `meta.*` 전무. (b) 확장 가능 — 각 endpoint write 경로에 `event_tx.send(...)` 추가 | 아래 이벤트 추가 |
| E3 | at-most-once, replay 없음. `since` 커서 없음. (c) 이번 사이클 불가. 재연결 시 **REST full refresh** 로 복구 | — |

### WS 이벤트 공식 목록

이벤트 이름은 모두 `:` 구분자 (기존 `agent:completed` 관례와 동일). 페이로드는 `{type, ...rest}` flat 구조 (HTTP 핸들러 direct broadcast) 또는 `{type, payload}` 포장 구조 (Tauri→bridge 경유). 모바일은 `type` 기준으로 switch 하고, rest 는 shape 별로 unknown-safe 읽기.

```
# 현존
message:new                         { conversationId, messageId, role }
agent:completed                     { conversationId, messageId? }
agent:error                         { conversationId, messageId?, error }
roundtable:progress                 { message full object }
roundtable:participant_status       { conversationId, name, engine?, status }

# 신규 (이번 PR 에서 HTTP 핸들러 broadcast 추가, Tauri bridge 리스트 확장)
plan:created                        { planId, conversationId, phase }
plan:phase_changed                  { planId, toPhase, fromPhase? }
plan:status_changed                 { planId, toStatus, fromStatus? }
plan:subtask_status_changed         { planId, subtaskId, status }
branch:created                      { branchId, conversationId, mode }
branch:archived                     { branchId }
branch:adopted                      { branchId, summaryMessageId, conversationId }
meta:new                            { notificationId, projectKey, kind }
meta:read                           { notificationId }
meta:dismissed                      { notificationId }
```

**구현 범위 (이번 PR)**:
- HTTP 핸들러 직접 broadcast: `plan:status_changed`, `plan:phase_changed` (approve/reject), `branch:created/archived/adopted`, `meta:read/dismissed`
- Tauri bridge 리스트에 모든 이름 등록 (미구현 emit 도 향후 자동 bridge)
- 아직 **미 broadcast**: `plan:created`, `plan:subtask_status_changed`, `meta:new` — 해당 액션을 HTTP 로 할 수 있게 되면 추가 가능. 현재는 Tauri 전용 경로

---

## F. 스코프 의사결정

| # | 답 |
|---|---|
| F1 | **γ < δ 비용**. γ 는 필드 확장만. δ 는 Meta 5개 + Branch detail + WS 이벤트. 권장: γ → δ 순서, γ' 병렬 |
| F2 | **이번 사이클 불가**: B4 revision diff (desktop 도 없음), D6 branch-of-branch ("1-depth only"), E3 WS replay (별도 큰 작업) |
| F3 | 데스크톱 추가 작업량: 총 1~1.5일. γ 응답 확장 1~2시간, δ Meta + WS 반나절, Branch detail 반나절 |
| F4 | **BFF 미채택**. 단일 HTTP API 유지. 장기적 재검토 여지 |

---

## 즉시 착수 범위

이 문서 작성 직후 다음 3건을 동일 브랜치에서 진행:

1. **Plan detail 응답 확장**: `GET /api/plans/{id}` + `GET /api/plans` 에 canonical 필드 전부 노출. `?include=subtasks,events` 쿼리 지원
2. **Meta HTTP 래퍼 5개**: Tauri 커맨드 래핑으로 `/api/meta-notifications/*` 엔드포인트 추가
3. **WS broadcast 확장**: plan.* / branch.* / meta.* 이벤트 브릿지 추가

미적용 항목 (다음 사이클):
- Branch detail endpoint (`GET /api/branches/{id}` + `adopted_message_id` 컬럼 add)
- Rounds aggregate endpoint
- Subtask status HTTP action
- Active plan pointer endpoint
- API URL 버저닝 (`/api/v1/`)
