---
title: Design Review Gate — Plan 승인 전 Architect↔Reviewer RT 선택 경로
status: planned
priority: P1
created_at: 2026-04-22
related:
  - src/components/tunaflow/chat/PlanProposalCard.tsx
  - src-tauri/src/commands/branches.rs                     # branch_mode 분기
  - src-tauri/src/commands/roundtable_helpers/              # RT deliberative 모드 + role guidance
  - docs/plans/harnessVerificationGapPlan.md                # §5 reviewer 규약 (본 plan 은 Phase 7 성격)
  - docs/plans/searchPipelineFromSecallPlan-part2.md        # 본 gate 를 거친 실제 사례 (Codex 3 라운드)
  - docs/plans/threadModelRoundtableRedesign.md             # Branch/RT 통합 모델
triggered_by:
  - 2026-04-22 세션 — Architect↔Codex 3 라운드로 Search Phase C Part 2 설계 blocker 4건 사전 해소.
    Dev→Review RT 만으로는 설계 단계 결함을 못 잡는다는 사실을 사용자 지적으로 확인.
---

# Design Review Gate — Plan 승인 시 RT 선택 경로

> 현재 RT 는 **구현 산출물 gate** (dev→review) 만 존재. 설계 단계에서 reviewer 가 invariant·대안·scope 를 검증하는 경로는 부재. 본 plan 은 PlanProposalCard 에 **사용자 1-click 선택 (RT 검토 vs 바로 승인)** 을 추가하고, RT 경로는 기존 Branch/RT 인프라를 재활용한다.

---

## TL;DR for Developer

1. **PlanProposalCard 에 승인 액션을 2분기** — 기존 단일 "승인" 버튼을 `[바로 승인]` / `[RT 검토 먼저]` 2 버튼으로 교체. 카드 상단에 판단 힌트 표시 (INV 수, subtask 수, 영향 경로). 힌트는 **가이드일 뿐 강제 규칙 없음**. 사용자 주권 유지.
2. **`branch.mode = 'design_review'` 신설** — 기존 `chat` / `roundtable` 2종에 3번째 mode 추가. `src-tauri/src/commands/branches.rs:178, 465` 의 `branch_mode` 분기에 반영. DB migration 불필요 (mode 는 TEXT 컬럼).
3. **Reviewer role guidance 를 이번 Codex 응답 포맷으로 고정** — `roundtable_helpers/types.rs::role_guidance("reviewer")` 에 `invariant_checks` / `scores(4-dim)` / `findings(BLOCKER|MAJOR|MINOR)` / `recommendations` / `failed_subtask_ids` / `verdict(pass|fail|escalate_to_human)` / `regression_check` 스키마 포함. harness Phase 1·2 의 기존 invariant_checks / regression_check 위에 scores·findings·verdict 를 얹는 구조.
4. **Reviewer 엔진은 Codex (또는 Gemini) 고정** — Architect 가 Opus 이므로 blind 확보. 사용자가 필요시 Settings 에서 변경 가능 (Q-2 참조).
5. **라운드 상한 3 + escalate 배지** — 3라운드 후에도 verdict=fail+BLOCKER 잔존 → UI 에 "human 확인 필요" + "강제 승인" 모달. harness Phase 2 divergence detector 와 연동 (동일 finding category N 회 반복 시 조기 escalate).
6. **Plan adopt = 문서 기반 머지** (기존 adopt 는 메시지 기반). `plan_document_id` + 본문 + RT transcript 를 main conversation 에 "plan 승인 완료" 시스템 메시지 + 문서 링크로 주입. 가장 큰 신규 작업 — Subtask 03 의 핵심 scope.
7. **Transcript auto-append** — RT 라운드 종료 시 reviewer 산출을 plan 문서 말미에 `## Codex Review (Round N — YYYY-MM-DD)` 섹션으로 auto-append. Developer 가 "왜 이렇게 결정됐지" 되짚을 때 source of truth.

구현 순서: 1 → 2 → 3 → 4 (평가·측정은 Developer 구현 후 수동 acceptance).

**하지 말 것**: (a) RT 자동 트리거 규칙 엔진, (b) plan proposal 외 다른 UI 에 design review 확산, (c) Codex 호출을 main thread 블로킹으로.

---

## Specification

### 1. UI — PlanProposalCard 승인 2분기

현재 `src/components/tunaflow/chat/PlanProposalCard.tsx` 에 "승인" 단일 버튼이 있다고 가정 (실제 prop 구조는 Developer 가 확인).

변경:

```tsx
<PlanProposalCard>
  <CardHeader>
    <Title>{plan.title}</Title>
    <HintRow>
      INV {plan.invariants.length} · Subtask {plan.subtasks.length} · {plan.touchedPaths.length} paths
      {shouldSuggestRT && <HintPill>RT 검토 권장</HintPill>}
    </HintRow>
  </CardHeader>
  <ActionRow>
    <Button variant="primary-subtle" onClick={onApproveDirect}>
      바로 승인 → 구현
    </Button>
    <Button variant="primary" onClick={onApproveViaRT}>
      RT 검토 먼저 (Architect ↔ Codex)
    </Button>
  </ActionRow>
</PlanProposalCard>
```

`shouldSuggestRT` heuristic (힌트용, 강제 아님):
```ts
const shouldSuggestRT =
  plan.invariants.length >= 3 ||
  plan.subtasks.length >= 2 ||
  plan.touchedPaths.some(p =>
    p.startsWith('src-tauri/src/db/migrations') ||
    p.startsWith('src-tauri/src/agents/') ||
    p.startsWith('src-tauri/src/commands/agents_helpers/send_common/')
  );
```

힌트 텍스트는 회색 14px, pill 은 노란색 accent. 버튼 클릭은 둘 다 즉시 동작.

### 2. Backend — `branch.mode = 'design_review'`

`src-tauri/src/commands/branches.rs:178`:
```rust
let branch_mode = input.mode.as_deref().unwrap_or("chat");
// before
let is_rt = branch_mode == "roundtable";
// after
let is_rt = matches!(branch_mode, "roundtable" | "design_review");
let is_design_review = branch_mode == "design_review";
```

`open_branch_stream` 또는 동등 경로가 `design_review` mode 진입 시:
1. 시스템 메시지 1건 자동 삽입: `"Design review started: plan=<plan_id>, reviewer=codex, round=1"`
2. RT payload 에 plan 본문 + subtask 파일들 + frontmatter `related` 경로 자동 주입
3. Reviewer role guidance (§3) 로드

DB migration 불필요 — `branches.mode` 가 TEXT 라 새 값 허용.

### 3. Reviewer role guidance 확장

`src-tauri/src/commands/roundtable_helpers/types.rs::role_guidance("reviewer")` (또는 `"critic"`) 에 design_review 모드 전용 스키마 고정:

```
**Reviewer guidelines (design_review mode):**
- You are a blind verifier of the Architect's plan.
- Produce the following sections in order:

## Invariant checks
```json
[{"id": "INV-N", "status": "pass|fail|cannot_verify", "evidence": "<file:line or reasoning>"}]
```

## Scores (1-5)
- plan_coverage: N/5 — reason
- code_quality: N/5 — reason
- test_coverage: N/5 — reason
- convention: N/5 — reason

## Findings
- [BLOCKER] <file:line> — <defect>
- [MAJOR] ...
- [MINOR] ...

## Recommendations
- [BLOCKER] <minimal fix>
...

## failed_subtask_ids
[NN, NN]  (or empty [])

## Verdict
pass | fail | escalate_to_human — <one-line reason>

## regression_check
{"prev_findings_resolved": [...], "newly_broken": [...]}
```

- Verdict MUST be `fail` if any `invariant_checks.status == "fail"`.
- Verdict MUST be `escalate_to_human` if round >= 3 and BLOCKER still present.
- No subjective "clean/nice/better" language — require file:line or concrete counter-example.
- Focus areas 는 Architect 가 prompt 에 추가 지정 가능.
```

이 guidance 는 harness Phase 1 (invariant_checks 필수) + Phase 2 (regression_check) 위에 design-review 전용 필드를 얹은 superset. `_mode` 파라미터로 분기:

```rust
pub fn role_guidance(role: &str, mode: Option<&str>) -> &'static str {
    match (role, mode) {
        ("reviewer" | "critic", Some("design_review")) => DESIGN_REVIEW_REVIEWER_GUIDANCE,
        ("reviewer" | "critic", _) => REVIEWER_GUIDANCE,  // 기존 Phase 1 guidance
        ...
    }
}
```

### 4. Reviewer 엔진

- **기본**: Codex CLI (`codex -p`) 또는 Codex app-server. Architect 가 Opus 이므로 blind 확보.
- **fallback**: Gemini CLI — Codex 미설치 시 자동 fallback (기존 engine resolve 경로 재활용).
- **Settings**: `Settings > 검색` 옆 새 섹션 `Settings > Design Review` 에 reviewer engine 드롭다운 (`codex` | `gemini`). 기본 `codex`.
- 호출 패턴은 기존 `start_codex_stream` 과 동일 — 별도 인프라 신설 없음.

### 5. 라운드 상한 + escalate

Plan frontmatter 또는 RT branch 에 `design_review_round: u8` (기본 1). 종료 조건:

| 조건 | 액션 |
|---|---|
| verdict=`pass` | RT 드로어에 "승인 가능" 배지, PlanProposalCard 의 "바로 승인 → 구현" 버튼 재활성화 + "RT 검토 완료 (N라운드)" 라벨 |
| verdict=`fail` + round < 3 | Architect 에게 reviewer 피드백 자동 전달 → Architect 가 plan 수정 → 자동 다음 라운드 트리거 |
| verdict=`fail` + round == 3 | `escalate_to_human` 배지. 사용자에게 "3라운드 후에도 blocker 있음. 강제 승인 하시겠습니까?" 모달. 선택지: (a) 강제 승인 → 구현, (b) Architect 에게 plan 전면 재작성 요청, (c) plan 폐기. |
| verdict=`escalate_to_human` (reviewer 자발적) | 동일 모달 |

강제 승인 시 plan 문서에 `force_approved_at_round: 3` + `blocker_findings: [...]` 메타 기록. Developer 는 구현 시 이 blocker 를 직접 판단.

**Divergence detector 연동**: reviewer 가 같은 finding category 를 N=2 라운드 연속 지적 시 round 3 기다리지 않고 즉시 escalate (harness Phase 2 §3.1 재사용).

### 6. Plan adopt — 문서 기반 머지

기존 `adopt_branch` (branches.rs) 는 **메시지 요약 삽입** 모델. Design review 는 **plan 문서 확정** 이 목적이라 경로가 다름:

```rust
pub fn adopt_design_review(
    conn: &Connection,
    branch_id: &str,
    plan_document_path: &str,
    transcript_section: &str,   // "## Codex Review (Round 3 — ...)" 블록
    app: &AppHandle,
) -> Result<(), AppError> {
    // 1) plan 문서 말미에 transcript_section append (문서 hygiene)
    let existing = std::fs::read_to_string(plan_document_path)?;
    let merged = format!("{}\n\n{}\n", existing.trim_end(), transcript_section);
    std::fs::write(plan_document_path, merged)?;

    // 2) main conversation 에 "design review 완료" 시스템 메시지 + 문서 링크
    insert_system_message(conn, &main_conv_id,
        format!("Plan [{}]({}) approved after {} review rounds — verdict=pass",
                plan_title, plan_document_path, rounds))?;

    // 3) branch.mode='design_review' 를 closed 로 mark (소프트 hide)
    conn.execute("UPDATE branches SET archived=1 WHERE id=?1", [branch_id])?;

    // 4) plan frontmatter 에 review 메타 주입 (선택)
    // design_reviewed_at: <timestamp>, review_rounds: <n>, reviewer_engine: codex
    app.emit("design_review_adopted", ...)?;
    Ok(())
}
```

**문서 경로 결정**: Architect 가 plan 산출 시 이미 `docs/plans/<slug>.md` 로 저장. PlanProposalCard payload 에 `plan_document_path` 필드 포함해야 함 — 이 필드가 현재 PlanProposalCard 에 있는지 여부가 **Open Question Q-1**.

### 7. Transcript auto-append

각 라운드 종료 시 reviewer JSON 응답을 markdown 으로 렌더 후 plan 문서 말미에 append:

```markdown
---
## Codex Review (Round 3 — 2026-04-22)

### Invariant checks
| INV | status | evidence |
|---|---|---|
| INV-1 | pass | … |

### Scores
plan_coverage: 4/5 · code_quality: 4/5 · test_coverage: 3/5 · convention: 4/5

### Findings
- [MINOR] …

### Verdict
pass — MINOR only
```

Collapsible 포맷은 markdown `<details>` 권장 (긴 transcript 가 plan 본문 가독성 해치지 않게).

---

## Invariants

- **[INV-1]** Design review RT 는 사용자 명시 클릭 (`[RT 검토 먼저]` 버튼) 으로만 발동한다. plan proposal 을 DB 에 기록하는 경로 / plan 자동 생성 경로 / 다른 RT 세션 / 임의 자동화 규칙 engine 등 **어떤 대체 경로로도 design_review 모드가 자동 시작되지 않는다**. **이유**: 사용자 주권 유지 + 토큰 비용 통제 + "모든 plan 이 RT 를 요구하진 않음" 사용자 명시 방침. **검증**: `grep "design_review" src-tauri/src -r` 결과가 UI 클릭 경로 (new_branch + mode=design_review) 1 곳만 트리거 진입점을 형성함을 확인. 테스트 — 자동 생성된 plan (예: metaAgent 등) 이 design_review 로 흐르는지 negative test.

- **[INV-2]** verdict=`fail` + BLOCKER finding 이 1건 이상 존재하는 상태에서는 PlanProposalCard 의 "바로 승인 → 구현" 버튼이 **비활성화** 된다. 강제 승인은 별도 모달 (사용자 2-step confirm) 을 거쳐야만 가능하며, 강제 승인 이력은 plan frontmatter `force_approved_at_round` 메타에 기록된다. **이유**: BLOCKER 가 잡힌 설계를 한 번의 클릭으로 지나갈 수 없게 하는 안전망. **검증**: Frontend unit test — reviewer verdict=fail + findings.some(f => f.severity === 'BLOCKER') 시 disabled=true assertion.

- **[INV-3]** RT 라운드는 **최대 3회**. 3회 도달 시 자동으로 `escalate_to_human` 상태로 전환되고, Codex 추가 호출은 차단된다. Divergence detector (harness Phase 2) 가 동일 finding category 를 2 라운드 연속 감지하면 round 3 도달 전이라도 즉시 escalate. **이유**: 무한 loop 방지 + doom-loop 비용 차단. **검증**: Integration test — reviewer 가 항상 fail 반환하는 mock 으로 4번째 호출 시도 시 차단 에러.

- **[INV-4]** `branch.mode='design_review'` 브랜치는 **shadow conversation** 이며, main conversation 에 reviewer transcript 를 직접 주입하지 않는다. main 에 주입되는 것은 adopt 시점의 "plan 승인 완료 + 문서 링크" 시스템 메시지 1건뿐. reviewer 의 상세 출력은 plan 문서 말미에만 append. **이유**: main 대화가 reviewer 의 세부 문장으로 오염되지 않음 — 사용자가 원할 때만 plan 문서를 펼쳐서 봄 (이번 세션 Part 2 patterns 과 동일). **검증**: Integration test — RT 세션 종료 후 main conversation 의 메시지 diff 가 1건 (시스템 메시지) 만 증가했는지.

- **[INV-5]** 사용자가 RT 진행 중 "취소" 를 클릭하면 design_review branch 는 archived 상태가 되고, **원본 plan 문서는 변경되지 않는다** (transcript append 없음). 중간 reviewer 응답은 branch 의 shadow conversation 메시지로만 남음 — 기존 branch drop 규칙과 동일 (stash 보존, 원본 보호). **이유**: 취소는 결정 권한을 사용자에게 돌려주는 경로이므로 irreversible 부작용 금지. **검증**: UI test — 취소 버튼 클릭 후 plan 문서의 md5 해시가 변경되지 않음을 확인.

---

## Rationale (reviewer-only)

### 왜 자동 트리거 엔진 대신 사용자 버튼인가

초기 구상은 heuristic 기반 자동 트리거 (INV 수 / subtask 수 / 영향 경로 grep) 였으나, 사용자 지적으로 방향 전환:
- 자동 규칙은 "모든 plan 을 검증" 쪽으로 drift 하기 쉽고, 이는 사용자 방침 "체감 개선으로 토큰 낭비 금지" 와 충돌.
- 간단한 UI fix / 문서 교정에 Codex 호출이 붙으면 marginal cost 커짐.
- 사용자가 상황 (긴급도, plan 규모, 자신감 수준) 에 맞게 1-click 결정하는 편이 **정보가 가장 많은 주체가 결정권을 갖는** 패턴.
- heuristic 은 **힌트 pill** 로 남겨 "참고" 수준 제공 → 사용자 판단 보조.

### 왜 기존 Branch/RT 를 재활용하는가

tunaFlow 는 이미 정교한 Branch/RT 인프라를 보유:
- shadow conversation 분기 (branches.rs)
- deliberative 모드 (roundtable_helpers/)
- RT 드로어 UI
- role_guidance + invariant_checks + regression_check (harness Phase 1·2 구현됨)
- tool-request 마커 체계 (reviewer 가 코드 조회)

design_review 를 별도 시스템으로 만들면 **동일 개념의 두 번째 구현** 이 생기고, 유지보수 부채가 2배. `branch.mode` 에 값 하나 추가 + reviewer guidance 1 variant 추가 = 최소 침습 경로.

### 왜 reviewer 엔진이 Codex 고정 (기본) 인가

- Architect 가 Opus → blind 확보를 위해 **다른 vendor** 필요.
- Gemini 도 사용 가능하나 tunaFlow 의 기존 사용 비중 (Codex > Gemini) 에 맞춤.
- Settings 에서 변경 가능하도록 유연성만 남김.
- Anthropic 간 Sonnet/Haiku reviewer 는 "vendor 1개 의존성 증가" → 기각.

### 대안 비교

| 대안 | 판정 | 사유 |
|---|---|---|
| 자동 heuristic 트리거 규칙 엔진 | 기각 | 사용자 주권 침해 + 룰 drift 리스크 |
| 별도 design-review 시스템 (branch/RT 미사용) | 기각 | 중복 구현, 유지보수 2배 |
| reviewer 엔진을 Opus 로 — Architect 와 동일 | 기각 | blind 확보 불가능 |
| 라운드 상한 없음 | 기각 | doom loop |
| Plan adopt = 기존 메시지 adopt 재활용 | 기각 | plan 은 문서, 메시지 아님. 정보 loss |
| **채택** (UI 2버튼 + branch.mode=design_review + Codex reviewer + 문서 adopt) | ✅ | 최소 침습, 사용자 주권, 기존 인프라 재활용 |

### Open questions

1. **Q-1 (PlanProposalCard payload 의 `plan_document_path`)**: 현재 PlanProposalCard 가 plan 문서 파일 경로를 prop 으로 받는지 확인 필요. 받지 않는다면 plan 생성 단계 (Architect 응답 파싱) 에서 path 추출 + payload 주입 로직 추가 필요 — 이는 Subtask 01 scope 초과 여지. Developer 가 기존 prop 구조 확인 후 결정.

2. **Q-2 (reviewer 엔진 기본값)**: 초기 릴리스는 Codex 고정. Gemini fallback 은 codex 미설치 환경에서만. 사용자 Settings 에 engine 선택 UI 를 즉시 추가할지 후속 PR 로 미룰지 — 본 plan 은 "Codex 기본 + Settings UI 는 옵션" 으로 두고 Developer 판단에 맡김.

3. **Q-3 (divergence detector 재활용)**: harness Phase 2 의 `regression_check` + divergence detector 가 design_review RT 에도 동일하게 동작하는지. 현재 dev_review RT 용으로 구현됐을 가능성 — mode 분기 또는 공통 helper 확인 필요.

4. **Q-4 (라운드 간 plan 문서 업데이트 정책)**: Architect 가 round N 에서 plan 을 수정하면 그 수정본은 round N+1 의 reviewer 입력이 됨. 수정본을 plan 문서에 즉시 write 할지, round 내 임시 버퍼에 둘지. 본 plan 은 "즉시 write + `## Codex Review (Round N)` append 는 round 종료 시" 를 권장하나 Developer 판단 가능.

5. **Q-5 (metaAgent 자동 plan 생성과의 충돌)**: 향후 metaAgent 가 plan 을 자동 생성하는 경로 (P0 metaAgentPlan) 와 본 design review gate 가 상호작용. 자동 생성 plan 은 사용자가 RT 를 선택할 기회가 있어야 하므로 PlanProposalCard UI 를 반드시 경유 — 이는 INV-1 로 강제.

### 측정 지표 (Developer 구현 후 수집)

- design_review RT 발동률 (사용자가 버튼 누른 비율)
- 평균 라운드 수 (1~3)
- 라운드당 잡힌 BLOCKER / MAJOR / MINOR 수
- `force_approved_at_round=3` 사례 수 (이건 적을수록 좋음)
- dev review 단계에서 design review 가 놓친 blocker 발견 수 (design review false negative)

이번 세션의 Search Phase C Part 2 (3 라운드, BLOCKER 4 + MAJOR 1 + MINOR 3) 를 baseline reference 로 사용.

---

## Subtask 구조

| # | 파일 | 범위 | 의존 |
|---|---|---|---|
| 01 | [-task-01.md](./designReviewGatePlan-task-01.md) | PlanProposalCard 2버튼 + 힌트 pill + Zustand 상태 + RT 드로어 mode 구분 표시 | — |
| 02 | [-task-02.md](./designReviewGatePlan-task-02.md) | Backend: `branch.mode='design_review'` + reviewer role guidance 확장 + Codex 호출 경로 + 라운드 상한 + escalate | 01 |
| 03 | [-task-03.md](./designReviewGatePlan-task-03.md) | Plan document adopt (문서 머지) + Transcript auto-append + Settings UI (reviewer engine 선택) | 02 |

3 subtask. 각 독립 PR 가능하나 01 없이는 02 가 사용자 경로 없이 dead code — 01 → 02 순서 권장.

---

## 관련 문서

- 본 gate 를 **거친 실제 사례**: `docs/plans/searchPipelineFromSecallPlan-part2.md` (Codex 3 라운드 transcript 가 본문 말미에 append 된 예시). 이번 세션에서 손으로 수행한 workflow 를 본 plan 이 제품화.
- harness 규약: `docs/plans/harnessVerificationGapPlan.md` §2 (invariants) + §3 (divergence/regression) + §5 (proposer 4-section)
- Branch/RT 인프라: `docs/plans/threadModelRoundtableRedesign.md`
- metaAgent 상호작용: `docs/plans/metaAgentPlan.md` (P0, 본 plan 의 INV-1 로 자동 트리거 금지 명시)
