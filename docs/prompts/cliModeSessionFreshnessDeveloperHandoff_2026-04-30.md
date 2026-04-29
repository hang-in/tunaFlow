---
title: Developer 핸드오프 — T9 cli mode session_freshness 적용 (architectural fix, v0.1.5-beta release blocker)
plan: docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md  # §4 Task 09
created_at: 2026-04-30
priority: P0 (release blocker — Lite 모드 강제 회피)
---

# Developer 핸드오프 — cli mode session_freshness 적용 (T9)

## 0. 한 줄 요약

`session_freshness.rs:14` 의 cli mode "적용 제외" 정책 제거 + cli mode 도 sdk-url 처럼 session key 등록. **double history (Claude session + tunaFlow ContextPack 의 compressed-memory) 차단** → paid API trigger 회피. 사용자가 Lite 모드 강제로 떨어지지 않게 — release blocker fix.

## 1. SSOT

- **Plan §4 Task 09**: `docs/plans/claudeTransportFlipHardeningPlan_2026-04-29.md` (commit `060161b` 머지 완료, T9 추가됨)
- 핵심 코드 영역 (read-first):
  - `src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs` — 적용 제외 정책 + LAST_DELIVERED_KEY
  - `src-tauri/src/commands/agents_helpers/send_common/persistence.rs:270-286` — `is_session_continuation` 분기 (이미 정상 동작, 본 task 는 등록 측만 변경)
  - `src-tauri/src/commands/agents.rs:286-298` — cli 분기 (stash_pending 호출 추가 위치)
  - `src-tauri/src/agents/claude.rs:stream_run` — stream_run 정상 종료 후 promote 호출 위치
  - `src-tauri/src/agents/claude_sdk_session.rs` (read-only — sdk-url path 동작 변경 X 검증용)

## 2. 가이드라인 (절대 깨지 마세요)

### 사이드 이펙트 방지
- `session_freshness.rs:14` 의 적용 제외 목록에서 **cli 만 제거**. 다른 항목 (codex CLI exec / gemini / opencode / RT participants / Branch shadow first send) 그대로 유지.
- `claude_sdk_session.rs` 절대 변경 X — sdk-url path 의 session key 등록은 이미 정상.
- 다른 엔진 (codex / gemini / ollama / lmstudio / opencode) 의 session_freshness 정책 변경 X.
- session key 형식 충돌 차단: cli 와 sdk-url 의 key prefix 분리 (예: `claude-code:cli:{resume_token}` vs `claude-code:sdk:{sdk_session_id}`). 같은 conv_id 에 두 path 동시 사용 시 conflict 0.

### 기능 완료 후 테스트
- `cd src-tauri && cargo check --message-format=short`
- `cd src-tauri && cargo test --lib agents_helpers::send_common::session_freshness`
- `cd src-tauri && cargo test --lib` (전체 — baseline 동일 또는 +N. 감소 시 회귀)
- `npx tsc --noEmit` (frontend 영향 0 검증)
- baseline (현 main `060161b`): FE 381 / Rust 564+ (정확한 카운트는 PR 작성 시점에 측정)

### 자체 리뷰 (PR 전)
- `git show HEAD --stat` self-review
- 변경 파일 외 0 (`git diff main --name-only` 로 확인)
- DO NOT 위반 0 — sdk-url path / 다른 엔진 미변경 grep
- session key 형식 unit test 1개 추가 (cli 와 sdk-url 의 key 가 다른 형식임을 보장)

## 3. 작업 상세

### Task 09-a — `session_freshness.rs:14` 적용 제외 정책 변경

**Changed file**: `src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs`

**Change description**:
- 모듈 doc comment (line 13-18) 의 "적용 제외 (항상 full)" 목록에서 **cli mode 항목 제거**:
  ```diff
  - 적용 제외 (항상 full):
  - - claude `--sdk-url` (claude_sdk_session::SESSIONS)
  - - codex app-server (codex_app_server::CONV_THREADS)
  -
  - 적용 제외 (항상 full):
  - - claude `-p` CLI 모드 (start_claude_stream의 비-sdk 경로)   ← 이 줄 제거
  - - codex CLI exec fallback
  - - gemini, opencode (one-shot)
  - - Roundtable participants
  - - Branch shadow conversation의 첫 send (LAST_DELIVERED 비어있어 자동으로 full)
  ```
- 적용 대상에 **cli mode 추가**:
  ```
  - claude `-p` cli (start_claude_stream 의 cli 분기)
  ```
- `current_session_key()` 함수가 cli mode 의 session key 반환하도록 보강 (현재 sdk-url 만 처리할 가능성 — 실제 함수 본문 read 후 분기 추가):
  ```rust
  pub fn current_session_key(conv_id: &str, engine: &str) -> Option<String> {
      match engine {
          "claude-code" | "claude" => {
              // sdk-url path: SESSIONS lookup 우선
              if let Some(sdk_key) = sdk_session_key(conv_id) {
                  return Some(format!("claude-code:sdk:{}", sdk_key));
              }
              // cli path: RESUME_IDS (claude_sdk_session::RESUME_IDS) 의 token 활용
              if let Some(resume_id) = claude_sdk_session::RESUME_IDS.lock().get(conv_id).cloned() {
                  return Some(format!("claude-code:cli:{}", resume_id));
              }
              None
          }
          _ => /* 기존 로직 */ ,
      }
  }
  ```
  (정확한 함수 signature + 분기는 코드 read 후 결정. RESUME_IDS 가 cli 와 sdk-url 모두 사용 — 분기 trigger 는 *어느 path 가 활성* 인지 detect 하는 다른 sentinel 필요)

### Task 09-b — `agents.rs` cli 분기에서 stash_pending + promote 호출

**Changed file**: `src-tauri/src/commands/agents.rs`

**Change description**:
- cli 분기 (line 280-305) 에서 sdk-url path 와 동일 패턴으로 session_freshness 호출:
  - send 시작 직전: `session_freshness::stash_pending(&msg_id, &session_key)` — session_key 생성 (resume_token 또는 conv_id fallback)
  - 정상 종료 후: `session_freshness::promote_pending_to_delivered(&msg_id, &cid, "claude-code")` (또는 finalize_engine_run 안에서 일괄 처리)
- finalize_engine_run 의 기존 로직 변경 X — promote 호출만 추가

### Task 09-c — `claude.rs:stream_run` 종료 후 session_id 반영

**Changed file**: `src-tauri/src/agents/claude.rs`

**Change description**:
- `stream_run` 정상 응답 후 `RunOutput.session_id` 가 새 session_id 보유. 이를 `RESUME_IDS` 에 update (이미 있는 로직 활용 또는 추가)
- T2 retry 흐름 (`stream_run_once` 두 번째 호출) 후에도 새 session_id 가 RESUME_IDS 에 등록되도록 검증 — 이미 정상이면 추가 변경 X
- promote 시점은 `agents.rs` 의 finalize 흐름이라 본 파일 변경 최소

### Task 09-b (보조 가드) — 첫 send compressed-memory 미inject

**Changed file**: `src-tauri/src/commands/agents_helpers/send_common/context_loading.rs` 또는 `persistence.rs`

**Change description**:
- cli mode 의 fresh session (첫 send, `is_session_continuation = false`) 에서도 compressed-memory layer 를 *기본 미inject* 정책 적용:
  - 정확한 hook: `persistence.rs:283` 의 "new session 분기" 로그 영역 — 그 다음의 ContextPack assemble (`assemble_prompt`) 가 conversational layer 를 inject 하는데, cli mode 일 때는 compressed-memory 만 skip
  - 명시적 분기: cli mode 의 fresh session = anchor 2 turns + structured (plan/artifacts) + user prompt. compressed-memory 미inject.
- (대안) 보조 가드 없이 가도 됨 — 첫 send 한 번만 paid API trigger 가능, 그 후 minimal mode 로 회복. 사용자 가시화 토스트 ("첫 send 가 paid API 차감, 다음 send 부터 정상") 로 충분
- **Developer 판단**: T9-b 보조 가드 미적용 (단순) vs 적용 (안전). 둘 중 하나로 결정 후 PR description 에 명시

## 4. DO — 반드시 지킬 것

1. Plan §4 Task 09 의 §"Change description" + §"Verification" + §"회귀 위험 가드" 라인 단위 따름
2. Task 09-a / 09-b / 09-c 분리 commit (axis 분리)
3. session key 형식 unit test 1개 추가 (cli vs sdk-url 의 key 가 다른 형식 보장)
4. PR 1개 합본 (T9 자체가 axis 단위 — 분리 PR 필요 없음). PR title 예: `fix(agents/claude): cli mode session_freshness 적용 — double history 차단 (T9)`
5. PR description: Plan SSOT 링크 + 각 sub-step 의 Verification 결과 + DO NOT 위반 0 + baseline 카운트 비교
6. CI watch 후 머지 (admin merge 회피 — Plan §"INV-2" 정신)

## 5. DO NOT — 사이드 이펙트 차단

- ❌ `claude_sdk_session.rs` 변경 (sdk-url path 동작 그대로)
- ❌ 다른 엔진 (`agents/codex.rs`, `agents/gemini.rs`, `agents/ollama.rs`, `agents/lmstudio.rs`, `agents/opencode.rs`) 의 session_freshness 정책 변경
- ❌ `agents.rs:resolve_claude_mode()` 변경 (transport flip default 자체)
- ❌ ContextPack 의 다른 layer (structured / retrieval / docs / crg / skills) 정책 변경
- ❌ DB schema 또는 settings 변경
- ❌ 새 dependency 추가
- ❌ frontend 영향 (TS/FE 변경) — 본 task 는 backend 한정

## 6. Verification (전체)

```bash
cd src-tauri && cargo check --message-format=short
cd src-tauri && cargo test --lib agents_helpers::send_common::session_freshness
cd src-tauri && cargo test --lib   # 전체 회귀 0
npx tsc --noEmit                    # frontend 영향 0

# 회귀 grep
git diff main --name-only | grep -E "agents/(codex|gemini|ollama|lmstudio|opencode|claude_sdk_session)" && echo "WARN: out of scope" || echo "OK"
```

Manual smoke (사용자 환경 한정 가능 — 사용자에게 PR branch 의 dev rebuild 권장):
- seCall main 채팅 (1,285 messages, Auto 모드) 첫 send → 정상 응답 (compressed-memory 미inject 또는 paid API 거부 시 retry)
- 두 번째 send → backend log `[memory_policy] skipped=[context:skipped(session-continuation), retrieval:skipped(session-continuation), compressed-memory:skipped(session-continuation)]` 표시
- 응답이 history 반영 (Claude session 자체가 보유)

## 7. CI 정책

- PR + CI watch 권장 (admin merge 회피). macOS + Windows CI 양쪽 ✓ 후 머지.
- 본 task 는 backend Rust 한정 — Windows 측 cross-platform 회귀 위험 낮음. 다만 session_freshness 가 양 OS 같은 코드 경로라 Windows CI 통과로 안전 검증.

## 8. 보고 포맷 (chat)

```
## T9 결과 (PR #N)

- 변경 라인 수 + 핵심 파일 (1~3줄)
- Verification 결과 (PASS/FAIL + 핵심 출력)
- baseline 대비 테스트 카운트 (FE/Rust)
- PR URL + 머지 commit hash
- DO NOT 위반 0 (sdk-url path / 다른 엔진 / 다른 layer 미변경 grep 결과)
- 다음: v0.1.5-beta release publish 결정 (mac architect 판단 영역)
```

## 9. 막히면 (escalate)

- session key 형식 충돌 (sdk-url path 회귀) → 즉시 chat 보고. 작업 중단 후 sdk-url path 동작 검증 step 추가
- T2 retry 흐름과 promote 호출 conflict (retry 후 LAST_DELIVERED 누락) → 사용자 fact 기반 검증 (사용자 환경에서 retry 시나리오 재현)
- T9-b 보조 가드 (첫 send compressed-memory 미inject) 의 응답 품질이 unacceptable → T9-b 제거 후 T9-a 만 진행 (단순 fallback)
- 회귀 가드 위반 의심 → 작업 중단 + chat 보고

## 10. 머지 후 다음

T9 머지 = v0.1.5-beta release publish 가능 상태. mac architect 가:
1. baseline 회귀 0 확인
2. CHANGELOG `[0.1.5-beta]` entry 정리 (T1~T9 + Windows hardening 묶음)
3. tag + release 자산 빌드 + Publish 진행

본 task 머지 즉시 외부 사용자 차단 회복 effective.
