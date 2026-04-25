---
title: Multi-Developer active plan 격리 — 옵션 결정 audit
status: decided
priority: P1
created_at: 2026-04-25
canonical: true
related:
  - docs/plans/multiDeveloperActivePlanIsolationPlan_2026-04-25.md  # SSOT
  - docs/plans/branchInheritsMainSessionPlan_2026-04-25.md  # brand session 재사용
  - src-tauri/src/commands/agents_helpers/send_common/context_loading.rs
  - src-tauri/src/commands/agents_helpers/context_pack/db_queries.rs
---

# Audit 결과 (Step 1)

plan 의 옵션 A/B/C/D 를 코드베이스 현황과 대조 검증한 결과, **plan 의 권장(A+B)** 은 의도는 정확하나 옵션 A 의 정확한 의미를 정정해야 한다. 결론은 **A′ + B 조합**.

## tunaFlow 데이터 모델 사실관계

확인한 사실:

1. `plans` 테이블의 active 상태는 `(conversation_id, status='active')` 한 자리. 같은 conv 안에서 동시 active plan 1개라는 가정이 백엔드 전체에 박혀있다 (`build_plan_section` SQL 의 `LIMIT 1`).
2. plan 1개는 **자기 implementation brand 와 review brand 를 갖고 있다** (`plans.implementation_branch_id` / `review_branch_id`). 즉 Developer/Reviewer 는 main conv 가 아니라 **각 plan 별 shadow conv (`branch:<id>`)** 에서 일하도록 설계된 모델이다.
3. 그러나 ContextPack 의 plan 섹션 빌드는 `resolve_plan_conversation_id(branch:* → main conv)` 로 root 로 거슬러 올라가 main conv 의 active plan 을 그대로 가져온다. 이 때문에 brand 안에서도 main conv 가 가진 1자리 active plan 이 그대로 노출되며, 다른 plan 의 brand 라 해도 같은 플랜만 보이는 게 아니라 **main 에서 마지막으로 활성화된 plan** 이 보인다.

즉 사용자 보고 케이스 (Coder Claude → readme-memento, Codex → Role Adapter Phase 1) 의 원인은:
- 두 Developer 모두 "main conv" 에서 직접 호출됐고 (brand 격리 단계 자체가 사용 안 됨), 또는
- 두 Developer 가 각자의 brand 에 들어가도 ContextPack 이 main conv 의 active plan 1자리를 그대로 inject 해 다른 plan 으로 오염됐음.

후자가 핵심이다. 이미 brand 라는 격리 단위가 존재하는데 ContextPack 빌더가 그것을 활용하지 않는다.

## 옵션 재평가

| 옵션 | 평가 |
|---|---|
| A (자동 sub-conv 격리) | UI/스토어/DB 영향 큼. 그런데 tunaFlow 는 이미 plan-당-brand 모델이 있어 신규 sub-conv 가 아니라 **기존 brand 매핑을 ContextPack 에 적용**하면 같은 격리 효과. 이게 옵션 A 의 본래 의도 (격리) 에 더 부합. → **A′** 로 변형 채택. |
| B (ContextPack sender 명시) | 변경 표면 작음. main conv 에서 직접 일하는 Architect 사용자 시나리오 (brand 진입 X) 에서도 sender + plan title 명시로 LLM robustness 보강. → 채택. |
| C (DB 매핑 전면 개편) | 기존 `plans.status='active'` SSOT 가 잘 자리 잡혀 있고, A′ 가 DB 변경 없이 같은 격리 효과. → **부결.** |
| D (parser 자동 plan 전환) | parser 정확도 의존. A′+B 로 INV 커버되면 불필요. → P3 future, 본 PR 제외. |

## 채택 — A′ + B

### Layer A′ — brand 기반 plan filter

`build_plan_section` / `load_context_data` 의 has_active_plan, plan_document 빌드 모두 **현재 conv 가 brand 면, `branches.id == implementation_branch_id || review_branch_id` 인 plan 만 surface** 한다.

- **Architect (main conv 또는 일반 brand 진입, plan 매칭 X)**: 기존 동작 그대로 — main conv 의 active plan 표시
- **Developer (impl brand 진입)**: 해당 brand_id 와 매칭되는 plan 만 노출 — 다른 plan 진행 중이라도 영향 X
- **Reviewer (review brand 진입)**: 해당 brand_id 와 매칭되는 plan 만 노출

이 변경은 데이터 모델 변경이 아닌 **lookup 정확성 향상**. brand 가 곧 plan 격리 boundary 라는 본래 설계를 ContextPack 에 일관되게 적용한다. Layer B (branchInheritsMainSession) 의 dynamic 섹션 drop 정책과도 충돌하지 않는다 (plan 섹션은 static 분류로 유지된다).

### Layer B — ContextPack sender 명시

active plan section 의 첫 줄에 sender 정보를 inline:

```markdown
## Active Plan (phase: implementation)

> **Sender**: developer (codex / gpt-5)
> **Plan**: Role Adapter Phase 1
```

이를 통해 같은 conv 에서 직접 (brand 진입 없이) 다른 Developer 를 호출하는 케이스 (사용자가 의도적으로 같은 conv 모드 선택 또는 brand 매핑이 아직 없는 단계) 에서도 LLM 이 "지금 메시지를 받은 Developer 와 plan 정합" 을 inline 으로 인지.

sender 정보 출처:
- **engine**: `prepare_engine_run` 의 `engine_key`
- **persona**: `SendWithClaudeInput.persona_label`
- **agent role**: `resolve_agent_role(conn, conversation_id)` (architect/developer/reviewer)

이 셋을 한 줄로 묶어 plan section 헤더에 inject. plan 이 없는 send 에는 영향 없음 (본 줄은 plan section 안에서만 출력).

## INV 만족 매핑

- **INV-1** (자동 격리 OR sender 명시) — A′ + B 모두 활성. brand 진입 시 A′ 격리, plan 매칭 안 되더라도 B 가 sender 명시.
- **INV-2** (다른 Developer plan 미주입) — A′ 가 brand 별 plan 만 노출해 직접 만족.
- **INV-3** (override = 같은 conv 모드) — main conv 에서 직접 호출은 기존 동작이 곧 same-conv 모드. B 만 활성. 별도 toggle 불필요.
- **INV-4** (모델별 차이 무관) — A′ 가 ContextPack 자체의 데이터를 격리하므로 Codex/Claude 의 instruction-following 차이가 영향 없음.

## 회귀 0 검증 포인트

1. main conv 에서 1 Developer + 1 active plan: 기존 동작 (main 의 active plan 그대로). ✅
2. brand 매핑 없는 일반 brand 진입 (사용자가 임의로 만든 branch): main 의 active plan fallback. plan 매칭이 안 되면 main conv plan 으로. ✅
3. `plans.implementation_branch_id` 미설정인 옛 plan (v26 이전): branch_id 매칭 자체가 0 → fallback 으로 main conv plan. ✅

## Override 옵션 toggle (Settings) — 본 PR 에서 보류

plan §INV-3 의 "두 Developer 같은 conv 강제" override 는 별도 UI toggle 로 명시할 수 있으나, 위 매핑처럼 main conv 에서 직접 호출 = same-conv 모드가 자연스러운 default. 현재 시점 별도 toggle 추가는 표면 확대 대비 효과 미미 → **본 PR 에서 보류**, 사용자 요청 발생 시 follow-up.

## Out-of-scope (N-A 메모)

- 옵션 C (DB 매핑 변경): 본 PR 비대상. 향후 multi-Developer 동시 active 가 1급 시민이 될 때 (P3 future).
- 옵션 D (parser): 본 PR 비대상.
- 새 sub-conv 자동 생성 + UI 라우팅: 본 PR 비대상. brand-기반 격리가 같은 효과를 더 작은 표면으로 제공.
