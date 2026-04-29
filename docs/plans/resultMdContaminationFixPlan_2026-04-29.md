---
title: result.md contamination fix — reviewer 입력 격리 + truncation/self-include 가드
status: completed
phase: merged
owner: developer-handoff
created_at: 2026-04-29
updated_at: 2026-04-29
merged_at: 2026-04-29
merged_pr: 211
merge_commit: bc34b53
verification:
  frontend_tests: 381
  rust_tests: 559
canonical: true
related:
  - docs/prompts/resultMdContaminationFixDeveloperHandoff_2026-04-29.md
  - src-tauri/src/commands/agents_helpers/send_common/context_loading.rs
  - src/lib/workflow/reportSync.ts
  - src-tauri/src/commands/project_tools.rs
  - src/locales/ko/workflow.json
---

# result.md contamination fix

## 1. Problem

리뷰어 Codex 가 v0.1.4-beta 후속 review 라운드에서 다음 verdict 를 냈습니다.

> Task 01~03 의 코드/문서 변경은 모두 정상. 코드 결함 없음.
> 그러나 result.md 의 verification 블록이 잘리고 자기 자신을 포함하는 패턴이 보여 conditional.

REVIEWER_TEMPLATE 에는 **"Never judge `*-result.md` quality"** 가 두 번 명시되어 있는데도 (`commands/project_tools.rs:690, 705`) verdict 가 result.md 작성 상태로 결정되었습니다. 즉 **정책 위반 verdict** 가 발생했고, 그 빈도와 재현성을 낮춰야 합니다.

## 2. Root cause (3 layer)

### 2-1. Reviewer 입력에 result.md 강제 첨부 (root)

`src-tauri/src/commands/agents_helpers/send_common/context_loading.rs:670-676`:

```rust
// Result report (review/rework phase — Reviewer needs implementation context)
if phase == "review" || phase == "rework" {
    if let Ok(doc) = std::fs::read_to_string(plans_dir.join(format!("{}-result.md", slug))) {
        combined.push_str("\n\n---\n\n");
        combined.push_str(&doc);
    }
}
```

ContextPack 이 result.md 본문을 reviewer 입력에 무조건 첨부합니다. REVIEWER_TEMPLATE 의 "Never judge" 규칙과 **모순된 신호** — 입력으로는 주면서 판정 금지하라는 지시는 모델 혼동을 유발합니다.

### 2-2. Truncation slice (defense gap)

`src/lib/workflow/reportSync.ts:64, 71`:

```ts
const summary = assistantMsgs[last].content.slice(0, 2000);          // 2000자 cap
const subtaskResults = assistantMsgs.slice(-10)
  .map((m) => stripTunaflowMarkers(m.content.slice(0, 500)));         // 500자 cap
```

하드코딩 byte slice 가 잘림 패턴(verification 블록 중간 절단)의 발생기. UTF-8 char boundary 도 고려하지 않아 한글 깨짐 가능.

### 2-3. Self-include vector (rare)

`syncResultReport` 가 `assistantMsgs.slice(-10)` 으로 최근 10개 assistant 메시지를 subtask 인자로 사용. 이전 review 라운드에서 chat 에 인용/요약된 prior result.md 본문이 다음 호출 인자로 다시 들어가면 **재귀 포함**됩니다. ContextPack 에 result.md 본문이 reviewer 입력으로 들어와 있고 그게 다시 assistant 메시지로 echoed 되면 self-include 사이클 형성.

## 3. Goals / Non-goals

### Goals
- (G1) reviewer ContextPack 에서 result.md 본문 자동 첨부 제거. reviewer 는 task 파일 + 코드 + Plan 문서로 판정.
- (G2) reportSync 의 byte slice 를 UTF-8 boundary-safe + 더 큰 상한 + `[truncated …]` 마커로 교체.
- (G3) self-include guard: implMessages 에서 prior result.md 인용 메시지를 sentinel 기반으로 식별·제외.
- (G4) workflow review 메시지(i18n)에서 result.md 경로를 reviewer 동선에 노출하지 않음.

### Non-goals
- ❌ REVIEWER_TEMPLATE 본문 변경 (이미 정책 명시되어 있음. 입력 단 차단이 더 신뢰할 수 있는 fix).
- ❌ `generate_result_report` Rust 시그니처/파일 출력 형식 변경 (다른 consumer 영향, 아키텍트 영역).
- ❌ `syncResultReport` 호출 트리거 변경 (review 워크플로우 흐름 자체 영향).
- ❌ `syncPlanDocument` / `syncReviewReport` 변경 (해당 truncation 없음, scope 외).

## 4. Subtasks

### Task 01 — ContextPack 에서 result.md 자동 첨부 제거 [P0, root]

**Changed files**: `src-tauri/src/commands/agents_helpers/send_common/context_loading.rs`

**Change description**:
- 라인 670-676 의 `if phase == "review" || phase == "rework" { read result.md }` 블록을 **삭제**.
- 같은 분기 안의 task files 첨부 (라인 658-668) 와 latest review report 첨부 (rework only, 라인 678-690) 는 **그대로 유지** — 둘은 reviewer 가 합법적으로 사용해야 하는 입력.
- 주석으로 변경 이유 한 줄 (`// 2026-04-29: result.md is auto-generated; injecting it into reviewer ContextPack created policy-violation verdict pattern. Reviewer must judge from task specs + code only.`).

**Verification**:
- `cd src-tauri && cargo check --message-format=short`
- `cd src-tauri && cargo test --lib send_common` (해당 모듈 테스트가 있다면 통과 확인. 없으면 컴파일만 OK 면 통과 처리)
- 회귀 grep: `rg "phase == .review|phase == .rework" src-tauri/src/commands/agents_helpers/send_common/context_loading.rs` — 남은 두 분기(task files, review report) 가 여전히 동작하는지 라인 확인.

**회귀 위험 가드**:
- 같은 함수 내 다른 phase 분기(planning, dev) 분기 절대 건드리지 말 것.
- result.md 의 다른 read 경로(`commands/plans.rs:968` 의 write 만 있음) 는 변경 대상 아님.

---

### Task 02 — reportSync truncation 을 UTF-8 boundary-safe + larger cap + 마커로 교체 [P1]

**Changed files**: `src/lib/workflow/reportSync.ts`

**Change description**:
- 헬퍼 추가: `truncateSafe(text: string, limit: number): string` — `Array.from(text).slice(0, limit).join('')` 로 코드포인트 단위 자르기. 잘렸으면 `\n\n[…truncated, original ${origLen} chars]` 마커 append.
- summary 상한: `2000` → `8000`. `slice(0, 2000)` 호출 → `truncateSafe(text, 8000)`.
- subtaskResults 상한: 각 메시지 `500` → `2000`. `slice(0, 500)` 호출 → `truncateSafe(text, 2000)`.
- 함수 시그니처/호출처 변경 없음 (string 반환 동일).

**Verification**:
- `npx tsc --noEmit`
- `npx vitest run src/lib/workflow/` (해당 디렉토리 테스트 통과)
- 새 unit test 1개 추가 (`reportSync.test.ts` 또는 인접): `truncateSafe('가나다라마', 3) === '가나다[…truncated, original 5 chars]'` 같은 boundary 테스트.

**회귀 위험 가드**:
- `reportSync.ts` 의 `syncPlanDocument`, `syncReviewReport` 는 건드리지 말 것 — 이들은 truncation 패턴이 없고 scope 외.
- `syncResultReport` 의 lastReworkIdx 탐색 로직(라인 52-62) 변경 금지 — manualVerification.ts 와 동일 로직 약속이 있음.

---

### Task 03 — self-include guard (sentinel 기반) [P1]

**Changed files**: `src/lib/workflow/reportSync.ts`

**Change description**:
- `syncResultReport` 안에서 assistantMsgs filter 시점에 sentinel 패턴 매칭으로 prior result.md 인용 메시지 제외:
  - sentinel: 메시지 본문 첫 200자 안에 `# Implementation Result:` 헤더 OR `> Plan Revision:` 헤더 라인이 있으면 result.md echo 로 간주.
  - 단순 `result.md` 문자열 매칭은 **금지** (false positive).
- 제외된 메시지 카운트는 `console.debug("[syncResultReport] excluded {N} echoed result.md messages")` 로 로그만.

**Verification**:
- `npx tsc --noEmit`
- 새 unit test: assistantMsgs 에 sentinel 포함 메시지 + 일반 메시지 섞었을 때 sentinel 만 제외되고 일반은 유지되는지 검증.

**회귀 위험 가드**:
- sentinel 패턴은 두 헤더 모두(`# Implementation Result:` AND `> Plan Revision:`) 동시 출현 시에만 매칭 — 한쪽만 보면 false positive 위험.
- 정상 코드 설명에 result.md 를 단순 언급한 메시지는 제외되지 않아야 함 (테스트로 보장).

---

### Task 04 — review 메시지에서 result.md 경로 격하 [P2, i18n]

**Changed files**:
- `src/locales/ko/workflow.json`
- `src/locales/en/workflow.json` (있다면)

**Change description**:
- `workflow.json:233` 부근 review 메시지 `body` 에서 `- 결과: \`docs/plans/{{slug}}-result.md\`` 라인을 **삭제**.
- 같은 블록의 `- Plan: \`docs/plans/{{slug}}.md\`` 는 유지.
- 영문 locale 도 동일 정리.

**Verification**:
- 검색으로 잔여 확인: `rg "result.md" src/locales/`
- review RT 진입 시 chat 메시지가 정상 렌더링되는지 visual 확인 (수동, 스크린샷 1장).

**회귀 위험 가드**:
- 다른 키(예: settings.json:323 `artifact_hint`) 는 별개 UI 도움말이라 건드리지 말 것.
- workflow.json 의 다른 메시지 템플릿 변경 금지.

## 5. Cross-cutting risks

| 위험 | 대응 |
|---|---|
| reviewer 가 implementation context 를 못 봐서 false fail | task 파일 + 코드 + Plan 문서로 충분히 판정 가능. e2e dummy plan 1회 검증 후 false-fail 빈도 모니터. |
| truncation 상한 상향으로 ContextPack 토큰 폭증 | 8k summary + 2k×10 subtask = 최대 28k. 기존 Standard tier (60k) 안에서 안전. |
| sentinel 매칭 false positive 로 정상 메시지 누락 | 두 헤더 동시 매칭 + unit test 로 보장. 의심 시 false positive log 확인. |
| 한글/이모지 char boundary 깨짐 (Task 02) | `Array.from(text)` 로 코드포인트 단위 — surrogate pair 안전. 단 grapheme cluster (조합 한글, ZWJ) 는 깨질 수 있으나 result.md 내용에서 critical 이슈 아님. |

## 6. Rollback

각 task 단위로 git revert 가능. Task 01 (Rust) 만 우선 적용해도 root cause 차단 효과는 즉시 확인 가능 (defense-in-depth Task 02-03 은 후속 가능).

## 7. Out of plan (별도 추적)

- REVIEWER_TEMPLATE 의 "Never judge result.md" 강조 표현은 plan 의 non-goal. 입력 단 차단이 신뢰성 더 높음.
- result.md 가 git 추적 파일이라는 점 자체의 적절성 (auto-gen 산출물인데 추적되어 PR diff 에 노이즈) — 추후 별도 plan 후보.
