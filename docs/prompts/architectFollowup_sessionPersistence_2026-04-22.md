# Architect 후속 요청 — Session Persistence 근본 수정 plan

> 이 메시지는 기존 Architect Opus 세션 (이전에 `architectHandoff_2026-04-22.md` 로 부트스트랩한 세션) 에 이어서 전달. 세션이 이어져 있지만 preview 트렁케이트로 이전 맥락 일부 유실 가능성 있으니 아래에 재진술.

---

## 발견된 이슈

**증상**: 같은 conversation, 같은 engine (claude-code), 2-3턴 이전 대화인데도 Architect 가 자기 직전 응답의 뒷부분을 보지 못함. 실제로 Architect 가 `<!-- tunaflow:tool-request:recent_turns:4 -->` 마커를 내고 context 를 명시적으로 재조회함.

**진단 (Developer 세션이 DB 직접 조회 완료)**:

1. conversation `029d11d5-799a-401d-87e3-4c3ee21750c1` (tunaReader 프로젝트) 에 assistant 응답 5건 모두 `status='done'` + 3000~4600자로 **DB 에는 온전히 저장됨**. 데이터 손실 아님.
2. trace_log 의 모든 턴에서 ContextPack mode = `Standard(auto:standard(baseline))` + `context` 섹션 포함.
   - `is_session_continuation=true` 였다면 `session_freshness` 로직이 `context` 섹션을 drop 했을 것.
   - 드롭 안 됨 = `is_session_continuation` 가 **false** 로 평가됨 = **tunaFlow 가 매 턴마다 "새 세션" 으로 판단하고 있음**.
3. 턴 간 간격: 18:35 → 18:41 → 18:48 → 18:56 (약 5-8분). 중간에 WS idle timeout / 앱 포커스 이동 / 기타 트리거로 `claude_sdk_session` 의 WS 세션이 종료되었을 가능성.
4. 사용자 화면에는 같은 대화로 보이지만 실제 Claude CLI 프로세스 레벨에선 매 턴 fresh spawn → Claude 자체 session 버퍼 비어있음.
5. 이를 tunaFlow 가 감지해 ContextPack 의 `context` 섹션으로 직전 turn 들을 주입하는데, 긴 assistant 응답 (3000~4600자) 이 preview 로 트렁케이트되어 뒷부분이 잘림.

**DB 증거 (참고)**:
```
trace_log: 모든 턴 mode=Standard(auto:standard(baseline)), ctx_sections 에 "context" 포함
agent_session_audit: 2건 모두 outcome=committed (panic/rollback 없음)
messages: 길이 134 하나 (recent_turns 요청 턴) 외 전부 3000~4600자 정상 저장
```

---

## 사용자 지침 (중요, 해결 범위 강제)

> **"체감 개선으로 토큰을 낭비하면 안 된다"**
> 
> **"압축되어서 사라진 원문이 아니라면 full content 주입은 하면 안 된다. 당연히 세션에 남아있는 컨텍스트니까."**

따라서 다음은 **금지 영역**:

- ❌ ContextPack 의 `context` 섹션에 preview 대신 full content 주입으로 "땜질"
- ❌ recent_turns 도구의 2000자 cap 단순 확대
- ❌ anchor 2 turn → N turn 확대 (재주입 자체가 토큰 낭비의 본질)

**허용되는 해결 방향**:

- ✅ `claude_sdk_session` (또는 동등 경로) 의 WS 세션 **persist + resume** 정확도 개선
- ✅ `session_freshness::is_session_continuation` 판정 로직의 **false positive/negative** 교정
- ✅ Claude 자체 session buffer 가 살아있는 경우 tunaFlow 의 재주입을 **완전히 skip** (이미 is_session_continuation=true 경로에 있음 — 이 경로가 실제로 타지는 빈도가 올라가야 함)
- ✅ 세션이 정말로 끊긴 케이스를 **정확히 검출** 해서 그 때만 Full 재주입. 지금은 "잘 유지되고 있는 세션"도 재주입 대상이 되는 게 문제.

근본 원칙: **Claude 자체 session buffer 를 source of truth 로** 하되, tunaFlow 의 is_session_continuation 판정이 이 사실을 놓치지 않게 해야 한다. 판정이 정확해지면 재주입은 불필요해지고 토큰 소비도 자연스럽게 감소.

---

## 요청

### 1. 상태 재검토 (직접 확인 필요)

아래 파일 / DB 를 직접 검증하고 위 Developer 진단과 일치/불일치를 보고:

- `src-tauri/src/agents/claude_sdk_session.rs` — WS 세션 관리, prewarm, kill, resume 로직
- `src-tauri/src/commands/agents_helpers/send_common/session_freshness.rs` — `current_session_key`, `is_session_continuation`, `stash_pending`, `promote_pending_to_delivered` 네 함수의 실제 판정 기준
- `src-tauri/src/commands/agents_helpers/send_common/persistence.rs` §A3 근처 — session_freshness 호출 지점, fresh vs continuation 분기 로그 (`eprintln!` 있는 부분)
- `conversations` 테이블의 `resume_token`, `resume_token_engine` 컬럼 활용처 — finalize_engine_run 에서 어떻게 갱신되는지
- 필요 시 `tool-request:rawq:claude_sdk_session` / `tool-request:graph:callers_of is_session_continuation` 사용

### 2. Root cause 가설 세우기

"매 턴마다 is_session_continuation=false 로 평가되는 이유" 를 코드 증거 기반으로 1~3개 가설 제시. 각 가설에 대해:
- 검증 방법 (logged evidence / 재현 실험)
- 수정 범위

### 3. Plan 작성

위 규약 (`harnessVerificationGapPlan.md` §5 proposer 4-section) 준수하여 plan document 작성:

파일명: `docs/plans/sessionContinuityFixPlan.md` (또는 더 나은 slug 제안)

- TL;DR — 근본 수정 절차 (토큰 낭비 우회 금지)
- Specification — `claude_sdk_session` 수정 지점, `session_freshness` 판정 조건 재정의, `resume_token` 활용 정확화, 필요 시 backoff/retry 로직
- Invariants — 최소 4개. 예시:
  - `[INV-?]` ContextPack 은 is_session_continuation=true 인 경우 recent_context + compressed_memory 를 **절대** 주입하지 않는다
  - `[INV-?]` is_session_continuation 판정은 Claude WS 세션의 live 상태 + resume_token 유효성 **양쪽** 을 고려한다
  - `[INV-?]` 세션 respawn 은 **검출된 실제 종료** (WS close, timeout, kill) 이후에만 발생
  - `[INV-?]` resume_token 만료/무효 시 fallback 경로는 토큰 폭증 없이 최소 context 로 복구
- Rationale (reviewer-only) — 왜 preview→full injection 이 아닌가, 대안 고려 과정, 측정 방법

### 4. Subtask 분해

Plan 을 3~5 subtask 로 쪼개서 각 subtask 파일 작성. Developer 세션이 순차 구현 가능한 단위.

---

## 출력 채널

Plan + subtasks 산출 후 그대로 여기 (Developer 세션) 로 가져오시면 1차 검토. 필요 시 Codex Reviewer 병렬 검토.

## 추가 팁

- 이 세션 (Architect) 자신도 이 버그의 영향 받고 있음. tool-request 필요 시 `probe_message` → `fetch_slice` → `full_message` 순으로 최소 토큰 사용.
- 동일 근거가 이미 이전 턴에 있었다면 재조회 전 자신이 가진 맥락 먼저 점검.
- 본 문제는 **`contextpack handoff truncation fix`** (세션 38 에서 deferred, `fix/contextpack-handoff-recall` 브랜치명 기록) 와 중첩될 가능성. 기존 계획이 있으면 합류.

시작하세요.
