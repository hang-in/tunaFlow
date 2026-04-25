---
title: User intent SSOT surfacing — architect 진입 시 사용자 의도 자동 lookup (메타 레벨)
status: implemented (Layer 1~4 — 2026-04-25)
priority: P2 (메타 — 같은 mismatch 영구 차단)
created_at: 2026-04-25
related:
  - docs/plans/branchInheritsMainSessionPlan_2026-04-25.md  # Task A, 본 plan 의 트리거 사례
  - docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md  # SSOT-first 메모리 철학
  - docs/reference/dataModelRevised.md  # conversation DB SSOT
canonical: true
owners:
  - architect (본 plan 작성)
  - developer (구현)
---

# 메타 문제

**사용자가 의도를 명시해도 다음 architect 세션이 그 의도를 못 봄** → 코드/문서 mismatch 누적. 본 plan 은 그 패턴 자체를 차단.

## 트리거 사례 (Task A)

2026-04-17 raw conversation log (037bb82f-...jsonl):

> 브랜치는 ws모드로 입장하는데 ... **컨텍스트팩에 모든걸 넣지말고 ... 검색할 수 있는 대규모의 컨텍스트 저장소(첫대화부터 직전대화까지 모두 원본으로 저장되어있는)** 가 있잖아?

이 의도가 8일간 코드/plan 으로 옮겨지지 않음. s36 sdk-url WS 작업 진행 중 brand session 통합이 별 task 로 띄워지지 않음. s37/s38 architect 들이 raw log 의도 surface 못함. 결과: 사용자가 오늘 다시 explain.

## 사용자 명시 (오늘 2026-04-25)

> 튜나플로는 이미 conversation DB(sqlite) 가 있잖아

→ 외부 secall 등이 아니라 **tunaFlow 자체 SQLite DB 를 architect 진입 시 자동 surface 활용** 하는 게 정석. 인프라는 이미 있음, layer 만 추가.

## tunaFlow 가 풀려는 메타 문제와 동일

tunaFlow 의 핵심 가치 = "AI 세션 가르면 맥락 잃음" 문제 해결. 그런데 본 프로젝트 자체에서 그 패턴이 발현. 자기 도구로 자기 의도를 surface 하는 dogfooding 이 본 plan 의 본질.

# 현재 상태

## (A) tunaFlow conversation DB

이미 SQLite 에 user / assistant 메시지 raw 저장. `messages_fts` (FTS5), `conversation_chunks` (vector via sqlite-vec), `compressed_memory`, `cross_session_links` 인프라 모두 존재.

## (B) ContextPack retrieval (현재)

`context_queries.rs:retrieve_relevant_chunks_with_overlap` (FTS5+vector) 가 작동 중. 하지만:
- **사용자 메시지 (role=user) 만의 weight 없음** — assistant 응답이 더 정렬 우위
- **architect persona 의 작업 주제와 키워드 expansion 없음** — 매칭 약함
- **명시적 "사용자 의도 SSOT" 섹션 없음** — ContextPack 에서 일반 retrieval chunk 와 섞임

## (C) Architect persona

`docs/agents/architect.md` 에 "이전 사용자 메시지 자동 lookup" 절차 없음. 작업 시작 시 자동으로 의도 surface 안 함.

# 설계 가설

## Layer 1 — Architect ContextPack 의도 섹션

ContextPack 빌더에 새 섹션 추가:

```
[USER_INTENT_LOOKUP]
- (2026-04-17) 브랜치는 ws모드로 입장 ... 컨텍스트팩 낭비
- (2026-04-25) 브랜치는 메인에서 바로 이어지는 건데 ContextPack 올리면 낭비
- (2026-04-25) DB(sqlite)가 있잖아 → 자체 검색 활용
[/USER_INTENT_LOOKUP]
```

- 작업 주제 추출: 현재 prompt + active plan/issue 에서 키워드 (예: "branch session", "context pack waste")
- 사용자 메시지 (role=user) 만 대상 + 키워드 매칭 + recency boost
- top N (~5) inline. 길이 cap (각 ~200 char)

## Layer 2 — 의도 키워드 추출 + 매칭

- 작업 주제 → 키워드 expansion: synonyms (한국어/영어 mix), 도메인별 (예: "session" → "세션, --resume, continuation")
- FTS5 + vector 매칭 (이미 존재 인프라)
- role=user 필터 (`m.role = 'user'`) + recency boost (timestamp 가중치)
- cross-conversation 매칭 (project 내 모든 conv) — 사용자가 어느 세션에 적었든 surface

## Layer 3 — Trace + Debug

- ContextPack trace 에 `intent_lookup` 섹션 (몇 개 매칭, 어떤 키워드, 점수)
- 미매칭 시 architect persona 가 사용자에게 명시적 질문 권장 (현재 작업과 부합하는 의도가 있는지)

## Layer 4 — Architect persona update

`docs/agents/architect.md` 에 명시:
- "작업 시작 시 [USER_INTENT_LOOKUP] 섹션 우선 검토"
- "의도와 현재 코드/문서 mismatch 감지 시 사용자에게 즉시 보고"

## Layer 5 — 외부 raw log 통합 (옵션, future)

`~/.claude/projects/-Users-d9ng-privateProject-tunaFlow/*.jsonl` 도 인덱싱. tunaFlow 가 자체 conversation DB 외에 Claude Code 의 raw log 까지 cover → 진정한 cross-session 의도 surface. 단, 별 plan 으로 분리 (큰 작업).

# Invariants

- **[INV-1]** Architect persona 의 ContextPack 에 [USER_INTENT_LOOKUP] 섹션이 항상 포함 (매칭 0 건이어도 빈 섹션으로). 검증: trace_log 의 ctx_sections 에 intent_lookup 출현 100%
- **[INV-2]** 사용자 메시지 (role=user) 만 매칭 대상. 검증: SQL 의 `WHERE m.role = 'user'` 명시
- **[INV-3]** raw 메시지 (DB SSOT) 가 truncate 없이 매칭 대상. 검증: messages 테이블 그대로 사용
- **[INV-4]** Cross-conversation 매칭 enabled (project 내 모든 conv). 검증: 다른 conv 의 사용자 메시지가 hit 하는 unit test
- **[INV-5]** Recency boost 적용 — 최근 사용자 메시지가 우위. 검증: 같은 키워드 매칭 시 timestamp DESC 정렬

# Developer 핸드오프 프롬프트

```
[작업] User intent SSOT surfacing — architect 진입 시 사용자 의도 자동 lookup (Plan userIntentSsotSurfacing)

[SSOT] docs/plans/userIntentSsotSurfacingPlan_2026-04-25.md 먼저 읽고 §설계 가설 (Layer 1~4) 순서대로 처리.

[배경 3줄]
- tunaFlow 가 풀려는 메타 문제 (세션 가르면 맥락 잃음) 가 본 프로젝트 자체에서 발현
- 사용자 의도가 raw log 에 박혀있어도 architect 가 surface 못해 mismatch 누적
- conversation DB + FTS5 + vector 인프라는 이미 있음 — layer 만 추가

[수정 범위]

1) Layer 1 — ContextPack [USER_INTENT_LOOKUP] 섹션:
   - prompt_assembly.rs 에 새 섹션 builder
   - architect persona 진입 시 항상 inline (매칭 0건이어도 빈 섹션)
   - top 5, 각 ~200 char cap

2) Layer 2 — 키워드 추출 + 매칭:
   - 작업 주제 추출: 현재 prompt + active plan/issue 키워드
   - synonym expansion (한/영 mix)
   - role=user 필터 + recency boost + cross-conv

3) Layer 3 — Trace:
   - trace_log.ctx_sections 에 intent_lookup 섹션 추가
   - 매칭 점수, 키워드 expansion 결과 보존

4) Layer 4 — Architect persona update:
   - docs/agents/architect.md 에 의도 lookup 절차 명시
   - mismatch 감지 시 사용자 보고 의무

5) 테스트:
   - 신규 unit test (FTS5 매칭 + role 필터 + cross-conv)
   - 수동: brand session 같은 과거 의도 키워드로 매칭 확인

[검증]
- cargo check / cargo test --lib
- 수동: architect 진입 + branch 관련 작업 → ContextPack 에 brand=ws 의도 자동 inline 확인

[셀프 이슈]
"feat: surface user intent in architect ContextPack from conversation SSOT"
이슈 본문에 메타 문제 + Task A 사례 인용
```

# 셀프 이슈 본문 (gh issue create 용)

```markdown
## Summary

Architect sessions repeatedly fail to surface previously expressed user intent because the conversation DB (SQLite) — which already holds raw user messages — is not specifically retrieved into the architect's ContextPack. This creates persistent mismatches between user intent and code/docs.

Concrete trigger: `branchInheritsMainSessionPlan_2026-04-25` (Task A). User intent ("brand inherits main session") was explicit in raw conversation log on 2026-04-17, but no architect session moved it into a plan/task for 8 days.

## Proposed solution

Add a `[USER_INTENT_LOOKUP]` section to the architect ContextPack:

- Extract task subject keywords from current prompt + active plan
- Match against `messages` (role='user', cross-conversation, recency boosted) via FTS5 + vector
- Inline top 5 matches with citation

Existing infra (FTS5, vector, conversations DB) is already in place. Only the section builder + architect persona update + trace logging are new.

## Why this matters

This is the meta-problem tunaFlow is built to solve, manifesting in tunaFlow itself. Closing this gap = dogfooding the core value proposition.

## Plan

`docs/plans/userIntentSsotSurfacingPlan_2026-04-25.md`. 4 layers (ContextPack section / keyword matching / trace / persona update).
```

# 후속 / Sibling

- **`branchInheritsMainSessionPlan_2026-04-25`** (Task A) — 본 plan 의 트리거 사례 + 즉시 fix
- **`longTermMemoryRoadmapPlan_2026-03-30`** — 본 plan 은 그 roadmap 의 Layer 4 (Long-term Memory) 의 구체 구현 중 하나
- **External raw log 통합** (future) — `~/.claude/projects/*.jsonl` 인덱싱. 별 plan
