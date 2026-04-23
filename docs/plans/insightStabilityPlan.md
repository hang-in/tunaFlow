---
title: Insight Stability — rawq 크래시 / 카테고리 skip / 토큰 집계 / 스피너 전파 4 버그 직렬 수정
status: ready-to-implement
priority: P0 (베타 공개 blocker)
created_at: 2026-04-23
related:
  - _research/_util/rawq/src/search/engine.rs            # ① OOB slice (외부 repo)
  - src/lib/insightOrchestration.ts                       # ② skip 로직
  - src-tauri/src/agents/claude.rs                        # ③ stream JSON usage 스키마 drift
  - src-tauri/src/commands/insight_extract.rs             # ④ timeout → status 전이
  - src/components/tunaflow/context-panel/EvaluationPanel.tsx  # ④ UI 구독 (확인 대상)
  - src/components/tunaflow/InsightPanel.tsx              # ④ 스피너 종료 지점
  - docs/plans/publicReadinessChecklistPlan.md            # 병렬 진행 plan
---

# Insight Stability

## TL;DR

사용자가 2026-04-23 tunaReader 에 Insight 분석을 돌린 결과 **4개 버그가 직렬로 엮여 $0.8+ 비용 + 613초 무응답 + UI 스피너 무한** 현상 재현. 외부 사용자가 베타에서 같은 경로 밟으면 첫인상 심각히 훼손. 4개 모두 핀포인트 파일/라인 확정됨, 총 수정량 ~40줄, 반나절 작업.

## Problem — 직렬 연관도

```
① rawq engine.rs:998 OOB panic (stale chunk)
   └→ rawq daemon 프로세스 전체 크래시
        └→ 특정 카테고리 snippets = 0
             ↓
② insightOrchestration.ts:219 skip 조건 너무 관대 (OR)
   └→ snippets=0 인데 extraContext≥1 이면 분석 계속 실행
        └→ 증거 없는 상태로 "증거 기반" 프롬프트 호출
             └→ LLM extended thinking 폭주 → $0.9 / finding hallucination
                  ↓
③ claude.rs StreamLine 스키마 drift
   └→ total_input_tokens / total_output_tokens top-level 파싱 → 항상 None
        └→ input_tokens=0, output_tokens=0 표시 (cost 는 정확)
             ↓
④ agent-timeout(613s) pid kill 은 되지만 insight_runs.status 'failed' 전이 누락
   └→ InsightPanel 구독 이벤트 미도달 → 스피너 무한
        └→ 사용자는 앱 재시작 또는 DB 수동 UPDATE 필요
```

## Specification

### Subtask 01 — rawq `ctx_before` clamp

**파일**: `/Users/d9ng/privateProject/_research/_util/rawq/src/search/engine.rs:995-1001` (tunaFlow 레포 밖 부모 경로. upstream = `github.com/auyelbekov/rawq`, MIT)

**배포 전략 — 로컬 patch only (베타 최우선)**:
- 이 클론은 tunaFlow 레포에 포함되지 않음. sidecar 바이너리만 tunaFlow 에 빌드 산출물로 포함
- 사용자 결정 (2026-04-23): upstream PR / fork 모두 생략. 로컬 patch + 재빌드만
- 향후 upstream 에 동일 fix 기여하고 싶으면 별도 작업 (베타 공개 blocker 아님)

**수정**:

```diff
-    let ctx_before_start = chunk.lines[0].saturating_sub(1 + context_lines);
-    let ctx_before_end = chunk.lines[0].saturating_sub(1);
+    let ctx_before_start = chunk.lines[0]
+        .saturating_sub(1 + context_lines)
+        .min(lines.len());
+    let ctx_before_end = chunk.lines[0]
+        .saturating_sub(1)
+        .min(lines.len());
     let context_before = if ctx_before_start < ctx_before_end {
         lines[ctx_before_start..ctx_before_end].join("\n")
```

**검증 테스트**: chunk.lines = [98, 105], source.lines().count() = 44 인 케이스에서 panic 없이 empty `context_before` 반환.

**원인**: 파일이 인덱싱 후 축소/수정됐는데 FS watcher 의 재인덱싱이 지연 → stale chunk 조회. 바로 아래 `chunk_end = chunk.lines[1].min(lines.len())` 는 이미 clamp 하는데 `ctx_before_*` 만 누락.

**부차 작업**: `ctx_after_*` 경로도 이미 clamp 되어있는지 재확인. 동일 패턴 발견 시 함께 수정.

**배포**: `_research/_util/rawq/` 에서 직접 수정 + 커밋 (로컬 브랜치) → tunaFlow 루트에서 `./scripts/build-rawq.sh` 실행 → sidecar 바이너리 재빌드. tunaFlow 레포에는 바이너리만 (또는 CI 에서 빌드). 소스 수정은 별도 repo 에 남음.

### Subtask 02 — insight skip 로직 스니펫 필수로 강화

**파일**: `src/lib/insightOrchestration.ts:219`

**수정**:

```diff
-      const hasData = catExtraction.snippets.length > 0 || catExtraction.extraContext.length > 0;
+      // 증거 기반 분석 원칙 — 스니펫이 없으면 extraContext 만으로는 LLM 환각 위험 큼
+      const hasData = catExtraction.snippets.length > 0;

       if (!hasData) {
-        onProgress?.(`${catLabel}: 사전 추출 데이터 없음, 건너뜀`);
+        onProgress?.(`${catLabel}: 스니펫 없음, 건너뜀 (extraContext=${catExtraction.extraContext.length})`);
         continue;
       }
```

**이유**: `buildAnalysisPrompt` 의 시스템 규칙 (line 83-88) 이 "증거 기반: 아래 제공된 스니펫에서 확인할 수 있는 문제만 보고" 를 명시. 스니펫 없이 호출하면 LLM 이 규칙과 입력 사이 모순에 빠져 extended thinking 폭주 + hallucination.

**주의**: 카테고리별로 스니펫은 없어도 extraContext (e.g. test output, lessons, memory) 가 유용할 수 있음 — 이 경우 **별도 프롬프트 경로** (증거 규칙 제거, summary-only) 가 이상적이지만 본 subtask 범위 외. 일단 skip 으로 막고, 후속 plan 에서 개선 고려.

**검증 테스트**: `catExtraction.snippets = []`, `extraContext = ["test output..."]` 인 케이스에서 LLM 호출 없이 skip 로그만 남는지 확인.

### Subtask 03 — claude stream JSON `usage` nested 파서 보정

**파일**: `src-tauri/src/agents/claude.rs:25-38`, `287-292`

**사전 확인** (필수): 다음 명령으로 실제 claude CLI 의 `result` 이벤트 JSON 구조 확인:

```bash
claude -p "say hi" --output-format stream-json --permission-mode bypassPermissions 2>/dev/null \
  | rg '"type":"result"' | head -1 | jq .
```

두 케이스 분기:

**케이스 A — top-level 유지 (레어)**:
현재 스키마 유지, 다른 원인 조사 (e.g. stream-json 버퍼 flush 시점 문제).

**케이스 B — `usage` nested (높은 확률)**:

```diff
 struct StreamLine {
     #[serde(rename = "type")]
     line_type: String,
     message: Option<StreamAssistantMsg>,
     result: Option<String>,
     is_error: Option<bool>,
     cost_usd: Option<f64>,
     total_cost_usd: Option<f64>,
     total_input_tokens: Option<i64>,
     total_output_tokens: Option<i64>,
+    usage: Option<StreamUsage>,
     session_id: Option<String>,
 }
+
+#[derive(Deserialize)]
+struct StreamUsage {
+    input_tokens: Option<i64>,
+    output_tokens: Option<i64>,
+    cache_creation_input_tokens: Option<i64>,
+    cache_read_input_tokens: Option<i64>,
+}
```

그리고 line 287-292:

```diff
 final_output = Some(RunOutput {
     content: parsed.result.unwrap_or_default(),
     cost_usd: parsed.total_cost_usd.or(parsed.cost_usd).unwrap_or(0.0),
-    input_tokens: parsed.total_input_tokens.unwrap_or(0),
-    output_tokens: parsed.total_output_tokens.unwrap_or(0),
+    input_tokens: parsed.total_input_tokens
+        .or_else(|| parsed.usage.as_ref().and_then(|u| u.input_tokens))
+        .unwrap_or(0),
+    output_tokens: parsed.total_output_tokens
+        .or_else(|| parsed.usage.as_ref().and_then(|u| u.output_tokens))
+        .unwrap_or(0),
     session_id: parsed.session_id,
 });
```

한 번에 두 스키마를 지원 → claude CLI 버전이 바뀌어도 안전.

`run()` 함수 (line 378-383, `--output-format json` 경로) 도 동일 패턴 적용. `ClaudeJsonOutput` 구조체 (line 85-92) 에 `usage: Option<ClaudeUsage>` 추가.

**검증**: 실제 claude CLI 돌려서 `cost_usd > 0` 인 케이스에 `input_tokens > 0` 도 함께 찍히는지 확인.

### Subtask 04 — agent-timeout → insight_runs status 전이 + UI 이벤트

**조사 필요**: `[agent-timeout] No output for 613s, killing pid` 메시지 출력 지점 확인.

```bash
rg -n "agent-timeout|No output for.*killing" /Users/d9ng/privateProject/tunaFlow/src-tauri/src
```

해당 watchdog 위치에서:

1. `pid kill` 직후 `insight_runs` 테이블의 현재 실행 row 를 `status = 'failed'`, `error = 'agent timeout after Ns'` 로 UPDATE
2. Tauri 이벤트 emit (e.g. `insight-run-updated` 또는 기존 이벤트명) → FE 구독이 InsightPanel 상태 갱신 → 스피너 종료

**InsightPanel.tsx 측** (or EvaluationPanel.tsx):
- 해당 이벤트 리스너에서 `run.status === 'failed'` 처리 분기 확인
- toast 표시 (`"분석 실패: agent timeout"`) + spinner off

**검증**: 아키텍처 카테고리 호출이 613초 넘어가는 케이스 재현 → 스피너가 자동으로 종료되는지 / toast 뜨는지 확인.

## Invariants

- **[INV-1]** rawq 의 FS slice/range 연산은 모두 `lines.len()` / `bytes.len()` 등 실제 길이로 clamp 한 후 접근. stale chunk 조회 시 panic 대신 empty 반환. **검증**: `engine.rs` 내 모든 `[x..y]` 슬라이스에 `.min(src.len())` 또는 동등 방어 코드 확인 (grep).

- **[INV-2]** Insight 카테고리 분석 호출 전에 **스니펫 1건 이상 보유** 확인. 없으면 LLM 호출 없이 skip. 이유: `buildAnalysisPrompt` 의 "증거 기반" 규칙과 모순 방지 → hallucination + extended thinking 폭주 차단. **검증**: `insightOrchestration.ts` 카테고리 루프 단위 테스트 — `snippets=[], extraContext=[...]` 케이스에서 `invoke("run_insight_analysis")` 호출 0회.

- **[INV-3]** Claude stream JSON / one-shot JSON 파서는 **top-level `total_*_tokens` 와 `usage.*_tokens` 두 스키마 모두 지원**. `.or_else()` chain 으로 fallback. 이유: claude CLI 버전 drift 방어. **검증**: mock JSON 두 셋 (old schema / new schema) 으로 유닛 테스트 각 케이스에서 `input_tokens > 0` 반환.

- **[INV-4]** Agent timeout watchdog 은 pid kill 과 **동일 트랜잭션** 에서 DB 상태 전이 수행. 단순 kill 후 상태 미전이는 금지. 이유: UI 스피너 무한 회피. **검증**: timeout 경로 단위 테스트 — watchdog fire 후 `insight_runs.status = 'failed'` 확인.

- **[INV-5]** InsightPanel 은 run status 변경 이벤트 구독으로 스피너 on/off 결정. polling 금지. 이유: DB SSOT + Tauri event 모델 일관성. **검증**: `InsightPanel.tsx` 에서 `setInterval` / `setTimeout` 기반 상태 체크 grep — 0 매치.

## Rationale

### 베타 blocker 승격 근거

4 버그가 직렬로 맞물려 있어 ①~④ 중 하나만 남겨도 외부 사용자 환경에서 다시 발화. 특히 ② 는 rawq 와 무관한 독립 버그 — rawq 를 고치더라도 네트워크 이슈 / 쿼리 0hit / 인덱싱 지연 등으로 snippets=0 상황 재현 가능. 베타 공개 전에 4개를 **한 번에 묶어서** 처리하는 것이 운영 부담 최소.

### 왜 `kind` 컬럼 같은 구조 변경을 안 하나

대안으로 rawq 실패를 구조적으로 처리하는 방안 (e.g. insight 파이프라인에 `degradedMode` 플래그 도입) 이 있으나:
- 4줄 clamp (①) 로 근본 원인 제거 가능
- 1줄 skip 조건 (②) 로 fallback safe 보장
- 구조 변경은 오버엔지니어링

### 외부 repo 수정 (subtask-01)

rawq 는 `_research/_util/rawq/` 에 있는 외부 차용 repo. publicReadiness Phase 1 에서 `_util/` 를 tunaFlow repo 에서 제거하고 References 섹션으로 감사만 남기는 결정과 연계 — rawq 원 repo PR 은 tunaFlow 공개와 별개 trackable.

## Developer 핸드오프 프롬프트

> 새 세션에 아래 blob 을 그대로 붙여넣는다. Developer #3 작업. Developer #1, #2 와 병렬 진행 가능 (건드리는 파일 독립).

```
[작업] Insight 분석 stability 4 버그 직렬 수정 — 베타 공개 blocker

[SSOT] docs/plans/insightStabilityPlan.md 를 먼저 읽을 것. 본 프롬프트는 요약이며 plan 이 우선.

[작업 순서]

1. Subtask 03 사전 확인 (5분):
   claude -p "say hi" --output-format stream-json --permission-mode bypassPermissions 2>/dev/null \
     | rg '"type":"result"' | head -1 | jq .
   → usage 가 nested 인지 top-level 인지 확정 → Subtask 03 케이스 A/B 결정

2. Subtask 01 (rawq clamp) — 로컬 patch only:
   - 파일: /Users/d9ng/privateProject/_research/_util/rawq/src/search/engine.rs:995-1001
   - plan Subtask 01 diff 그대로 적용
   - ctx_after_* 경로도 재확인 (이미 clamp 돼있으면 추가 수정 불요)
   - _research/_util/rawq/ 내에서 로컬 커밋 (push 불요, upstream PR 생략)
   - tunaFlow 루트에서 ./scripts/build-rawq.sh 실행 → sidecar 재빌드
   - 재빌드 후 앱 실행 → 기존에 panic 나던 쿼리 재현 안 됨 확인

3. Subtask 02 (skip 로직):
   - 파일: src/lib/insightOrchestration.ts:219
   - plan Subtask 02 diff 그대로 적용
   - 단위 테스트 추가 (선택): snippets=[], extraContext=["..."] 케이스에서 invoke 0회

4. Subtask 03 (claude usage parser):
   - 파일: src-tauri/src/agents/claude.rs:25-38, 287-292, 378-383
   - 케이스 A 면 원인 재조사 (stream 버퍼 flush 등), 케이스 B 면 plan diff 적용
   - StreamLine + ClaudeJsonOutput 양쪽에 usage fallback
   - 단위 테스트: old schema / new schema 두 mock 각각 input_tokens > 0 반환

5. Subtask 04 (timeout → status 전이):
   - rg -n "agent-timeout|No output for.*killing" src-tauri/src 로 watchdog 위치 탐지
   - pid kill 직후 insight_runs.status = 'failed' UPDATE + Tauri event emit
   - InsightPanel.tsx (또는 EvaluationPanel.tsx) 구독 측에서 failed 처리 + 스피너 off + toast
   - 재현 검증: 긴 프롬프트로 timeout 유도 → 스피너 자동 종료 확인

[검증]
- cargo check + cargo test --lib
- npx tsc --noEmit + npx vitest run
- 수동 reproduce: tunaReader 에서 Insight 분석 실행 → 아키텍처/테스트 카테고리에서 스피너 정상 종료 + 비용 합리적 (카테고리당 $0.1 미만) 확인
- Insight 로그에 input_tokens / output_tokens 0 아닌 실제값 표시 확인

[커밋]
- Subtask 별 커밋 분리 권장:
  - fix(rawq): clamp ctx_before slice against source length (원 repo)
  - fix(insight): skip category when snippets empty (evidence-based rule)
  - fix(claude): parse usage nested in stream-json result event
  - fix(insight): propagate agent-timeout to insight_runs status + UI event
- 각 커밋 본문에 Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR]
- 제목: fix(insight): stabilize 4-bug chain (rawq clamp / skip strict / usage parser / timeout propagation)
- 설명에 plan 의 "직렬 연관도" 다이어그램 붙이고, 재현 전후 비용/시간 비교 수치 기재

[브랜치]
fix/insight-stability (또는 유사)
```

## 관련 문서

- `docs/plans/publicReadinessChecklistPlan.md` — 베타 공개 checklist (병렬 진행)
- `docs/plans/i18nPlan.md` §11 — i18n PR A 핸드오프 (병렬 진행)
- `docs/reference/sessionHistory.md` — rawq 관련 이전 이슈 참고
