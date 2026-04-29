---
title: claude rate_limit_event 가시화 + ContextPack cost 효율화 + overage 안내
status: ready
phase: planning
priority: P0 (사용자 가시 회귀 + v0.1.5-beta release blocker)
created_at: 2026-04-29
canonical: true
related:
  - src-tauri/src/agents/claude.rs
  - src-tauri/src/commands/agents_helpers/send_common/context_loading.rs
  - src/components/tunaflow/RuntimeStatusBar.tsx
  - docs/ideas/agentApiQuotaErrorUxIdea_2026-04-29.md  # 본 plan 의 idea 모태
trigger:
  reported_at: 2026-04-29
  reporter: 사용자 (d9ng)
  symptom: |
    backend stderr "API Error: 400 ... You're out of extra usage" 가 채팅에 노출.
    그러나 사용자 main quota 충분, Claude Code interactive 동시 정상 동작.
  diagnosis: |
    terminal `claude -p --output-format stream-json --verbose
    --dangerously-skip-permissions --model claude-opus-4-7 "test"` 직접 호출 →
    rate_limit_event 응답에 **status: "allowed"** + 다음 informational 필드:
    - rateLimitType: "five_hour" (informational)
    - overageStatus: "rejected" (자동 paygo 차단)
    - overageDisabledReason: "org_level_disabled" (org 레벨 정책)
    사용자 console fact (5시간 25% / 주간 34% / Sonnet 0% / Design 0%) 와 비교 시
    실제 한도 도달 X. 진짜 원인은 **모델별 별도 quota (Opus 한정 한도) 또는
    org_level_disabled 로 paygo fallback 거부** 의 둘 중 하나로 좁혀짐.
---

# Claude rate_limit_event 가시화 + ContextPack cost 효율화

## 0. Problem

**사용자 인지와 실제 차단 원인의 괴리**:

| 사용자 영역 | 사용자 console 표시 | 실제 한도/사용 |
|---|---|---|
| 5시간 rolling | 25% 사용 | 한도 도달 한참 전 |
| 주간 (모든 모델 통합) | 34% 사용 | 마진 충분 |
| Sonnet 별 한도 | 0% | 미사용 |
| Claude Design | 0% | 미사용 |
| **Opus 별 한도** | console 미표시 추정 | **별도 영역, 가능성 높음** |
| paygo extra usage | console 미표시 | **org_level_disabled — 자동 결제 차단** |

진짜 원인 가설 (사용자 보고 + 진단 결과 부합):
- **(α) Opus 별 quota 영역 소진** — Sonnet 0% / Design 0% 와 별개. Opus 가 더 좁은 별도 한도 가능성 높음. tunaFlow 가 Opus 4.7 default 사용 → 누적 소진
- **(β) paygo fallback 거부** — 단일 호출이 일반 quota 한도 초과 시 paygo 자동 결제 시도 → `org_level_disabled` 로 거부
- (γ) minute/hour rate limit — 5시간과 다른 단기 limit (가능성 낮음)

**tunaFlow 측 영향**:
- 단일 send 의 cost ≈ **$0.139 USD** (input 6 + cache_creation 20896 + cache_read 16181 + output 29 = ~37K tokens)
- ContextPack 이 매번 ~36K input tokens — 5시간 안 ~50회 호출 시 ~$7 / 한도 도달 가능
- raw error message ("out of extra usage") 가 사용자에게 main quota 부족으로 잘못 인식

**Claude Code interactive vs tunaFlow `-p` headless 차이 (검증 결과)**:
- `claude --version` = 2.1.120 (사용자 다운그레이드, sdk-url 정책 회피)
- `claude -p "..."` 단순 호출 = 정상
- `claude -p --output-format stream-json --verbose --dangerously-skip-permissions --model claude-opus-4-7` (tunaFlow argv) = 정상 (mac architect 환경)
- 동일 머신 / 계정에서 mac architect 가 OK 인 시점에 tunaFlow 만 차단 = **5시간 rolling 한도가 사용자 작업 누적 + tunaFlow 의 큰 ContextPack 으로 도달**

## 1. Invariants

| ID | 내용 |
|---|---|
| **INV-RL-1** | rate_limit_event payload 가 `result` event 와 별개로 stream-json 에 포함됨 — backend 가 별도 parse |
| **INV-RL-2** | 사용자 가시화 = 사용자 결정 영역. 본 plan 은 "정보 노출" 만, "결제 결정" 은 사용자 영역 |
| **INV-RL-3** | ContextPack 효율화는 회귀 위험 — 기존 응답 품질 baseline 유지 (외부 사용자가 "응답이 나빠졌다" 보고하지 않을 것) |
| **INV-1** | macOS / Windows / Linux 동일 동작 — claude CLI path 는 모든 OS 같음 |

## 2. Goals / Non-goals

### Goals
- (G1) **rate_limit_event 가시화** — backend 가 stream-json 의 rate_limit_event parse → frontend 에 5시간 한도 / overage 상태 / reset 시간 표시
- (G2) **사용자 친화 에러 메시지** — "out of extra usage" 거부 시 "5시간 한도 도달, claude.ai/settings/usage 에서 5시간 한도 / overage 상태 확인" 메시지
- (G3) **ContextPack cost 가시화** — RuntimeStatusBar 에 단일 send 예상 cost (cache_creation + cache_read 기준) 와 5시간 누적 cost 표시
- (G4) **README / INSTALL 에 overage 정책 안내 한 단락** — 사용자가 release 받기 전 5시간 한도 / overage 옵션 인지
- (G5) (선택) **ContextPack cache hit ratio 측정** — `cache_creation_input_tokens` 대비 `cache_read_input_tokens` 비율을 trace 에 기록 + RuntimeStatusBar 에 평균 표시

### Non-goals
- ❌ Anthropic billing 정책 자체 변경 시도 (불가능)
- ❌ overage 자동 결제 활성화 (사용자 영역, 안내만)
- ❌ ContextPack 자체 구조 변경 (compress / dedup) — 별 plan 영역 (idea A1/A2 와 cross-link)
- ❌ 다른 엔진 (Codex / Gemini) 의 rate limit — 별 plan
- ❌ Claude Code interactive 와의 billing path 통합 시도 (Anthropic 측 변화 필요)

## 3. Subtasks

### Task 01 — Backend rate_limit_event parser [P0]

**Changed files**:
- `src-tauri/src/agents/claude.rs` (StreamLine struct 확장 + parsing)

**Change description**:
- `StreamLine` struct 에 새 variant 추가:
  ```rust
  enum StreamLineKind {
      System, Assistant, Result,
      RateLimitEvent {
          status: String,           // "allowed" / "blocked"
          resets_at: Option<i64>,   // unix timestamp
          rate_limit_type: String,  // "five_hour" / "monthly"
          overage_status: String,   // "rejected" / "allowed"
          overage_disabled_reason: Option<String>,
          is_using_overage: bool,
      },
  }
  ```
- `serde::Deserialize` 로 `type: "rate_limit_event"` 분기 매핑
- `stream_run` 의 reader loop 에서 해당 line 을 parse → `on_progress` 콜백 또는 새 콜백 (`on_rate_limit`) 으로 frontend 전달
- `RunOutput` 에도 마지막 rate_limit_event 정보 저장 (after-run 표시 용)

**Verification**:
- `claude -p --output-format stream-json --verbose --dangerously-skip-permissions "test"` 호출 → rate_limit_event line 정상 parse 확인
- 단위 테스트: rate_limit_event JSON 입력 → parsed struct 일치
- 기존 system / assistant / result event 회귀 0 (`cargo test --lib agents::claude` baseline)

**회귀 위험 가드**:
- `StreamLine` 의 다른 variant (system / assistant / result) 변경 X
- claude CLI 가 rate_limit_event 안 보내는 경우 (구버전) graceful degrade — 무시
- `on_progress` 콜백 signature 변경 시 기존 caller 영향. 새 콜백 추가 권장 (`on_rate_limit`).

### Task 02 — Frontend rate_limit indicator (RuntimeStatusBar) [P0]

**Changed files**:
- `src/stores/streamStore.ts` (rate_limit state 추가)
- `src/components/tunaflow/RuntimeStatusBar.tsx` (indicator 렌더링)
- `src/locales/{ko,en}/runtime.json` (라벨)

**Change description**:
- `streamStore` 에 `rateLimit: { status, resetsAt, rateLimitType, overageStatus, ... } | null` state
- backend 의 `on_rate_limit` event 가 store 갱신
- RuntimeStatusBar 에 indicator (현재 mode/cost indicator 옆):
  - 정상 (status="allowed", overageStatus="allowed"): 초록 dot
  - 경고 (status="allowed", overageStatus="rejected"): 노란 dot + "5시간 한도 사용 중, overage 비활성화됨"
  - 차단 (status="blocked"): 빨간 dot + reset 카운트다운 ("X분 후 reset")
- 클릭 시 detail 팝오버 — 5시간 한도 / overage 상태 / claude.ai/settings/usage 링크

**Verification**:
- dev 모드에서 send 1회 → indicator 정상 표시
- rate_limit_event 없이 응답 (구버전 CLI) → indicator 비표시 (graceful)
- `npx tsc --noEmit && npx vitest run`

**회귀 위험 가드**:
- RuntimeStatusBar 의 다른 indicator (mode / engine / project) 변경 X
- store 의 다른 state (streaming / messages) 영향 X
- macOS / Windows / Linux 동일 표시 (cfg 분기 X)

### Task 03 — 차단 시 사용자 친화 에러 메시지 [P0]

**Changed files**:
- `src-tauri/src/agents/claude.rs` (result error 분기)
- `src/components/tunaflow/...` (error 모달 또는 inline 메시지)

**Change description**:
- `claude.rs:325-327` 부근 `result.is_error: true` 분기 에서 result.message 의 keyword 매칭:
  - `"out of extra usage"` 또는 `"overageStatus": "rejected"` 패턴 → 새 `AppError` variant `RateLimitExceeded`
  - 기타 400 → 기존 raw error 유지
- frontend 의 streamStore 가 `RateLimitExceeded` 받으면 dedicated 모달:
  - 제목: "Claude API 5시간 한도 도달"
  - 본문: "5시간 rolling rate limit 초과. claude.ai/settings/usage 에서 한도 / overage 옵션 확인하시기 바랍니다."
  - 버튼: (1) `claude.ai/settings/usage` 링크 (2) "다른 엔진으로 전환" — 활성 엔진 dropdown
  - 자동 reset 시간 표시 (`resetsAt`)

**Verification**:
- 의도적 한도 도달 시뮬 어렵 (실제 한도 도달 대기 X) → unit test: AppError parsing 만 검증
- 외부 사용자 보고 또는 본 사용자 다음 한도 도달 시 manual smoke

**회귀 위험 가드**:
- 다른 400 에러 (정상 인증 실패 / 모델 미사용 등) 분기 영향 X
- 모달이 다른 경로 (cancel / 정상 종료) 와 interfere X

### Task 04 — README / INSTALL 에 overage 정책 안내 [P1]

**Changed files**:
- `README.md`, `README.ko.md`, `INSTALL.md`

**Change description**:
- "Known Constraints" 섹션 (또는 새 섹션 "Anthropic billing 정책") 추가:
  - tunaFlow 가 `claude -p` headless mode 사용 — Claude Code interactive 와 같은 5시간 rolling 한도 + 같은 overage 옵션 적용
  - main 월별 quota 와 별개 — Pro/Max 구독자도 5시간 한도 도달 가능
  - org-level overage disabled 인 사용자는 한도 도달 시 자동 차단 (수동 결제 불가)
  - 권장: claude.ai/settings/usage 에서 overage 옵션 확인 / 활성화
  - 한 단락 + 링크

**Verification**:
- 문구 검토 — Anthropic 정책 부정확 표현 없는지

**회귀 위험 가드**:
- 다른 README 섹션 변경 X

### Task 05 — (선택) ContextPack cost 가시화 [P2]

**Changed files**:
- `src-tauri/src/agents/claude.rs` (RunOutput 의 cost / tokens 필드 활용)
- `src/components/tunaflow/RuntimeStatusBar.tsx` (cost indicator)

**Change description**:
- 기존 `RunOutput.cost_usd` 와 `total_input_tokens / total_output_tokens` 활용
- `cache_creation_input_tokens` / `cache_read_input_tokens` 도 추가 parsing (`StreamUsage` 에 이미 있음)
- RuntimeStatusBar 에 indicator: 마지막 send 의 cost (`$0.139`) + 5시간 누적 cost (있으면)
- 5시간 누적은 sliding window — 최근 5시간 trace 합산
- 클릭 시 cost breakdown (input / output / cache_creation / cache_read 분리)

**Verification**:
- send 1회 후 indicator cost 표시 정상
- 누적 cost 가 sliding window 안 정확

**회귀 위험 가드**:
- 다른 RuntimeStatusBar indicator 변경 X
- DB / trace schema 변경 시 migration 필요 — 본 task 가 schema 변경하지 않으면 OK

## 4. Cross-cutting risks

| 위험 | 대응 |
|---|---|
| claude CLI 구버전이 rate_limit_event 미전송 | parser graceful — 없으면 indicator 비표시 |
| rate_limit_event payload 형식 변경 | 사용자 환경의 실제 payload 로 schema 확정 (Task 01 진단 단계). 신규 필드 추가는 backward-compat |
| frontend 모달이 다른 엔진 (Codex / Gemini) 에러와 conflict | 본 plan 은 claude 한정. 다른 엔진의 rate limit 은 별 plan |
| ContextPack 효율화 부재로 한도 빠른 재도달 | 본 plan 은 가시화만. 효율화는 별 idea (`bkitReferenceAdoptionIdea` Idea A1 priority preserve, A2 fingerprint dedup) cross-link |

## 5. Rollback

각 task 분리 commit. Task 01 단독 revert 시 Task 02/03 의 frontend 가 rate_limit 정보 못 받음 → indicator 자동 비표시 (graceful).

## 6. Phase + 적용 시점

**Phase 1 (P0, v0.1.5-beta release blocker)**: Task 01 + 02 + 03 — 가시화 + 친화 에러. 사용자 즉시 가치.

**Phase 2 (P1, v0.1.5-beta 묶음)**: Task 04 — README/INSTALL 안내.

**Phase 3 (P2, v0.1.6-beta 후속)**: Task 05 — cost 가시화.

## 7. Cross-link to existing ideas

- `agentApiQuotaErrorUxIdea_2026-04-29.md` — 본 plan 은 그 idea 의 Layer 1+2+3 의 *claude 한정 부분 구체화*. idea 의 4 Layer 패턴 그대로 활용.
- `bkitReferenceAdoptionIdea_2026-04-29.md` Idea A1 (Priority Preserve) / A2 (Fingerprint Dedup) — ContextPack 효율화로 한도 도달 빈도 감소. 별 plan 작성 시 본 plan 과 link.

## 8. v0.1.4-beta 와 본 plan 관계

- v0.1.4-beta = transport flip (`-p --resume`) — 본 plan 의 root cause 가 아님 (`claude -p` 자체가 5시간 한도 path). Claude Code interactive 와 무관하게 *모든* `claude -p` 호출은 같은 한도 영역
- 즉 v0.1.4-beta 가 *문제를 만든 것* 이 아니라 *기존부터 있던 한도 가시화* 가 부족한 상태였음 — 다만 transport flip 으로 사용자가 더 자주 한도 도달
- 본 plan 은 v0.1.5-beta 의 핵심 가치 — 사용자가 본 인지 (main quota 풍부) 와 실제 한도 (5시간 rolling) 의 괴리 차단

## 9. 진단 단계 (Task 01 시작 전 fact 확보)

### 9.1 검증된 fact

- terminal 직접 호출로 rate_limit_event payload 확인됨 (mac architect 환경)
- payload `status: "allowed"` + `rateLimitType / overageStatus / overageDisabledReason / isUsingOverage / resetsAt` 5 필드 모두 informational 정보
- claude CLI 2.1.120 에서 동작 확인. 다른 버전 호환성은 Task 01 진단 시 확인
- 사용자 console 사용량 fact: 5시간 25% / 주간 34% / Sonnet 0% / Design 0% — *실제 한도 도달 아님*

### 9.2 추가 검증 필요 (가설 (α)/(β) 확정 위해)

| 단계 | 명령 / 행동 | 분기 |
|---|---|---|
| **D1** | claude.ai/settings/usage 직접 방문 → "Opus" 별 한도 표시 / "extra usage" 영역 명시 확인 | Opus 별 한도 표시 + 그게 거의 소진 → (α) 확정. extra usage 영역 0 + console 표시 → (β) 확정 |
| **D2** | tunaFlow 에서 model = `claude-sonnet-4-6` 으로 변경 후 같은 호출 시도 | 정상 응답 → (α) 확정 (Opus 별 한도 영역 한정). 동일 차단 → (α) 빗나감, (β) 또는 다른 가설 |
| **D3** | 차단 발생 시점의 backend log 의 정확한 token 사용량 (input / cache_creation / cache_read) 캡처 | 단일 호출 token 이 일반 quota 단일 호출 한도 초과 → (β) 확정 |

(α)/(β) 모두 본 plan 의 가시화 + 친화 에러 메시지가 사용자 즉시 가치. fix path 는 다름:
- (α) → tunaFlow 의 model 자동 fallback (Opus 거부 시 Sonnet 으로) 또는 Settings 에서 model 변경 안내
- (β) → ContextPack 효율화 (Idea A1/A2 우선순위 ↑) + 사용자에게 paygo 활성화 권유

### 9.3 다른 사용자 환경 호환성

다른 사용자 환경 (Windows / Linux) 의 payload 형식 변동 가능성 매우 낮음 — Anthropic API 단의 응답이라 OS 무관.

### 9.4 사용자 즉시 회복 옵션 (D2 검증 전이라도)

- **단기**: tunaFlow Settings 에서 model 을 Sonnet 으로 변경 (사용자 fact "Sonnet 0%" 활용)
- **중기**: claude.ai/settings/usage 에서 paygo 옵션 (overage) 활성화 — org-level 정책 변경 가능 시
- **장기**: ContextPack cost 효율화 (별 plan + idea A1/A2 cross-link)
