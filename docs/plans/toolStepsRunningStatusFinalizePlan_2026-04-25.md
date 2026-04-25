---
title: Tool steps "running" status 잔존 → 메시지 spinner 무한 회전 (hotfix)
status: ready-to-implement
priority: P1 (사용자 가시 UX 결함)
created_at: 2026-04-25
related:
  - src/stores/slices/agentStreamHelper.ts (saveToolSteps)
  - src/components/tunaflow/message/ToolStepsView.tsx (collapsed 모드)
  - src/stores/toolStepsStore.ts (handleProgress status 갱신 로직)
  - src/lib/toolSteps.ts (직렬화/역직렬화)
canonical: true
owners:
  - architect (본 문서 작성 + hotfix 직접 구현)
---

# 요약

Long-doc 태스크 (Codex/Gemini 메타) 완료 후 메시지 본문은 정상 표시되지만 메시지 안의 **tool steps spinner 가 영원히 회전**. 시스템 spinner 와 phase 전이는 모두 정상 종료된 상태에서 발생.

# 현재 상태 (사실 확인)

## 정상 파이프라인 (의도된 흐름)

```
engine
  → "__STEP__:{...status:'running', name:'X'}" 로 progress 이벤트 emit
  → toolStepsStore.handleProgress 가 stepsMap[messageId] 에 running step push
  → "__STEP__:{...status:'done', name:'X'}" emit
  → handleProgress (toolStepsStore.ts:42-60) 에서 같은 name 의 마지막 running step 을 찾아 done 으로 갱신
  → agent:completed 이벤트
  → agentStreamHelper.ts:saveToolSteps 가 stepsMap 을 progressContent 로 직렬화 + DB 저장
  → tsStore.clear(messageId)
  → 메시지 isStreaming=false 로 전환 → MessageItem 이 effectiveProgress 에서 deserialize → ToolStepsView 렌더 (collapsed 모드)
```

## 비정상 케이스 — running 잔존

다음 중 **하나라도** 발생하면 step 의 status="running" 이 saveToolSteps 시점까지 유지됨:

1. **race**: 마지막 done event 가 agent:completed 이벤트보다 늦게 도착
2. **name mismatch**: engine 별 step name 정규화 차이 (whitespace, suffix, case) 로 line 47 의 `updated[i].name === step.name` 매칭 실패. 새 step 으로 push 되고 기존 running 은 그대로
3. **누락**: engine 이 마지막 done emit 자체를 하지 않고 stream 종료

### 결과

- `progressContent` 에 `"status":"running"` 이 직렬화돼 DB 영구 저장
- 재렌더 시 ToolStepsView 의 **collapsed 모드 lastStep** (toolStepsView.tsx:41) 이 running 이면 → StepLine line 107-109 의 `Loader2 animate-spin` 이 영구 표시
- 사용자 증언과 정확히 일치

## 추가 관찰

- **350회 카운트는 정상 범위** (long-doc 7 task × 평균 50 step). 이번 plan 의 대상 아님
- **plan → dev 전이는 무관**. impl-complete 마커 (planProposalParser.ts:451) 와 toolSteps 시스템은 완전히 별개 경로
- **엔진별 빈도 차이**: Claude stream-json 이 가장 안정적, Codex app-server 와 Gemini CLI 가 race 위험 더 큼 (별 plan 으로 엔진별 emit 패턴 분석 권장)

# 설계

## Layer A — `saveToolSteps` finalize (Primary)

**파일**: `src/stores/slices/agentStreamHelper.ts:131-139`

```ts
export async function saveToolSteps(messageId: string): Promise<void> {
  const tsStore = useToolStepsStore.getState();
  const steps = tsStore.getSteps(messageId);
  if (steps.length > 0) {
    // agent:completed 가 도달한 시점에 stream 은 종료된 것이므로 남은
    // "running" step 은 race/누락 케이스로 정의상 마무리된 상태. done 으로
    // finalize 해서 멈춘 spinner 가 progressContent 에 굳지 않게 한다.
    const finalized = steps.map((s) =>
      s.status === "running" ? { ...s, status: "done" as const } : s
    );
    invoke("save_progress_content", { messageId, progressContent: serializeSteps(finalized) })
      .catch((e) => console.debug("[save-steps]", e));
    tsStore.clear(messageId);
  }
}
```

근거: `agent:completed` 이벤트의 의미 자체가 "stream 종료" → 그 시점에 running 인 step 은 의도와 무관한 잔존. done 으로 처리하는 게 의미상 정확.

## Layer B — `ToolStepsView` 렌더 단 defensive fallback (Secondary)

**파일**: `src/components/tunaflow/message/ToolStepsView.tsx:40-63` (collapsed 모드)

`!isStreaming` 인데 status="running" 인 step 이 들어오면 done 으로 표시. **기존 DB 의 오염된 progressContent 도 자동 흡수** → 마이그레이션 불요.

```tsx
if (!isStreaming) {
  // Defensive: progressContent 가 과거 race/누락으로 status="running" 인
  // step 을 포함할 수 있다. 비-스트리밍 시점에서는 정의상 모든 step 이
  // 완료 상태이므로 표시상 done 으로 fallback (마이그레이션 불요).
  const displaySteps = steps.map((s) =>
    s.status === "running" ? { ...s, status: "done" as const } : s
  );
  const lastStep = displaySteps[displaySteps.length - 1];
  // ... 이후 displaySteps 와 lastStep 사용
}
```

# Invariants

- **[INV-1]** `progressContent` 에 새로 저장되는 직렬화 결과는 `"status":"running"` 을 포함하지 않는다 (Layer A)
- **[INV-2]** `!isStreaming` 모드의 ToolStepsView 는 어떤 입력이든 spinner 를 표시하지 않는다 (Layer B fallback)
- **[INV-3]** 기존 DB progressContent 의 오염 데이터는 Layer B 가 런타임에 흡수 — 별도 마이그레이션 불요
- **[INV-4]** Streaming 중 (`isStreaming=true`) 에는 정상적으로 running step 의 spinner 가 회전한다 (실제 진행 중 표시 손상 X)

# 테스트

- 수동 smoke (PR 필수):
  1. Codex 또는 Gemini 로 long-doc 태스크 1회 실행 → 완료 후 메시지 collapse 시 spinner **멈춤** 확인
  2. 기존 ConvId 로 collapsed 메시지 재방문 시에도 spinner 멈춤 (Layer B 흡수 확인)
  3. 새 stream 진행 중에는 마지막 running step 에서 spinner 정상 회전 (Layer A 가 streaming 단계엔 영향 X)
- 자동 테스트는 제한적. 신규 unit test 가능하면 toolStepsStore + saveToolSteps mock 으로 finalize 동작만 verify

# Rationale

## 왜 두 레이어 모두 필요한가

- Layer A 만 → 신규 메시지에만 적용. 이미 DB 에 저장된 오염 data 는 그대로
- Layer B 만 → DB 에는 여전히 running 으로 저장. 다른 클라이언트/도구가 progressContent 를 raw 로 읽으면 잘못된 표시
- A + B 조합으로 **저장 단 + 표시 단** 양쪽 방어. 마이그레이션 불요

## 왜 엔진별 emit 패턴 수정은 별 plan 인가

이 plan 은 "어떤 이유든 race/누락이 발생해도 UI 가 망가지지 않게" 가 목표. 엔진별 emit 정합성 (왜 race 가 발생하나) 은 더 깊은 조사 + 엔진별 PR. 분리.

# Developer 핸드오프 — 이번엔 Architect 가 직접 구현

규모 (2 파일, 각 5~10줄 수정) 가 작아 #185 fix 와 동일하게 Architect 세션이 직접 PR 작성. Developer 세션 별도 띄우지 않음.

# 관련 기록

- onboardingCancelLeakFixPlan_2026-04-25 (스코프 다름, 별 PR)
- 후속 별 plan: 엔진별 step emit 패턴 분석 (Codex app-server / Gemini CLI / Claude stream-json 비교)
- 본 plan 자체는 hotfix 머지 후 archive 처리
