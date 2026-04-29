---
title: claude transport flip 후속 hardening — stale resume_token 자동 회복 + ContextPack revival + rate_limit 가시화
status: ready
phase: planning
priority: P0 (v0.1.5-beta release blocker)
created_at: 2026-04-29
canonical: true
supersedes: docs/plans/claudeRateLimitVisibilityPlan_2026-04-29.md  # 진단 history 보존, fix path 는 본 plan
related:
  - docs/plans/claudeResumeSessionTransitionPlan_2026-04-29.md  # v0.1.4-beta transport flip
  - docs/ideas/agentApiQuotaErrorUxIdea_2026-04-29.md
  - src-tauri/src/agents/claude.rs
  - src-tauri/src/commands/agents.rs
  - src-tauri/src/commands/agents_helpers/send_common/persistence.rs
  - src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs
  - src-tauri/src/commands/agents_helpers/send_common/context_loading.rs
trigger:
  reported_at: 2026-04-29
  reporter: 사용자 (d9ng)
  symptom: "tunaFlow 의 seCall 프로젝트 (오랫동안 미사용) 에서 claude send 시 400 'out of extra usage' — 같은 사용자/계정/시간대 mac architect / Windows 의 다른 프로젝트는 정상."
  diagnosis_history:
    - "5시간 rolling 한도 가설 — 빗나감 (사용자 console 25%)"
    - "Opus 별 한도 가설 — 빗나감 (모든 architect Opus 4.7 정상)"
    - "billing path mismatch (sdk-session vs cli) 가설 — 부분 사실 but root 아님"
    - "확정: stale resume_token + 시간 경과 + cli mode 의 ContextPack skip 정책 결합"
---

# claude transport flip 후속 hardening

## 0. Context

v0.1.4-beta hotfix (PR `4396aa6`) 에서 claude CLI 2.1.121 의 `--sdk-url` 정책 차단을 회복하기 위해 transport default 를 `cli` (`-p --resume`) 로 flip 했음. 의도된 회복이었으나 다음 부작용이 confirmed:

1. **stale resume_token**: `conversations.resume_token` 이 *과거 sdk-url 시점에 만들어진 session id* 또는 *시간 경과 (TTL 추정)* 한 token. cli mode 가 그 stale token 으로 `--resume` 시도 → Anthropic 측 거부 ("out of extra usage" 형태로 표시). 사용자 시각: "한동안 안 쓴 conversation 이 갑자기 거부 받음"
2. **ContextPack revival 부재**: 거부 후 fresh session 으로 자동 fallback 흐름이 없음. `is_session_continuation` skip 정책이 cli mode 가정 그대로 유지 → fallback 발생 시 Claude 측 history 0 + tunaFlow ContextPack 도 skip → "history 모두 잃은 응답"
3. **rate_limit_event 미가시화**: backend stream-json 에 `rate_limit_event` payload 가 들어오지만 frontend 에 노출 안 됨. 사용자 가 본인 한도 / overage 상태 / reset 시점 모름
4. **cli mode 의 session_freshness 미적용 (architectural)** — 2026-04-30 발견: `session_freshness.rs:14` 가 명시적으로 cli mode 를 "적용 제외 (항상 full)" 로 정의. 즉 cli mode 가 `--resume <id>` 로 Claude session 보존하면서도 *추가로* tunaFlow ContextPack 의 conversational layer (compressed-memory) 도 매 send 마다 inject → **double history**. 큰 history conversation 에서 paid API 영역 차감 trigger → "out of extra usage" 거부. 사용자 case: seCall main (1,285 messages, 766K chars) 에서 Auto 모드 거부 / Lite 모드 정상. 사용자 지적 "같은 세션이면 ContextPack 안 넣어야" 정확.

본 plan 은 넷을 묶어 v0.1.5-beta 의 release blocker 로 처리.

## 1. Invariants

| ID | 내용 |
|---|---|
| **INV-TFH-1** 🔴 | macOS 회귀 0. 모든 변경은 cfg 격리 또는 cross-platform 무영향 검증 |
| **INV-TFH-2** | fresh session 자동 fallback 시 사용자 가시화 의무. silent fallback 금지 (사용자가 history 누락 인지 못 함) |
| **INV-TFH-3** | DB migration 은 idempotent. 7일 stale token NULL 처리는 1회 한정 marker 로 보호 |
| **INV-TFH-4** | ContextPack budget 한도 (60K Standard) 안에서 fresh session 첫 호출 처리. budget 초과 시 추가 truncate 정책 (priority preserve 와 cross-link, idea A1) |
| **INV-TFH-5** | Anthropic billing path 자체 변경 시도 X — 사용자 환경의 정책 (org_level_disabled paygo 등) 은 안내만 |
| **INV-TFH-6** | (2026-04-30 추가) cli mode 의 session_freshness 적용 시 sdk-url path 동작 변경 X. 두 path 의 session key 형식 분리 보장 (충돌 0) |

## 2. 진단 timeline (history 보존)

| 단계 | 가설 | 결과 |
|---|---|---|
| D1 | 5시간 rolling 한도 도달 | 빗나감 — 사용자 console 25% 사용 |
| D2 | Opus 별 한도 영역 소진 | 빗나감 — 모든 mac architect Opus 4.7 정상 |
| D3 | sdk-session vs cli billing path mismatch | 부분 사실 — Windows = sdk-session OK / mac = cli 거부. 다만 root 는 아님 |
| D4 | 프로젝트 단위 변수 (seCall vs tunaReader) | seCall 거부 / tunaReader OK 확인 |
| **D5 (확정)** | seCall 의 *오래된 conversation 의 stale resume_token* + cli mode `--resume` 거부 | 사용자 fact 부합. 스크래치패드 (fresh session) 정상 = 결정적 증거 |

## 3. Goals / Non-goals

### Goals
- (G1) **stale resume_token 자동 detect + fresh session fallback** — 사용자 액션 0 자동 회복
- (G2) **fresh session 시 ContextPack revival** — `is_session_continuation` skip 자동 해제, full mode + anchor 2 turns 정책 발동 (이미 부분 구현, fallback 경로 연결만)
- (G3) **사용자 가시화** — fallback 발생 시 토스트, ContextPack revival 안내, RuntimeStatusBar 의 rate_limit indicator
- (G4) **DB migration** — 7일+ stale resume_token 일괄 NULL (1회, idempotent)
- (G5) **친화 에러 메시지** — backend 의 "out of extra usage" raw 노출 차단, 명시적 안내 모달
- (G6) README/INSTALL 안내 — v0.1.4-beta 업그레이드 사용자 대상
- (G7) **cli mode 의 session_freshness 적용 (architectural fix)** — 같은 session 의 두 번째 send 부터 compressed-memory skip → double history 차단 → paid API trigger 회피 (사용자 핵심 지적)

### Non-goals
- ❌ Anthropic billing 정책 변경 시도
- ❌ sdk-session 자동 부활 (별 plan, claude CLI 2.1.121 정책 변동 시점에)
- ❌ 다른 엔진 (codex / gemini / ollama / lmstudio) 의 stale token — claude 한정 (다른 엔진은 session resume 모델 다름)
- ❌ ContextPack 자체 효율화 (별 idea, bkit A1/A2)

## 4. Subtasks

### Task 01 — `rate_limit_event` parser + RunOutput 확장 [P0]

**Changed files**: `src-tauri/src/agents/claude.rs`

**Change description**:
- `StreamLine` enum 에 `RateLimitEvent` variant 추가 — Anthropic stream-json 의 `type: "rate_limit_event"` line 매칭
- 필드: `status / resets_at / rate_limit_type / overage_status / overage_disabled_reason / is_using_overage`
- `stream_run` reader 가 해당 line parse → 새 콜백 `on_rate_limit` 으로 frontend 전달
- `RunOutput` 에 `last_rate_limit: Option<RateLimitInfo>` 추가 (after-run 표시 용)

**Verification**:
- terminal 직접 호출 (`claude -p --output-format stream-json --verbose ...`) 결과의 rate_limit_event payload 와 parsed struct 일치
- unit test: rate_limit_event JSON → struct
- 기존 system / assistant / result event 회귀 0

**회귀 위험 가드**:
- 기존 `StreamLine` variant 변경 X
- 구버전 claude CLI (rate_limit_event 미전송) 는 graceful — 무시
- `on_progress` 콜백 signature 변경 X — 새 콜백 분리

### Task 02 — Stale resume_token detect + auto fallback [P0, 핵심]

**Changed files**:
- `src-tauri/src/agents/claude.rs` (result error 분기 + retry 흐름)
- `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` (fallback 경로의 session_freshness 갱신)

**Change description**:
- `claude.rs:result.is_error: true` 분기에서 다음 keyword 매칭으로 stale resume_token detect:
  - `"out of extra usage"` (사용자 보고 패턴)
  - `"invalid_request_error"` + `--resume <id>` 동반
  - `404` `"session not found"` (혹시 있다면)
- detect 시 한 번 retry — `--resume` 인자 제거 + 같은 prompt + 같은 system_prompt
- retry 도 fail 이면 raw error 그대로 return (다른 원인)
- retry 성공 시:
  1. DB 의 해당 conversation 의 `resume_token = NULL`
  2. 새 session_id 를 result event 에서 받아 새 resume_token 으로 update
  3. `session_freshness` 의 `LAST_DELIVERED` 키 clear → 다음 send 가 `is_session_continuation = false` 로 인식 (이미 구현된 분기 활용)
  4. frontend 에 fallback 이벤트 emit (`session:fresh_fallback`)

**Verification**:
- 의도적 stale token 시뮬레이션: DB 의 resume_token 을 random invalid 값으로 set 후 send → retry without `--resume` → 정상 응답 + DB resume_token 갱신
- retry 실패 시 raw error 그대로 (정상 인증 실패 등) → 회귀 0
- mac/Linux baseline cargo test --lib 동일

**회귀 위험 가드**:
- retry 는 1회 한정 (무한 loop 방지)
- detect keyword false positive 위험 — 정상 4xx 인증 실패가 retry 트리거되지 않게 keyword 정확히 (`--resume` 동반 + specific keyword)
- 다른 엔진 (codex/gemini/...) 영향 0 — claude.rs 한정 변경

### Task 03 — Fresh session ContextPack revival 자동 발동 [P0, 사용자 핵심 지적]

**Changed files**:
- `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` (session_freshness 분기 활용)
- `src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs` (필요 시 새 helper)

**Change description**:
- Task 02 의 fallback 경로에서 `session_freshness::clear_delivered_key(conv_id)` 호출 → 다음 send 시점에 `is_session_continuation = false` → 이미 구현된 "full mode + anchor 2 turns" 분기 자동 발동 (이번 retry 자체에는 fresh session 이지만 ContextPack 은 이미 assembled 상태라 다음 send 부터 효과)
- **단**: retry 자체의 prompt 도 ContextPack revival 하려면 retry 시점에 ContextPack 재assemble 필요. 옵션 A (간단): retry 는 same prompt 그대로 (Claude 가 fresh session 이라 첫 응답은 history 0). 옵션 B (정확): retry 직전 ContextPack 재assemble 후 `is_session_continuation: false` 로 추가 layer inject
- 권장: 옵션 B — 사용자 가시 응답 품질 보장. ContextPack assemble 함수 (`assemble_prompt`) 를 retry path 에서 한 번 더 호출
- budget 초과 시 priority preserve (idea A1 cross-link, 본 plan 에서는 기존 budget 정책 그대로)

**Verification**:
- stale token retry 시나리오에서 backend log 의 `[session_freshness]` 가 retry 시점에 `new session` 으로 표시되는지
- retry 응답에 이전 conversation history 가 반영됐는지 (간단한 multi-turn 대화로 검증)
- mac/Linux baseline 동일

**회귀 위험 가드**:
- ContextPack 재assemble 비용 (DB query / rawq / docs read) 한 번 더 → retry latency ↑. 그러나 fallback 자체가 rare 라 acceptable
- `is_session_continuation` 분기 자체 변경 금지 (이미 정상 동작 중) — Task 02 의 LAST_DELIVERED clear 만으로 자동 발동
- retry 실패 시 ContextPack 재assemble 결과 버려도 상태 오염 0 — pure function 가정 유지

### Task 04 — UI 가시화 (토스트 + RuntimeStatusBar indicator) [P0]

**Changed files**:
- `src/stores/streamStore.ts` (`session:fresh_fallback` 이벤트 + rate_limit state)
- `src/components/tunaflow/RuntimeStatusBar.tsx` (rate_limit indicator)
- `src/components/tunaflow/...` (토스트 또는 inline 메시지)
- `src/locales/{ko,en}/runtime.json`

**Change description**:
- fresh fallback 이벤트 수신 시 토스트 1회 (세션당 conversation 별 dismiss flag):
  > "Claude 세션이 만료되어 새로 시작합니다. 이전 대화 내용은 ContextPack 으로 복구 중 — 첫 응답이 약간 느릴 수 있습니다."
- RuntimeStatusBar rate_limit indicator (Task 01 의 `last_rate_limit` 활용):
  - 정상: 초록 dot
  - overage rejected: 노란 dot + "5시간 한도 사용 중, overage 비활성화"
  - blocked: 빨간 dot + reset 카운트다운
- 클릭 시 detail 팝오버 (`claude.ai/settings/usage` 링크 포함)

**Verification**:
- dev 모드 manual smoke — 위 시나리오
- macOS / Windows / Linux 동일 표시
- `npx tsc --noEmit && npx vitest run`

**회귀 위험 가드**:
- 토스트 spam 차단 — conversation 별 sessionStorage flag (재시작 시 reset)
- RuntimeStatusBar 의 다른 indicator 영향 0
- streamStore 의 다른 state 영향 0

### Task 05 — DB migration v49: 7일+ stale resume_token NULL [P1]

**Changed files**:
- `src-tauri/src/db/migrations.rs` (v49 idempotent migration)
- `src-tauri/src/db/schema.rs`

**Change description**:
- v49 marker 추가
- migration: `UPDATE conversations SET resume_token = NULL WHERE resume_token IS NOT NULL AND resume_token_engine IN ('claude', 'claude-code') AND id IN (SELECT conversation_id FROM messages GROUP BY conversation_id HAVING MAX(timestamp) < unixepoch() - 7*86400)`
- 즉 마지막 메시지가 7일 이상 지난 conversation 만 영향. 활성 사용 conversation 은 보존
- idempotent — v49 marker 있으면 skip
- migration 후 1회 console log: `[migration v49] cleared N stale resume_tokens (>7 days idle)`

**Verification**:
- 단위 테스트: 7일 미만 / 7일+ 두 conversation 으로 SQL fixture → 7일+ 만 NULL
- migration 두 번 실행해도 idempotent (skip)
- `cd src-tauri && cargo test --lib db::migrations`

**회귀 위험 가드**:
- 다른 엔진 (codex / gemini) 의 resume_token 영향 0 — `IN ('claude', 'claude-code')` 명시
- messages 테이블의 timestamp 컬럼 schema 확인 후 SQL 정확화 (혹시 `created_at` 등 다른 이름)
- migration 실패 시 graceful (다음 실행 시 retry) — Tauri 가 panic 으로 처리하지 않게 try/catch

### Task 06 — "Claude 세션 재시작" 메뉴 노출 강화 [P1]

**Changed files**:
- `src/components/tunaflow/...` (conversation 우클릭 메뉴 또는 RuntimeStatusBar 의 claude 영역)
- `src/lib/api/agents.ts` (이미 있는 `restart_sdk_session` invoke wrapper)

**Change description**:
- 현재 `restart_sdk_session` 명령 이미 backend (`agents.rs:113`) + frontend wrapper 있음 (검증 후 위치 확정)
- conversation 우클릭 메뉴 또는 RuntimeStatusBar 의 claude engine 카드에 "Claude 세션 재시작" 버튼 추가
- 클릭 시 `restart_sdk_session` invoke + 토스트 ("세션 재시작 완료, 다음 send 가 fresh session")
- Task 02 의 자동 fallback 와 별개의 *수동 trigger* — 사용자 가시 control

**Verification**:
- dev 모드 manual smoke — 메뉴 클릭 → DB resume_token NULL 확인 → 다음 send 가 fresh session 으로 시작

**회귀 위험 가드**:
- 기존 `restart_sdk_session` 동작 변경 X — UI 노출만
- 다른 메뉴 항목 영향 0

### Task 07 — 친화 에러 메시지 (Layer 1+2 of agentApiQuotaErrorUxIdea) [P1]

**Changed files**:
- `src-tauri/src/agents/claude.rs` (result error 분기, Task 02 와 같은 영역)
- frontend error 모달

**Change description**:
- Task 02 의 stale resume_token detect 외에 일반 4xx 분류 (idea A1 의 ApiErrorKind enum):
  - `QuotaExceeded` (true 사용량 한도, 5h/weekly)
  - `RateLimited` (429)
  - `AuthFailure` (401)
  - `ModelUnavailable`
  - `Unknown` (fallback raw)
- 분류된 에러를 frontend dedicated 모달로 — claude.ai/settings/usage 링크 + 다른 엔진 전환 dropdown
- Task 02 의 `RateLimitExceeded` (정확히는 stale resume 다른 의미) 와 분리

**Verification**:
- unit test: 각 4xx 응답 → 정확한 ApiErrorKind 분류
- frontend 모달 manual smoke (각 case)

**회귀 위험 가드**:
- 정상 응답 분기 영향 0
- 다른 엔진 (codex/gemini/...) 영향 0 — claude 한정

### Task 08 — README / INSTALL / CHANGELOG 안내 [P1]

**Changed files**: `README.md`, `README.ko.md`, `INSTALL.md`, `CHANGELOG.md`

**Change description**:
- README "Known Constraints" 또는 새 섹션 "Anthropic billing 안내":
  - tunaFlow 가 `claude -p` headless mode 사용 — Pro/Max plan 의 5시간 rolling 한도 + overage 정책 동일 적용
  - 한동안 미사용 conversation 의 resume_token 이 stale 일 수 있음 — v0.1.4-beta 이후 첫 send 시 자동 fallback (Task 02). 수동 재시작은 우클릭 메뉴 (Task 06)
  - claude.ai/settings/usage 에서 한도 / overage / "extra usage" 옵션 확인 권장
- CHANGELOG `[0.1.5-beta]` 새 섹션 (또는 `[0.1.4-beta]` 후속 항목으로 묶음, cadence 결정 필요)

**Verification**:
- 문구 검토 — Anthropic 정책 부정확 표현 없는지

**회귀 위험 가드**:
- 다른 README/INSTALL 섹션 변경 X

### Task 09 — cli mode session_freshness 적용 (architectural) [P0, 2026-04-30 추가]

**Trigger fact (사용자 보고 + mac architect 검증)**:
- 사용자 seCall main (1,285 messages, 766K chars) Auto 모드 거부, Lite 모드 정상
- mac architect terminal 직접 호출 (큰 prompt 55K chars 포함) **모두 정상** — 즉 paid API trigger 의 결정적 요인은 *prompt 자체* 가 아니라 *cli mode 의 double history (Claude session + tunaFlow ContextPack 의 compressed-memory 동시 inject)*
- 사용자 지적: "같은 세션이면 ContextPack 안 넣어야 한다" — 정확한 architectural insight

**Changed files**:
- `src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs` (적용 제외 정책 변경)
- `src-tauri/src/commands/agents.rs` (cli 분기에서 session key 등록)
- `src-tauri/src/agents/claude.rs` (stream_run 종료 시 session_id promote_pending_to_delivered 호출)

**Change description**:
- `session_freshness.rs:14` 의 "적용 제외" 목록에서 **claude `-p` cli 모드 제거**
- cli mode 의 session key 정의: `claude-code:{resume_token}` 또는 `claude-code:{conversation_id}` (resume_token 없을 때 fallback)
- `agents.rs:286-298` 의 cli 분기에서 `session_freshness::stash_pending(msg_id, key)` 호출 (sdk-url path 와 동일 패턴)
- `claude.rs:stream_run` 정상 종료 후 (또는 finalize_engine_run 안) `promote_pending_to_delivered` 호출 → LAST_DELIVERED_KEY 갱신
- 다음 send 가 같은 session 이면 `is_session_continuation = true` → `persistence.rs:280` 의 `drop recent_context + compressed_memory` 분기 자동 발동 → minimal mode

**T2/T3 와의 결합** (사용자 핵심 지적):
- 첫 send: resume_token NULL = fresh session. ContextPack full mode (anchor 2 turns + structured 만, compressed-memory 없음). 응답 정상.
- 첫 send 의 응답에서 새 session_id 받음 → `LAST_DELIVERED_KEY` 등록.
- 두 번째 send 부터: `is_session_continuation = true` → minimal mode. paid API trigger 회피.
- 만약 첫 send 에서 stale resume_token detect 되면: T2 retry 발동 → resume_token 제거 + retry → fresh session 으로 새 session_id 받음 → 같은 흐름.

**보조 가드 (T9-b)**: 첫 send (fresh session) 의 ContextPack 도 *compressed-memory 미inject* 정책 — anchor 2 turns + structured (plan/artifacts) + 사용자 prompt 로 충분. 즉 cli mode 는 *항상* compressed-memory 미inject. (sdk-url path 의 fresh session full mode 와 다른 정책 — cli mode 는 paid API trigger 회피 우선).

**Verification**:
- `cd src-tauri && cargo check && cargo test --lib agents_helpers::send_common::session_freshness`
- dev 모드 manual smoke (사용자 환경 한정 가능):
  - seCall main 채팅 Auto 모드에서 첫 send → 정상 (compressed-memory 미inject)
  - 두 번째 send → backend log `[memory_policy] skipped=[context:skipped(session-continuation), retrieval:skipped(session-continuation), compressed-memory:skipped(session-continuation)]` 표시 (sdk-url path 와 동일 패턴)
  - 응답 품질 history 반영 (Claude session 자체가 보유)
- baseline 회귀 0 (FE 381 / Rust 564+ 또는 다음 cycle 시점 baseline)

**회귀 위험 가드**:
- sdk-url path 의 session_freshness 동작 변경 X (해당 path 는 이미 정상)
- 다른 엔진 (codex/gemini/ollama/lmstudio) 의 session_freshness 정책 변경 X
- 첫 send 의 응답 품질이 sdk-url path 의 fresh session 보다 약간 낮음 (anchor 2 turns 만) — sdk-url 의 full mode 와 다른 정책. 사용자 가시화 토스트 또는 README 안내 권장
- LAST_DELIVERED_KEY 의 key 형식이 cli/sdk-url 충돌 없는지 검증 (예: `claude-code:` prefix 분리)
- T2 의 retry 흐름과 T9 의 session_freshness 등록 충돌 없는지 검증 (retry 후에도 정상 promote_pending_to_delivered 호출)

**Phase 분류**: P0, **Phase 1** (자동 회복 핵심) 으로 격상. T1~T4 와 같은 우선순위.

## 5. Cross-cutting risks

| 위험 | 대응 |
|---|---|
| **(T9 신규) cli mode session_freshness 의 session key 충돌** — sdk-url path 의 LAST_DELIVERED_KEY 와 cli mode 의 key 가 같은 conv_id 에 다른 형식으로 등록 | session key 형식 분리 (`claude-code:cli:{resume_token}` vs `claude-code:sdk:{sdk_session_id}`). conv_id 별 history 일관성 검증 unit test |
| **(T9 신규) 첫 send 응답 품질 저하** — cli mode 의 fresh session 도 compressed-memory 미inject 정책 → 응답 품질 sdk-url path 보다 낮음 | 사용자 가시화 토스트 + Settings 에서 사용자 override 가능 (기본 OFF — paid API 회피 우선) |
| stale token detect keyword false positive | Task 02 의 keyword 를 *`--resume <id>` 동반 + specific message* 조합으로 좁힘 |
| retry 자동화 → 무한 loop | retry 1회 한정 (count flag) |
| ContextPack 재assemble 비용 ↑ → fallback 응답 latency | rare event 라 acceptable. UI 토스트로 사용자 가시 |
| migration v49 가 활성 conversation 영향 | 7일 cutoff + messages.timestamp 기준. 사용자 활성 사용 = 무관 |
| Windows 환경 (sdk-session path) 영향 | claude.rs 의 cli mode 분기 안에서만 동작. sdk-session 호출 시 적용 안 됨 — 회귀 0 |
| Anthropic 정책 변경으로 keyword 형식 변화 | graceful — 매칭 안 되면 raw error 그대로 (Task 07 의 Unknown fallback) |

## 6. Rollback

각 task 분리 commit. Task 02 (auto fallback) 단독 revert 시 Task 03/04 의 자동 발동 path 사라짐 → 사용자가 수동 (Task 06) 으로 회복 가능.

migration v49 (Task 05) revert 는 별 migration 으로 처리 — 한 번 NULL 된 token 은 복구 불가능 (Anthropic session id 자체 만료라 의미 없음). 즉 Task 05 는 destructive but graceful.

## 7. Phase + 적용 시점

**Phase 1 (P0, v0.1.5-beta release blocker)**: Task 01~04 + **Task 09 (cli session_freshness 적용)** — auto fallback + ContextPack revival + UI 가시화 + double history 차단. 외부 사용자 onboarding 좌절 차단의 핵심.

**Phase 2 (P1, v0.1.5-beta 묶음)**: Task 05~08 — migration + 메뉴 + 친화 에러 + 안내.

**Phase 3 (P2, v0.1.6-beta 후속)**: cost 가시화 (이전 plan 의 §Task 05 — RuntimeStatusBar 의 cost indicator). 본 plan scope 외.

## 8. baseline + 검증 카운트

- 본 plan 시작 시점 (main `cc5a79a` 또는 그 이후): FE 381 / Rust 564+
- 작업 후 동일 또는 +N (각 task 의 unit test 추가만큼). 감소 시 회귀.

## 9. Cross-link

- `agentApiQuotaErrorUxIdea_2026-04-29.md` — Layer 1 (error 분류) 의 claude 한정 구체화. Task 07 가 그 idea 의 Phase 1
- `bkitReferenceAdoptionIdea_2026-04-29.md` Idea A1 (Priority Preserve) — fresh session ContextPack 어셈블 시 budget 초과 가드. 별 plan 작성 시 본 plan 과 link
- `claudeResumeSessionTransitionPlan_2026-04-29.md` — v0.1.4-beta transport flip SSOT. 본 plan 이 그 후속

## 10. v0.1.5-beta release timing 영향

본 plan 은 v0.1.5-beta 의 핵심 가치 — Phase 1 (Task 01~04) 머지가 release publish 의 사실상 차단 사유. Windows hardening / W-CI-1 등 다른 트랙과 병렬 가능 (영역 다름).

CHANGELOG `[0.1.4-beta]` 안에 묶을지 `[0.1.5-beta]` 새 섹션 만들지는 release timing 결정 — 본 plan §Task 08 에서 확정.
