---
title: Branch cancel semantics — audit (Task A same-session model 후속)
status: completed
created_at: 2026-04-25
updated_at: 2026-04-25
canonical: true
related:
  - docs/plans/branchCancelSemanticsPlan_2026-04-25.md
  - docs/plans/branchInheritsMainSessionPlan_2026-04-25.md  # PR #198 머지 완료
  - src-tauri/src/agents/claude_sdk_session.rs
  - src-tauri/src/commands/agents.rs
  - src-tauri/src/commands/roundtable.rs
  - src/stores/slices/runtimeSlice.ts
  - src/components/tunaflow/NewMessageInput.tsx
---

# 배경

PR #198 (`branchInheritsMainSession`) 머지 후 brand 에서 cancel 버튼이 작동하지 않는 사용자 보고 (2026-04-25).
PR #198 은 SESSIONS / RESUME_IDS 키를 main root 로 normalize 했지만, cancel 경로는 동일 normalize 가 안 됐다.

# 현재 cancel 경로 (BE)

## CancelRegistry (`src-tauri/src/lib.rs:13`)

```rust
pub struct CancelRegistry(pub Arc<parking_lot::Mutex<HashSet<String>>>);
```

- `cancel(conv_id)` → set 에 conv_id insert
- `check_and_consume(conv_id)` → set 에 있으면 remove 후 true
- `clear(conv_id)` → 정상 완료 시 호출 (실제로는 미사용)

키 = conversation_id. brand 는 `branch:<branch_id>` 그대로 사용 — normalize 없음.

## cancel_running tauri command (`src-tauri/src/commands/roundtable.rs:385`)

```rust
pub fn cancel_running(conversation_id: String, cancel: State<CancelRegistry>) -> Result<(), AppError> {
    cancel.cancel(&conversation_id);
    Ok(())
}
```

전달된 conv_id 그대로 set 에 넣음. normalize 안 함.

## is_cancelled callback in stream_run_sdk (`src-tauri/src/agents/claude_sdk_session.rs:708,749`)

```rust
// agents.rs:239
{ let c = cid2.clone(); let r = cancel_arc; move || { r.lock().remove(&c) } }
```

- closure 가 `cid2` (start_claude_stream 으로 들어온 conv_id, brand 면 `branch:b20`) 캡처
- `set.remove(&cid2)` — set 에 자기 conv_id 가 있으면 true 반환 (consume)

cancel branch (claude_sdk_session.rs:752):
```rust
let interrupt = serde_json::json!({ "type": "control_request", "request": { "subtype": "interrupt" }}).to_string();
let _ = session.to_claude_tx.send(interrupt);
return Err(AppError::Agent("cancelled by user".into()));
```

→ 진행 중 stream interrupt + 함수 Err return. `finalize_engine_run` 이 그 Err 을 받아 message status 갱신.
→ session (process / SESSIONS / RESUME_IDS) 은 그대로 살아있음. **이미 옵션 X (stream abort only) 에 가까운 동작.**

# 현재 cancel 경로 (FE)

## runtimeSlice.cancelOperation (`src/stores/slices/runtimeSlice.ts:321`)

```ts
cancelOperation: async (threadId?: string) => {
    const target = threadId ?? get().selectedConversationId;
    if (target) {
        try { await invoke("cancel_running", { conversationId: target }); } catch ...
    }
    ...
}
```

caller 가 threadId 안 넘기면 selectedConversationId fallback. brand 는 caller 가 brand 의 shadow conv_id 를 명시적으로 넘겨야 함.

## NewMessageInput cancel 버튼 (`src/components/tunaflow/NewMessageInput.tsx:579-586`)

```tsx
{isCurrentThreadRunning && (
    <button onClick={() => cancelOperation(selectedConversationId ?? undefined)} ... >
        Cancel
    </button>
)}
```

- `isCurrentThreadRunning = !!effectiveThreadId && runningThreadIds.includes(effectiveThreadId)` (line 186)
- `effectiveThreadId = threadMode ? threadBranchConvId : selectedConversationId` (line 185)

→ 버튼 visibility 는 `effectiveThreadId` (brand 면 `branch:b20`) 기준이지만, **클릭 핸들러는 `selectedConversationId` (main conv id) 를 cancel 에 넘김**. 불일치.

# 진단 — 왜 brand cancel 이 작동 안 하나 (옵션 A 추정)

PR #198 의 normalize 는 SESSIONS / RESUME_IDS 만 root main 으로 정규화. cancel registry 는 conv_id 단위 그대로:

1. brand 송신 → `start_claude_stream(input.conversation_id="branch:b20")` → `cid2 = "branch:b20"` 캡처
2. brand 의 `is_cancelled` callback 은 `set.remove("branch:b20")` 호출
3. 사용자가 cancel 클릭 → FE 가 `cancel_running(selectedConversationId="conv-abc")` 호출
4. registry: `set.insert("conv-abc")` — brand:b20 이 아닌 main conv id 추가
5. brand 의 `is_cancelled` callback → `set.remove("branch:b20")` → 없음 → false → cancel 무시

→ **No-op (사용자 보고와 일치).**

만약 FE 가 brand 모드에서 brand:* 를 cancel 에 정확히 넘기더라도:
- `set.insert("branch:b20")` → brand 의 callback 이 `set.remove("branch:b20")` 으로 정상 cancel
- 그러나 main 동일 송신 후 cancel 도 같은 brand session 의 stream 임 (PR #198 same-session) → main 송신 → main `is_cancelled` callback 은 `set.remove("conv-abc")` 보는데, brand 가 아닌 main 의 cancel 은 그 키로 들어가야 cancel 됨
- 즉 brand send / main send 별로 cancel 키 분리되면 same-session 이라도 stream 식별이 명확

# 결론 — 두 곳 fix 필요 (옵션 X)

## (1) FE — Cancel 버튼이 effectiveThreadId 를 정확히 넘김

`NewMessageInput.tsx:581` 변경:
```tsx
onClick={() => cancelOperation(effectiveThreadId ?? undefined)}
```

## (2) BE — cancel registry 키 그대로 유지 (normalize 안 함)

PR #198 의 SESSIONS / RESUME_IDS 와 달리, **cancel 은 conv_id 단위로 분리** 가 의도에 맞음:
- 같은 main session (brand=main 공유) 에서도 brand 의 stream 만 abort 하고 싶으면 conv_id 단위 식별이 필요
- brand 에서 cancel 클릭 → brand 의 is_cancelled callback 만 발동 → brand 의 stream 만 interrupt → main 영향 없음

cancel registry 의 conv_id 키는 그대로 두고 (normalize 안 함), 단 FE 가 정확한 conv_id (brand 면 brand:*, main 면 main) 를 전달해야 함.

## (3) Cancel 의미 stream abort only 재확인

기존 코드 `claude_sdk_session.rs:749-757` 가 이미:
- control_request "interrupt" 보내고
- Err("cancelled by user") return
- session / SESSIONS / RESUME_IDS / process 모두 유지

→ **이미 옵션 X 에 가까운 구현**. 추가 stream abort token 도입 불필요. 단지 normalize 분리 정책 + FE 호출자 fix.

# Invariants 만족 여부

| INV | 현재 상태 | fix 후 |
|---|---|---|
| INV-1: stream abort only, session 유지 | claude_sdk_session.rs 이미 그렇게 구현 | OK |
| INV-2: brand cancel 이 main 영향 없음 | brand cancel 자체가 동작 안 함 | OK (FE 가 brand:* 를 정확히 전달) |
| INV-3: main cancel 도 stream abort | main 은 동작 (selectedConversationId 일치) | OK |
| INV-4: session kill 은 별도 명시적 command | restart_sdk_session 그대로 유지 | OK |

# 비고 — codex_app_server / claude CLI / gemini

각 엔진의 `is_cancelled` callback 도 `cancel_arc.lock().remove(&conv_id)` 패턴으로 동일 — start_claude_stream / start_gemini_stream / start_codex_run 의 cid 캡처 유지하면 PR #198 normalize 와 무관하게 conv_id 단위로 동작. 추가 변경 불필요.

# 변경 범위 (작은)

- FE: `src/components/tunaflow/NewMessageInput.tsx:581` 한 줄
- BE: 변경 없음 (이미 옵션 X 의미 충족)
- 테스트: cancel 계약 단위 테스트 보강 (brand cancel main 미영향 등)

# 후속 (선택)

- 옵션 Z (per-sender 격리, 동일 conv 내 메시지별 식별) 은 future plan. 현재는 conv 단위 격리만으로 사용자 의도 충족.
- session kill scenario (engine 변경 등) 는 `restart_sdk_session` 명시 호출 — UI cancel 버튼과는 별 경로 유지 확인.
