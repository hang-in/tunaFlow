---
title: result/insight 보고 문서의 tunaflow 마커 잔존 정리 (C-2 / B-16)
status: ready-to-implement
priority: P1
created_at: 2026-04-24
related:
  - docs/plans/postBetaBacklogPlan_2026-04-24.md  # B-16
  - docs/plans/insightStabilityPlan.md
  - docs/posts/10-메타에이전트.md  # 재발 기록
canonical: true
owners:
  - architect (본 문서 작성)
  - developer (구현)
---

# 개요

`<!-- tunaflow:* -->`, `<!-- subtask-done:N -->`, `<!-- impl-complete -->` 류 내부 마커가 **사용자 가시 산출물**(프로젝트 `docs/plans/*-result.md`, `docs/insight/findings/*.md`, `docs/insight/latest-report.md`) 에 그대로 섞여 나가는 회귀가 관찰됨.

1 회차 수정은 `syncResultReport` 안에서 로컬 `stripMarkers` 를 만들어 처리했으나, 함수 내부 closure 라 **insight 자동 export 경로** 에서 다시 새지는 현상이 10 편 기록에 재등장. 재발 원인 = 스크럽 로직이 공용 유틸이 아니라 한 함수 안에만 있음.

이 plan 은 **"마커 스크럽 = 전역 단일 유틸 + DB 쓰기 시점에 적용"** 원칙으로 일원화하고 재발을 막는다.

# 현재 상태 (사실 확인)

## (A) `src/lib/workflow/reportSync.ts`

- `stripMarkers = (text) => ...` 가 **line 47, `syncResultReport` 안에** 정의됨
- 같은 파일의 `syncPlanDocument` (line 10), `syncReviewReport` (line 19) 는 마커 스크럽 **안 함**
  - Plan 문서는 `planApi.generatePlanDocument` 가 DB plan 레코드로 생성 (LLM 생 텍스트 경유 없음) → 실무상 마커 없을 가능성 높음
  - Review 문서는 `verdict.findings / recommendations` 파서 결과 (planProposalParser → reviewer 응답 marker 추출 후 **payload 만 넘김**) → 마커 없을 가능성 높음
  - 다만 안전망 관점에서 동일 유틸을 모두 통과시키는 편이 낫다
- `syncResultReport` 내부 call site 2 개는 이미 `stripMarkers` 적용 중 (line 66, 73)

## (B) Insight 자동 export — 별도 경로 (마커 스크럽 없음)

1. `src/lib/insightOrchestration.ts:260` — 마커 파싱 실패 시 **LLM 원본 response** 를 `createInsightReport(..., response.slice(0, 5000), ...)` 로 DB 에 그대로 저장. 원본 텍스트 안에 마커 섞여 있으면 DB 보관.
2. `src/lib/insightOrchestration.ts:273, 308` — `parsed.summary` 를 그대로 `createInsightReport` 에 넣음. LLM 이 summary 문자열에 마커를 포함해 반환하면 DB 오염.
3. `src/lib/insightOrchestration.ts:286-300` — `findingInputs.description` 은 `f.description + f.evidence`, `snippet` 은 `f.snippet`. JSON marker payload 안에 들어있는 값이지만 LLM 이 description 자체에 마커 인라인으로 써 놓는 케이스 관찰됨 (기록 출처: 10 편).
4. `src-tauri/src/commands/insight.rs:400` `export_insight_to_files` — DB 값을 그대로 `.md` 로 직렬화. 여기서 마커 있으면 파일에도 그대로 나감.

즉 **"DB 에 이미 마커가 들어간 값"** 이 Rust export 단에서 그대로 흘러나가는 구조. 수정 지점은 **DB 쓰기 직전** 이 제일 확실하다 (export 시점 스크럽은 기존 DB 값을 안 고치므로 추가 cleanup 가 필요해짐).

# 설계

## (1) 공용 유틸 분리 — `src/lib/workflow/markerScrub.ts` (신규)

```ts
// src/lib/workflow/markerScrub.ts
/**
 * tunaFlow 내부 마커 제거. 사용자 가시 산출물 (docs/plans/*.md, docs/insight/*.md)
 * 에 새지 않아야 하는 모든 write 경로에서 이 함수를 통과시킬 것.
 *
 * 대상 마커:
 *   <!-- tunaflow:TOKEN -->            (plan-proposal / insight-findings / etc.)
 *   <!-- tunaflow:TOKEN:NUM -->        (subtask-ref, etc.)
 *   <!-- subtask-done:N -->
 *   <!-- impl-complete -->
 *
 * 추가로 연속 공백 라인을 2개로 정규화.
 */
export function stripTunaflowMarkers(text: string): string {
  return text
    .replace(/<!--\s*tunaflow:[a-z_-]+(?::\d+)?\s*-->/g, "")
    .replace(/<!--\s*subtask-done:\d+\s*-->/g, "")
    .replace(/<!--\s*impl-complete\s*-->/g, "")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}
```

이름을 `stripTunaflowMarkers` 로 두어 **도메인 접두사 명확화** — 전역 "stripMarkers" 는 다른 곳에서도 쓰일 수 있는 범용 느낌이 강함.

## (2) `reportSync.ts` 리팩토링

- import 후 기존 `stripMarkers` 로컬 정의 삭제
- `syncResultReport` 내부 2 개 호출을 `stripTunaflowMarkers(...)` 로 치환
- `syncReviewReport` 는 **추가 처리 없음** (payload 는 파싱된 구조체라 마커 비포함이 정상) — 단 `testOutput` 이 있으면 그건 LLM/CI 텍스트일 수 있으므로 `stripTunaflowMarkers(testOutput)` 통과
- `syncPlanDocument` 는 Rust 단 DB plan→md 생성이라 FE 에서 스크럽 의미 없음 → 변경 안 함

## (3) Insight 쓰기 경로 — `insightOrchestration.ts` 스크럽 주입

**원칙: DB 에 들어가기 직전에 스크럽**. 그래야 후속 export/read 어디서도 다시 신경 쓸 필요 없다.

수정 지점 (각 함수 호출 인자 쪽):

| 라인 | 수정 전 | 수정 후 |
|---|---|---|
| 264 | `response.slice(0, 5000)` | `stripTunaflowMarkers(response).slice(0, 5000)` |
| 273, 308 | `parsed.summary` | `stripTunaflowMarkers(parsed.summary)` |
| 294 (description) | `` `${f.description}\n\n**Evidence**: \`${f.evidence}\`` `` 혹은 `f.description` | `stripTunaflowMarkers(f.description)` 먼저 감싼 뒤 evidence 병합 |
| 298 (snippet) | `f.snippet` | `f.snippet ? stripTunaflowMarkers(f.snippet) : undefined` |

import 추가: `import { stripTunaflowMarkers } from "./workflow/markerScrub";`

## (4) 2 차 안전망 — Rust `export_insight_to_files` (옵션 / 권장)

DB 레거시 오염분이 남아있을 가능성에 대비해 Rust 쪽에도 간단한 regex 스크럽을 추가. 이미 존재하는 오염분은 한 번 export 돌 때 자동 정화되는 효과도 있다.

- 파일: `src-tauri/src/commands/insight.rs`
- 의존성: `regex` crate 이미 Cargo.toml 에 있음 (line 49) → 추가 설치 불요
- 구조: 함수 상단에 `fn strip_tf_markers(s: &str) -> String` 하나 놓고, 다음 필드 출력 직전 통과시킴:
  - `f.description`, `f.snippet`, `f.resolution`
  - `session.summary`

Regex 패턴 (Rust regex 문법):
```rust
static MARKER_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"<!--\s*(?:tunaflow:[a-z_-]+(?::\d+)?|subtask-done:\d+|impl-complete)\s*-->").unwrap()
});
```

구현 참고: `regex` 는 이미 다른 곳에서 쓰고 있고 `once_cell::sync::Lazy` 대신 본 프로젝트는 `lazy_static` (Cargo.toml line 41) 를 쓰므로 컨벤션 맞춰 `lazy_static!` 로 작성해도 된다.

## (5) 테스트

### FE (vitest)

- 신규: `src/lib/workflow/markerScrub.test.ts`
  - 단일 마커 / 멀티 마커 / payload 붙은 마커 / 공백 정규화 4 케이스
- 회귀: `src/lib/workflow/reportSync.test.ts` (이미 있으면 보강, 없으면 생략 — grep 결과 미존재)

### Rust (cargo test --lib)

- `src-tauri/src/commands/insight.rs` 내부 `#[test] fn strip_tf_markers_*`
  - empty / no marker / one marker / mixed 3~4 케이스

## (6) grep 기반 최종 감사

수정 후 다음이 0 건이어야 함:
```bash
rg -n '<!-- tunaflow:|<!-- subtask-done:|<!-- impl-complete' \
  /Users/d9ng/privateProject/<테스트 프로젝트>/docs/plans/ \
  /Users/d9ng/privateProject/<테스트 프로젝트>/docs/insight/
```
+ 회귀 재현 방법: 테스트 프로젝트에 풀사이클 1회 돌려 새로 생성된 `docs/plans/*-result.md`, `docs/insight/latest-report.md` 에 마커 없는지 육안 확인.

---

# Developer 핸드오프 프롬프트

> 새 세션에 아래 blob 을 통째로 붙여넣기.

```
[작업] tunaFlow 내부 마커가 사용자 가시 산출물(docs/plans/*.md, docs/insight/*.md)에 새지 않도록 스크럽 로직 일원화 (C-2 / B-16)

[SSOT] docs/plans/resultReportMarkerCleanupPlan_2026-04-24.md 를 먼저 읽고, 아래 순서대로 처리.

[배경 요약]
- 1차 수정은 src/lib/workflow/reportSync.ts:47 의 `stripMarkers` 로컬 함수. syncResultReport 안에서만 동작.
- Insight 자동 export 경로 (insightOrchestration.ts → createInsightReport / createInsightFindingsBatch → Rust export_insight_to_files) 는 이 스크럽을 전혀 안 거침 → 재발.
- 해결: 공용 유틸로 뽑고, DB 쓰기 직전 스크럽. Rust 쪽 export 에도 안전망 하나 더.

[수정 범위]

1) 신규: src/lib/workflow/markerScrub.ts
   - stripTunaflowMarkers(text: string): string
   - 정규식: <!-- tunaflow:TOKEN(:NUM)? --> / <!-- subtask-done:N --> / <!-- impl-complete --> + \n{3,} -> \n\n + trim

2) 수정: src/lib/workflow/reportSync.ts
   - 파일 상단 import: import { stripTunaflowMarkers } from "./markerScrub";
   - 라인 47~52 의 로컬 stripMarkers 삭제
   - 라인 66 / 73 호출을 stripTunaflowMarkers(...) 로 치환
   - syncReviewReport: testOutput 파라미터가 있으면 그것도 스크럽 통과 (planApi.generateReviewReport 호출 직전)

3) 수정: src/lib/insightOrchestration.ts
   - import 추가: import { stripTunaflowMarkers } from "./workflow/markerScrub";
   - 라인 264: response.slice(0,5000) → stripTunaflowMarkers(response).slice(0,5000)
   - 라인 273, 308: parsed.summary → stripTunaflowMarkers(parsed.summary)
   - 라인 286~300 findingInputs map 블록:
     * description: f.evidence 있으면 stripTunaflowMarkers(f.description) 로 감싼 뒤 evidence 병합
                    없으면 stripTunaflowMarkers(f.description)
     * snippet: f.snippet ? stripTunaflowMarkers(f.snippet) : undefined

4) 수정(옵션, 강력 권장): src-tauri/src/commands/insight.rs
   - 파일 상단에 lazy_static!(static ref TF_MARKER_RE: regex::Regex = ...) 하나
   - fn strip_tf_markers(s: &str) -> String { TF_MARKER_RE.replace_all(s, "").to_string() ... 연속 빈 줄 정규화 }
   - export_insight_to_files 안에서 다음 필드 출력 직전 통과:
       f.description, f.snippet, f.resolution, session.summary
   - 패턴: r"<!--\s*(?:tunaflow:[a-z_-]+(?::\d+)?|subtask-done:\d+|impl-complete)\s*-->"

5) 테스트
   - 신규 src/lib/workflow/markerScrub.test.ts — 4 케이스 최소:
       * "hello <!-- tunaflow:plan-proposal --> world" → "hello  world" (공백 정규화 후 trim)
       * "done\n\n\n\n<!-- subtask-done:3 -->\ndone" → "done\n\ndone"
       * "" → ""
       * marker 없음 → 원본
   - 신규 또는 기존 insight.rs 에 #[test] fn strip_tf_markers_* 2~3 케이스

[검증]
- npx tsc --noEmit: 0 에러
- npx vitest run: 신규 테스트 포함 전량 pass
- cd src-tauri && cargo test --lib: 신규 테스트 포함 전량 pass
- cd src-tauri && cargo check --all-targets: 0 에러
- 풀사이클 smoke (선택):
    1. 테스트 프로젝트에서 plan 1개 풀사이클 돌림 (Plan → Dev → Review → Done)
    2. docs/plans/<plan>-result.md, docs/plans/<plan>-review.md 에 "<!-- tunaflow:" / "<!-- subtask-done" / "<!-- impl-complete" grep 0 건 확인
    3. insight 분석 1회 돌림 → docs/insight/findings/*.md / docs/insight/latest-report.md 동일 grep 0 건 확인

[커밋]
- refactor(workflow): extract stripTunaflowMarkers to shared util
- fix(insight): scrub tunaflow markers at DB write time (prevent leak into exported files)
- chore(insight): Rust-side safety net for export_insight_to_files
- test: coverage for stripTunaflowMarkers + Rust equivalent

각 커밋 trailer 에 Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목]
fix(output): consolidate tunaflow marker scrubbing across result/insight export paths

[주의]
- git stash drop/clear 금지
- Rust 쪽은 lazy_static (Cargo.toml 에 이미 있음) 컨벤션 유지 — once_cell 새로 추가하지 말 것
- markerScrub.ts 는 DOM/React 의존 없어야 함 (워커/CLI 에서도 재사용 가능해야)
- insightOrchestration.ts 의 description evidence 병합 순서 유지 (먼저 스크럽 → 그 뒤 evidence 덧붙임)
```

# Invariants

- **[INV-1]** `docs/plans/*-result.md`, `docs/plans/*-review.md`, `docs/insight/findings/*.md`, `docs/insight/latest-report.md` 어느 파일에도 `<!-- tunaflow:` / `<!-- subtask-done:` / `<!-- impl-complete` 문자열이 포함되지 않는다. **검증**: rg 0 건.
- **[INV-2]** 마커 스크럽 로직은 `stripTunaflowMarkers` (FE) 와 `strip_tf_markers` (Rust) **두 군데** 만 존재한다. 다른 파일에 중복 정의 금지. **검증**: `rg 'function stripMarkers|fn strip_markers' src/` 결과 0.
- **[INV-3]** 새로 추가되는 "사용자 가시 산출물 쓰기 경로" 는 이 유틸을 반드시 통과시켜야 한다. (코드 리뷰 체크리스트에 포함)
- **[INV-4]** DB 에 이미 저장된 레거시 오염분은 Rust export 의 2 차 안전망이 런타임에 정화해준다 — 따로 DB 마이그레이션 불요.
- **[INV-5]** `parsed.findings` JSON 안의 description/snippet 이 스크럽을 거치면서 **유효 payload 가 잘리지 않아야** 한다 (regex 는 마커만 잡고 일반 텍스트는 그대로 둠을 테스트로 보증).

# Rationale

## 왜 공용 유틸로 뽑나
기존 구조는 "마커 스크럽 = `syncResultReport` 로컬 함수". 이게 원인이 되어 **insight 경로가 완전히 별도로 동작하면서 재발** 했음. 한 번 더 같은 패턴으로 가면 다음 write path 생겼을 때 또 샌다. 공용 유틸 하나 + 호출하는 경로 쪽에 "DB 쓰기 직전 통과" 규칙만 유지.

## 왜 DB 쓰기 시점이지 export 시점이 아닌가
- 현재 구조: **여러 경로에서 write, 하나 경로에서 export**. export 단에만 스크럽하면 write 마다 다른 마커 가진 값이 DB 에 퇴적됨.
- 결국 export 가 여러 경로로 분기하면 (Rust 외 FE export 포함) 다시 뚫림.
- 원칙: "DB 에는 깨끗한 문자열만" → 어떤 export 경로든 안전.
- Rust export 단 스크럽은 **레거시 정화용 2차 방어**. 1차는 write 시점.

## 왜 Rust 쪽에도 스크럽을 넣나
이미 DB 에 오염분이 들어있을 가능성 (지금까지의 insight 실행 기록). FE 만 고치면 **기존 DB 값은 영원히 안 지워짐**. 하지만 DB 마이그레이션으로 일괄 UPDATE 하기엔 스크럽 로직의 correctness 가 실전 검증되지 않음 → 1 회 잘못 치환 시 되돌릴 수 없는 파괴. 런타임 스크럽이 더 안전.

## 왜 `parsed.summary` 도 스크럽해야 하나
LLM 응답을 marker 로 감싸 달라고 지시했는데, 모델이 (특히 Gemini, Ollama small model) summary 필드 안에 또 marker 를 인라인으로 써 놓는 사례 관찰됨. JSON 파싱 성공해도 value 가 오염.

## 왜 `syncReviewReport` 에는 안 적용하나
`ParsedReviewVerdict` 는 이미 `planProposalParser` 에서 **marker 추출 후 payload 만 남긴 구조체**. findings/recommendations 는 string array 지만 element 단위로 parser 가 골라낸 내용 → 마커 들어갈 여지 극히 낮음. `testOutput` 만 예외 (LLM raw 일 가능성) → 여기만 스크럽.

# 스코프 외 (별도 plan)

- `reportSync.test.ts` 신규 테스트 작성 — 유닛 테스트 보강 plan 에 병합 가능
- DB 레거시 값 일괄 UPDATE (마이그레이션) — 필요성 의심스럽고 리스크 큼, 런타임 스크럽으로 충분
- Plan 문서 (`syncPlanDocument`) Rust 단 스크럽 — 현재 Rust 가 DB plan record 를 템플릿에 끼우는 구조. 템플릿 치환 단계에 FE 에서 만든 문자열이 흘러 들어가는 경로 없음. 필요 시 별도 감사.

# 관련 기록

- `docs/plans/postBetaBacklogPlan_2026-04-24.md` **B-16** — 같은 이슈 백로그 등재분. 본 plan 머지 시 해당 항목 "완료 처리" 표기.
- `docs/posts/10-메타에이전트.md` — 재발 기록 출처.
- `docs/plans/insightStabilityPlan.md` — insight 파이프라인 안정화 plan (본 건은 해당 plan 의 후속).
