---
title: Harness Verification Gap — Invariants / Divergence / Regression / Transactional Boundary
status: planned
priority: P1
created_at: 2026-04-22
related:
  - docs/archive/plans/completed/roundtableBlindVerifierPhasePlan_2026-03-30.md
  - docs/archive/plans/completed/roundtableParticipantRoleBlindUiPlan_2026-03-30.md
  - src-tauri/src/commands/roundtable_helpers/types.rs  # role_guidance()
  - src-tauri/src/commands/roundtable_helpers/prompt.rs
  - src-tauri/src/commands/roundtable_helpers/sequential.rs
  - src-tauri/src/commands/roundtable_helpers/deliberative.rs
  - docs/ideas/litertLmIntegrationIdea.md  # tunaFlow 범위 확인 (소형 LLM X)
---

# Harness Verification Gap 해결 플랜

> **전제**: tunaFlow 는 이미 4역할 RT (proposer / reviewer / verifier / synthesizer) + blind 모드 + 역할별 token cap 차등을 보유. 본 플랜은 **신설이 아니라 확장**.

---

## 0. 배경 — 현재 구조 재확인

`src-tauri/src/commands/roundtable_helpers/types.rs:92-123` 의 `role_guidance()`:

| 역할 | 현재 지침 요지 |
|---|---|
| `proposer` | 독립 분석, 결론 먼저, 가정 flag |
| `reviewer`/`critic` | 4차원 점수 (plan_coverage/code_quality/test_coverage/convention), findings + recommendations 분리, failed_subtask_ids |
| `verifier`/`judge` | 증거 기반, verdict 먼저, 관찰 vs 추론 구분 |
| `synthesizer`/`lead` | consensus/contested/dissent 3섹션, vote tally 일관성 |

**Blind 모드**: 다른 참여자 응답 미표시 → 독립 판정
**Output cap**: proposer=1200 / reviewer=900 / verifier=800 / synthesizer=2000

구조는 튼튼함. 그러나 **실제 operation 중에 Opus 수준의 결함을 하위 모델이 잡아낸 실례가 드물다**. 원인은 구조가 아니라 **입력 명세의 부재** — reviewer 에게 "무엇을 봐야 하는지" 가 구체적으로 전달되지 않음.

---

## 1. 문제 정의 (구체 4건)

### 1.1 Invariants 누락 (Verification Gap 의 핵심)
- Proposer/Architect 가 알고 있는 암묵적 invariants (예: "adopt 중 streaming suspend 유지", "mutex re-entrant 금지", "read/write 같은 thread 중첩 금지") 는 **코드에도, 스펙에도 명시되지 않음**.
- 결과: Developer 가 invariant 위반 구현 → Reviewer 가 "코드 깔끔하다" 로 통과 → 인간 재검토 때 발견.
- 실례: `broadcast_event` re-entrant deadlock (세션 before PR #111), `finalize_engine_run` deadlock (work-safety.md 2026-04-22).

### 1.2 Divergence / Doom Loop
- Reviewer 가 같은 결함 카테고리를 N 번 반복 지적해도 자동 에스컬레이션 없음.
- Developer 가 "fix 가 regression 을 유발" 하는 케이스 감지 없음 — 이전 라운드 수정사항이 되돌아가도 reviewer 는 새 결함에만 집중.

### 1.3 Regression Gate 부재
- Reviewer 가 **새로 지적된 문제** 에만 집중. 이전 라운드에서 PASS 된 항목이 이번 라운드에서 깨져도 감지 안 함.

### 1.4 State Pollution (Transactional Boundary)
- 에이전트 중간 실패 시 DB 에 half-formed row 가 남음 (`plans`, `artifacts`, `conversation_memory`).
- 다음 세션이 이를 "기정사실" 로 읽음.
- SessionFreshness 개념은 있으나 transactional boundary 는 아님.

---

## 2. Phase 1 — Invariants Checklist 포맷 강제 (P0 within this plan)

### 2.1 Proposer 산출물 필수 필드 추가

기존 role_guidance(`"proposer"`) 확장:

```
**Proposer guidelines (extended):**
- Form your analysis independently.
- Lead with conclusion.
- Flag assumptions explicitly.
- **(NEW) Emit `## Invariants` section** — structured list of constraints that the implementation must NEVER violate.
  Format: `- [INV-N] <short statement> — <why it matters>`
  Examples:
    - [INV-1] broadcast_event 내부에서 db.write.lock() 호출 금지 — 동일 thread 재진입 deadlock 방지
    - [INV-2] adopt 중 streaming subscription 해제 금지 — 메시지 소멸 방지
```

### 2.2 Reviewer 지침 확장

```
**Reviewer guidelines (extended):**
- ... (existing 4-dim scoring)
- **(NEW) Invariants verification** — for each INV-N in the proposer's output,
  emit an entry in `invariant_checks`:
    { "id": "INV-1", "status": "pass|fail|cannot_verify", "evidence": "<file:line or reasoning>" }
- Verdict MUST be `fail` if ANY invariant check is `fail`.
- If `cannot_verify`, Developer must add explicit test that exercises the invariant.
```

### 2.3 구현 지점

- `src-tauri/src/commands/roundtable_helpers/types.rs` — `role_guidance()` 문자열 확장
- `src-tauri/src/commands/roundtable_helpers/prompt.rs` — 출력 스키마에 `invariants` / `invariant_checks` 필드 추가
- Rust unit test: `test_role_guidance_proposer_contains_invariants`, `test_role_guidance_reviewer_contains_invariant_checks`

### 2.4 효과 가설
- Opus 가 알고 있는 지식을 **텍스트로 강제 추출** → 하위 모델도 line-by-line 검증 가능.
- Invariant 를 쓸 수 없는 Architect 는 "기본으로 사용자가 확인" → human-in-the-loop 명확화.

---

## 3. Phase 2 — Divergence Detector + Regression Gate

### 3.1 Divergence Detector
- 동일 RT 에서 reviewer 가 같은 finding category (예: `defect_type: "deadlock"`) 를 N 번 (기본 N=3) 연속 지적 시:
  - Synthesizer 의 verdict 를 강제로 `escalate_to_human` 으로 overwrite
  - UI 에 명시적 경고 badge

### 3.2 Regression Gate
- Reviewer 출력에 `regression_check` 필드 추가:
  ```
  regression_check: { "prev_findings_resolved": [id, id, ...], "newly_broken": [id, id, ...] }
  ```
- Developer 에게 전달되는 summary 는 `newly_broken` 을 우선 표시.
- `newly_broken` 이 비어있지 않은 round 가 2회 연속되면 divergence detector 트리거.

### 3.3 구현 지점
- `roundtable_helpers/deliberative.rs` — 라운드 간 상태 전달 확장
- DB 스키마: `roundtable_findings` 테이블 (이미 있는 findings 를 라운드 단위로 normalize). 마이그레이션 v43 후보.
- Frontend: RT 드로어에 regression 배지

---

## 4. Phase 3 — Transactional Session Boundary

### 4.1 문제
- `stream_run` 도중 에이전트가 panic / 사용자 취소 / timeout 시 DB 에 중간 상태 남음 (messages, plans, artifacts, memory).
- 다음 요청이 이를 정상 상태로 오인.

### 4.2 설계
- 각 "에이전트 세션" 을 SQLite SAVEPOINT 로 감싼다:
  ```rust
  conn.execute("SAVEPOINT agent_session_{sid}", []);
  // ... run agent, write rows ...
  if ok {
      conn.execute("RELEASE SAVEPOINT agent_session_{sid}", []);
  } else {
      conn.execute("ROLLBACK TO SAVEPOINT agent_session_{sid}", []);
  }
  ```
- 단 SQLite SAVEPOINT 는 write connection 단일 트랜잭션에서만. tunaFlow 는 dual r/w — write lock 잡고 있는 동안 다른 write 는 대기.
- 실시간 streaming 메시지 append 는 SAVEPOINT 밖에 두고, 실패 시 **세션 상태만 rollback, streaming 기록은 보존** (사용자가 실패 맥락을 볼 수 있어야 함).

### 4.3 구현 지점
- `commands/agents_helpers/send_common/persistence.rs` — 현재 4-split (A0 read / A1 write / A2 read / A3 write) 구조를 transactional wrapper 로 감싸기
- DB 마이그레이션 v44: 복구 로그 테이블 `agent_session_audit` — 어떤 세션이 commit / rollback 됐는지 기록
- `finalize_engine_run` 경로의 mutex re-entrant 주의 (최근 사고 2026-04-22)

### 4.4 위험
- Streaming 중 장시간 write lock 유지 = 다른 write 기아. 현재 세션 35 의 per-chunk lock + yield 패턴과 충돌 가능.
- 대응: SAVEPOINT 는 "구조화된 commit 경계" (plan/artifact/memory 등) 에만. 실시간 streaming chunk 는 outside.

---

## 5. Phase 4 — Architect 출력 2-Track 분리 (토큰 효율)

### 5.1 문제
- Architect 산출물은 설계 근거·대안·tradeoff 까지 verbose. Developer 는 이걸 통째로 받아 읽음 → 실제로 필요한 건 실행 지침인데 토큰 낭비.

### 5.2 설계
Architect 출력 강제 구조:

```markdown
## TL;DR for Developer
<5-20 줄, 실행 지침만. Invariants 링크>

## Specification
<구체 interface/signature/behavior>

## Invariants
<Phase 1 참조>

## Rationale (reviewer 전용)
<설계 근거·대안·tradeoff>
```

Developer 에게 전달되는 ContextPack 은 TL;DR + Specification + Invariants 만. Rationale 은 reviewer 컨텍스트에 별도 section 으로.

### 5.3 구현 지점
- `commands/agents_helpers/send_common/prompt_assembly.rs` — role 별 context filter 추가
- role_guidance("proposer") 확장

---

## 6. Phase 5 — Dual-Reviewer A/B (후순위, 비용 측정 후 결정)

### 6.1 가설
- Codex 단일 reviewer 대신 Codex + Gemini 병렬. 다른 blind spot.
- 두 모델이 동시 지적 → high-signal. 한쪽만 지적 → medium-signal.

### 6.2 실행
- 기존 RT `deliberative` 모드로 즉시 가능 (구조 있음).
- 실험: 10-20 PR 을 Dual-Reviewer vs Single-Reviewer 로 A/B. 측정:
  - Opus-level defect catch rate
  - False positive rate (recommendation 까지 포함)
  - 토큰 비용 배수 (input 동일, output 만 2배 → ~1.7x)

### 6.3 결정 기준
- Catch rate 개선이 **비용 배수보다 큰 경우**에만 default 전환.
- 그렇지 않으면 "deep review" 모드로만 제공 (사용자가 명시 요청).

---

## 7. 구현 순서 (의존성)

```
Phase 1 (Invariants)  ──┬─> Phase 2 (Divergence/Regression)  ──┐
                        │                                      │
                        └─> Phase 4 (Architect 2-track)        │
                                                               │
Phase 3 (Transactional boundary, independent)  ────────────────┤
                                                               │
                                                               ▼
                                                    Phase 5 (Dual-Reviewer A/B)
```

Phase 1 이 모든 것의 전제 — **invariants 가 없으면 regression gate 도, verifier 도 체크할 대상이 없음**.

---

## 8. 측정 지표

| Phase | 측정 | 목표 |
|---|---|---|
| 1 | Reviewer verdict 에 `invariant_checks` 필드 출현율 | 100% |
| 1 | Invariant 위반이 reviewer 에 의해 catch 된 비율 | >70% (현재 추정 <20%) |
| 2 | Doom loop (동일 finding 4+회 반복) 발생률 | 0 — escalate 로 전환 |
| 2 | Regression (이전 PASS → 현재 FAIL) catch 율 | 100% |
| 3 | 에이전트 중간 실패 후 stale row 잔존율 | 0 |
| 4 | Architect→Developer 전달 토큰 평균 | -30% |
| 5 | Opus-level defect catch rate 개선 | A/B 측정 후 결정 |

---

## 9. Scope 경계 (하지 않을 것)

- **소형 LLM 내장** — Phase 5 에서 Gemma/LiteRT-LM 을 reviewer 로 두는 안은 tunaFlow 범위 밖 (memory: `project_product_scope.md`). 상용 CLI (Claude/Codex/Gemini) 만 사용.
- **Property-based test 자동 생성** — 유망하나 별도 plan.
- **Architect 모델 Sonnet 강등** — 지금은 Opus 고정. "Deep mode dial" 은 별도 UX plan.

---

## 10. 기존 문서 연계

- `roundtableBlindVerifierPhasePlan_2026-03-30` (archive/completed) — blind verifier 구조의 원형
- `roundtableRoleTerminologySeparationPlan_2026-03-30` (active) — 프로필 role vs RT role 분리. 본 plan 의 전제
- `docs/reference/work-safety.md` — re-entrant deadlock 사례 (Phase 3 근거)

---

## 11. Open Questions

1. Invariants 가 너무 많아지면 reviewer 검증 부담 폭증 — 상한 (예: 최대 7개) 을 강제할 것인가?
2. `cannot_verify` invariant 가 다수일 때 Developer 에게 test 를 쓰게 할지, 아니면 human 에 에스컬레이션할지?
3. Transactional boundary 가 RT 전체 (proposer→reviewer→synthesizer) 를 감싸야 하는가, 아니면 stage 별로 분리?
4. Dual-reviewer 비용이 가치 없으면 rejected — rejection criteria 를 사전에 기록할 것.
