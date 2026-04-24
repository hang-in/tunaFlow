---
title: Manual verification gate — impl-complete 와 Reviewer 사이 사용자 확인 단계 (B-19)
status: ready-to-implement
priority: P1
created_at: 2026-04-24
updated_at: 2026-04-24 (Issue #176 커뮤니티 피드백 반영 후 ready 승격)
related:
  - docs/plans/postBetaBacklogPlan_2026-04-24.md  # B-19
  - docs/plans/resultReportMarkerCleanupPlan_2026-04-24.md  # C-2 (선행 권장)
  - docs/agents/developer.md
  - docs/agents/reviewer.md
  - https://github.com/dghong/tunaFlow/issues/176
canonical: true
owners:
  - architect (본 문서 작성)
  - developer (구현, 피드백 수령 후)
---

# 배경

Reviewer persona 는 `docs/agents/reviewer.md` §"Read code only" 규칙상 **shell / 빌드 / 런타임 실행이 금지**돼 있다 (코드만 읽고 판정). 그 결과 다음 카테고리 검증은 **owner 가 없다**:

- 실제 UI 클릭/렌더 동작 (예: 버튼 눌렀을 때 다이얼로그가 제대로 닫히는지)
- 외부 API / 네트워크 응답 (dev 서버 동작, CORS 등)
- OS 인터랙션 (파일 선택 다이얼로그, 클립보드, 권한)
- 지각적 품질 (색상, 타이포, 애니메이션 자연스러움)

현재는 Review 라운드가 이런 항목도 떠안아 → "코드상 문제 없음" 판정 후 실제로 돌렸을 때 버그가 터지는 회귀 반복. 비용 낭비 + 사용자 신뢰 저하.

**해결 방향**: `impl-complete` 와 `startReviewRT` 사이에 **사용자 확인 게이트** 삽입. Developer 가 실행 불가 항목을 report 에 명시 → UI 다이얼로그로 사람이 확인 → pass 면 Review 진행 / fail 면 Rework.

Issue #176 의 요구사항과 정확히 일치. 이슈 댓글의 추가 질문("fail 시 실패 사유 입력?") 은 MVP 에 optional 텍스트 필드로 포함.

# 현재 상태 (사실 확인 — 2026-04-24 기준)

## (A) Review workflow 진입점

- `src/lib/workflow/reviewWorkflow.ts:52-77` `startReviewRT()`
  - Line 58: `updatePlanPhase(plan.id, "review")` — phase 전환
  - Line 59: `createPlanEvent(plan.id, "impl_completed", "developer")` — 이벤트 발생
  - Line 61-62: `syncResultReport()` — result.md 생성 (fire-and-forget)
  - **게이트 삽입 지점 = Line 58 직전**. phase 전환 전에 게이트를 돌려야 fail 시 "review" 로 잘못 들어가지 않는다.

## (B) Rework 진입 경로 (재사용 대상)

- `src/lib/workflow/reviewWorkflow.ts:290` — Review fail 시 `updatePlanPhase(plan.id, "rework")` 호출
- 같은 파일에서 `createReworkReasonArtifact(plan, findings, ...)` 로 rework 사유 artifact 생성
- UI 는 `DevProgressView.tsx:222`, `ReviewVerdictCard.tsx:38` 의 `handleRework` 버튼 → Developer 브랜치 재진입

게이트 fail 시 동일 경로를 그대로 사용한다 — state machine 은 건드리지 않는다.

## (C) result report 생성 경로

- `src/lib/workflow/reportSync.ts:37-83` `syncResultReport()`
- 현재 `knownIssues: string[] = []` (line 76) — 여기에 manual 검증 결과를 태깅할 수 있으나, **게이트는 report 생성 *이전* 에 돌아야 함**. report 에 "Manual: ..." 가 나오는 건 Developer 응답 안에서 이미 요구됨 (아래 (D)).
- **C-2 (`resultReportMarkerCleanupPlan`) 와의 관계**: `⚠️ Manual: ...` 라인은 **마커가 아니라 사람이 읽어야 하는 텍스트**이므로 C-2 `stripTunaflowMarkers()` 의 대상이 아니다. 그러나 report 본문에는 들어가야 하고 (사용자가 무엇을 확인했는지 기록용), parser 입력으로도 사용돼야 하므로 **손대지 않는다**.

## (D) Developer persona — "Manual" 지시 (현재 없음)

- `docs/agents/developer.md:20-38` 현재 Verification 섹션은 **shell 실행 가능 항목만** 다룸 (`npx tsc --noEmit`, `vitest run` 등)
- Manual 항목 (shell 로 못 잡는 것) 에 대한 지시는 **없음** → Developer 가 알아서 챙기거나 묵살
- 이 plan 에서 `## Manual Verification — FLAG DON'T RUN` 섹션을 추가한다.

## (E) plan_events 스키마

- `src/types/index.ts:333-340` `PlanEvent { id, planId, eventType: string, actor?, detail(JSON), createdAt }`
- 기존 사용된 eventType 문자열:
  - `impl_completed`, `review_passed`, `review_failed`, `review_conditional`, `rework_reason`, `design_review_suggested`
- **DB 마이그레이션 불필요** — `eventType` 이 자유 문자열이므로 신규 값만 추가하면 된다.
- 신규 값: `manual_verification_passed`, `manual_verification_failed`, `manual_verification_skipped`
- `detail` 에 `{ items: [{ label, status: "pass"|"fail"|"skip", reason?: string }] }` JSON 저장

## (F) Settings persist 패턴

- `src/components/tunaflow/settings/IdentityAnalysisSettings.tsx:1-80` — 토글 샘플
- `appStore.ts` 의 `getSetting<T>(key, fallback)` / `setSetting<T>(key, value)` 사용
- 기존 booleanToggle 예시: `backgroundInsightEnabled` — 동일 패턴으로 `skipManualVerificationGate` 추가

## (G) Dialog 컴포넌트

- 전용 confirmation dialog 컴포넌트는 없음 (daisyUI modal 기반 ad-hoc)
- `src/components/tunaflow/CreateRoundtableDialog.tsx` 가 가장 근접한 참고 — useState 로 open/data 관리, Portal 없이 모달 구현
- **신규 컴포넌트로 작성**하고, 재사용 가능한 형태 (items prop + onComplete callback) 로 두면 향후 design_review gate 등에도 재활용 가능

# MVP 설계 (1~2일)

## (1) Developer persona 업데이트

**파일**: `docs/agents/developer.md`

현재 §"Verification — MANDATORY" (line 24-38) 다음에 신규 섹션 추가:

```markdown
## Manual Verification — FLAG, DO NOT RUN

Shell 로 확인 불가능한 항목 (UI 클릭, OS 다이얼로그, 외부 API 응답, 지각적 품질) 은
**직접 실행하지 말고**, 응답에 다음 형식으로 열거만 한다:

```
⚠️ Manual: 프로젝트 선택 드롭다운을 열고 "All" 옵션이 맨 위에 표시되는지 확인
⚠️ Manual: Settings 열고 "Skip gate" 토글을 켠 상태로 재시작 후 토글 상태 유지 확인
```

- 1 줄 1 항목. prefix `⚠️ Manual:` 은 필수.
- 구체적으로 **무엇을 눌러서 / 어떤 결과가 나와야 하는지** 쓰기.
- "테스트해주세요" 같은 막연한 지시 금지.
- 이 라인은 chat message 에만 쓰고 파일에는 쓰지 않는다 (기존 impl-complete 마커 규칙과 동일).

tunaFlow 가 이 항목들을 모아 **사용자에게 확인 다이얼로그**로 제시한다.
사용자가 직접 pass/skip/fail 판정한다.
```

## (2) 파서 — `src/lib/manualVerification.ts` (신규)

```ts
// src/lib/manualVerification.ts
import type { Message } from "@/types";

export interface ManualVerificationItem {
  label: string;         // "⚠️ Manual:" prefix 제거 후 본문
  source: "developer";   // 향후 확장 여지
}

export interface ManualVerificationResult {
  status: "pass" | "fail" | "skip";
  reason?: string;       // fail 인 경우 사용자가 입력한 실패 사유
}

export interface ManualVerificationReport {
  items: ManualVerificationItem[];
  results: ManualVerificationResult[];  // items 와 동일 순서/길이
}

/**
 * Developer 의 마지막 assistant 메시지에서 `⚠️ Manual: ...` 라인을 추출.
 * Rework 가 있었으면 마지막 Rework 이후 범위만 본다 (syncResultReport 로직과 동일).
 */
export function extractManualItems(implMessages: Message[]): ManualVerificationItem[] {
  let lastReworkIdx = -1;
  for (let i = implMessages.length - 1; i >= 0; i--) {
    if (implMessages[i].role === "user" && implMessages[i].content.includes("### 🔄 Rework")) {
      lastReworkIdx = i;
      break;
    }
  }
  const relevant = lastReworkIdx >= 0 ? implMessages.slice(lastReworkIdx + 1) : implMessages;
  const lastAssistant = [...relevant].reverse().find((m) => m.role === "assistant");
  if (!lastAssistant) return [];

  const items: ManualVerificationItem[] = [];
  const re = /^\s*⚠️\s*Manual:\s*(.+)$/gm;
  let match: RegExpExecArray | null;
  while ((match = re.exec(lastAssistant.content)) !== null) {
    const label = match[1].trim();
    if (label.length > 0) items.push({ label, source: "developer" });
  }
  return items;
}
```

**정규식**: `⚠️` 유니코드 이모지 + 공백 + `Manual:` + 공백 + 본문. multi-line flag 로 라인 앞 여백 허용.

## (3) Dialog — `src/components/workflow/ManualVerificationGate.tsx` (신규)

**Props**:
```ts
interface Props {
  open: boolean;
  items: ManualVerificationItem[];
  onComplete: (results: ManualVerificationResult[]) => void;
  onCancel: () => void;
}
```

**UI 구조**:
- daisyUI modal (`CreateRoundtableDialog.tsx` 패턴 참조)
- 제목: "수동 확인이 필요한 항목"
- 부제: "Developer 가 직접 확인할 수 없어 사용자 확인을 요청했습니다"
- item 별 행:
  - 라벨 텍스트 (read-only)
  - 세 버튼 segmented: [Pass] [Skip] [Fail]  — default: 미선택
  - Fail 선택 시 그 행 아래에 textarea 등장 ("실패 사유" — optional)
- 하단:
  - "모두 Pass 로 표시" 버튼 (단축)
  - "진행" 버튼 — 전부 선택돼야 활성화
  - "취소" 버튼 — 게이트 자체를 취소 (workflow 멈춤, phase 유지)

**상태**:
- 각 item 별 state: `"pass" | "skip" | "fail" | null`
- Fail 인 항목 별 reason state
- 최소 1개가 null 이면 "진행" 버튼 disabled

**재사용성**: `items` + `onComplete` 만 prop 이므로 향후 다른 게이트 (디자인 리뷰 승인 등) 에도 그대로 사용 가능.

## (4) reviewWorkflow.ts 게이트 호출

**위치**: `startReviewRT()` 진입 직후 (line 52-58 사이).

```ts
// src/lib/workflow/reviewWorkflow.ts (모식도)
import { extractManualItems, type ManualVerificationResult } from "../manualVerification";
import { getSetting } from "../appStore";

export async function startReviewRT(
  plan: Plan,
  implMessages: Message[],
  testOutput?: string,
  reviewers?: ReviewerChoice[] | string[],
  // 신규 — UI 가 dialog 띄우고 결과 넘겨줌
  runManualGate?: (items: ManualVerificationItem[]) => Promise<ManualVerificationResult[] | null>,
): Promise<StartReviewRTResult> {
  // ─── Manual Verification Gate ───
  const skip = await getSetting<boolean>("skipManualVerificationGate", false);
  if (!skip && runManualGate) {
    const items = extractManualItems(implMessages);
    if (items.length > 0) {
      const results = await runManualGate(items);
      if (results === null) {
        // 사용자가 dialog 취소 → phase 그대로 유지, 에러 throw
        throw new Error("Manual verification cancelled by user");
      }
      const hasFail = results.some((r) => r.status === "fail");
      const eventType = hasFail ? "manual_verification_failed" : "manual_verification_passed";
      await planApi.createPlanEvent(plan.id, eventType, "user", {
        items: items.map((it, i) => ({
          label: it.label,
          status: results[i].status,
          reason: results[i].reason,
        })),
      });
      if (hasFail) {
        // Rework 경로 진입 — 기존 함수 재사용
        await planApi.updatePlanPhase(plan.id, "rework");
        const failItems = items
          .map((it, i) => ({ label: it.label, reason: results[i].reason }))
          .filter((_, i) => results[i].status === "fail");
        const reason = [
          "Manual verification 실패:",
          ...failItems.map((f) => `- ${f.label}${f.reason ? ` (${f.reason})` : ""}`),
        ].join("\n");
        createReworkReasonArtifact(plan, reason);
        throw new ManualVerificationFailed(failItems);
      }
    } else {
      // 0 items → 게이트 skip, Review 진행
      await planApi.createPlanEvent(plan.id, "manual_verification_skipped", "system", {
        reason: "no manual items found in impl response",
      });
    }
  }
  // ─── 기존 로직 ───
  await planApi.updatePlanPhase(plan.id, "review");
  await planApi.createPlanEvent(plan.id, "impl_completed", "developer");
  // ... (이하 기존)
}

export class ManualVerificationFailed extends Error {
  constructor(public readonly failedItems: Array<{ label: string; reason?: string }>) {
    super(`Manual verification failed: ${failedItems.length} item(s)`);
  }
}
```

**호출 UI 측 주입**: `startReviewRT` 를 호출하는 컴포넌트 (예: `DevProgressView.tsx` 의 impl-complete 핸들러) 에서 `runManualGate` 콜백을 넘긴다. 콜백은 `ManualVerificationGate.tsx` 를 열고 Promise 로 결과 반환.

**왜 callback 주입 방식인가**: workflow 함수가 React 컴포넌트를 직접 알지 못하게 분리. 테스트 시 mock 콜백으로 게이트 동작 검증 가능.

## (5) Settings 토글

**파일**: 기존 Settings 화면 (예: `src/components/tunaflow/settings/DeveloperSettings.tsx` 또는 신규 섹션)

```tsx
const [skipGate, setSkipGate] = useState<boolean>(false);

useEffect(() => {
  getSetting<boolean>("skipManualVerificationGate", false).then(setSkipGate);
}, []);

const onToggle = async (next: boolean) => {
  await setSetting("skipManualVerificationGate", next);
  setSkipGate(next);
};

// JSX
<label>
  <input type="checkbox" checked={skipGate} onChange={(e) => onToggle(e.target.checked)} />
  수동 확인 게이트 건너뛰기 (개발자 모드)
</label>
<p className="text-xs text-gray-500">
  Developer 의 ⚠️ Manual 항목을 다이얼로그 없이 자동 pass 처리합니다.
  UI 검증을 자주 건너뛰면 회귀를 놓칠 수 있습니다.
</p>
```

## (6) 테스트

### FE (vitest)

- 신규: `src/lib/manualVerification.test.ts`
  - 0 items (assistant 에 ⚠️ Manual 없음)
  - 1 item
  - 여러 item (멀티라인)
  - Rework 이후 범위만 추출 (Rework 이전 항목 무시)
  - 비ASCII 유니코드 깨지지 않음
- 신규: `src/components/workflow/ManualVerificationGate.test.tsx`
  - items 렌더링
  - 각 버튼 클릭 → state 변경
  - 전부 선택 전까지 진행 버튼 disabled
  - fail 선택 시 reason textarea 등장
  - "모두 Pass" 버튼 → 일괄 pass + 진행 활성화

### FE (workflow 통합)

- 신규 또는 기존 `reviewWorkflow` 관련 테스트에 케이스 추가:
  - skipGate=true → 게이트 우회, Review 진행
  - 0 items → 게이트 우회, Review 진행
  - all pass → Review 진행 + `manual_verification_passed` event
  - any fail → rework phase + `manual_verification_failed` event + ManualVerificationFailed throw
  - cancel → phase 그대로, Error throw

# Invariants

- **[INV-1]** 게이트 fail 시 plan phase 는 절대 "review" 로 들어가지 않는다. rework 또는 원래 phase 유지. **검증**: workflow 통합 테스트.
- **[INV-2]** `skipManualVerificationGate=true` 면 `extractManualItems` 도 호출하지 않고 바로 Review 진행한다 (설정 1 회 조회 비용만).
- **[INV-3]** items.length === 0 이면 dialog 를 띄우지 않는다 — 개발자가 처음 쓸 때 매번 빈 다이얼로그가 튀어나오는 UX 사고 방지.
- **[INV-4]** 게이트 결과는 반드시 `plan_events` 에 기록된다 (best-effort — 저장 실패해도 UX 차단은 안 함). Insight 분석이 이 이벤트를 참조할 수 있어야 함.
- **[INV-5]** 사용자가 dialog 를 취소 (X 버튼) 하면 phase 전환 없음. 나중에 impl-complete 다시 눌러 재진입 가능.
- **[INV-6]** Rework 사유 artifact 에는 **Manual 실패 항목 전체 텍스트 + 사용자 입력 사유** 가 포함된다. Developer 가 rework 시 뭘 고쳐야 할지 알아야 한다.

# Rationale

## 왜 Developer 응답 파싱이지 Plan 단에서 item 지정 아닌가

Plan 단에서 Architect 가 manual 항목을 미리 써두는 방식도 고려했으나:
- Architect 는 **구현 전** 단계 → 실제 구현이 끝난 뒤 "뭐를 눌러봐야 되는지" 는 Developer 가 더 잘 안다
- Plan 에 쓰면 하드코딩돼서 중간에 구현 변경된 경우 stale
- Developer 가 자기 응답에 쓰는 게 자연스러움 (기존 Verification 섹션과 동일 패턴)

## 왜 callback 주입 방식인가

workflow 함수 (Rust 도 아니고 그냥 TS 이지만 UI-agnostic) 가 React 다이얼로그를 import 하면 **순환 의존**이 생기고, 테스트 시 mock 하기 어렵다. callback 주입은:
- workflow 는 "게이트 돌려주세요" 만 요청
- UI 레이어가 dialog 를 만들고 결과 반환
- 테스트에서는 mock callback 하나로 시나리오 커버

## 왜 "skip" 옵션이 pass 와 별도인가

- **pass** = 확인해봤고 OK
- **skip** = 확인 안 했지만 진행하고 싶음 (예: 장비 없음, 나중에 할 것)
- **fail** = 확인해봤고 문제 있음

skip 을 pass 에 합치면 **실제로 검증 안 된 항목이 통과한 것처럼 기록** 되어 Insight 분석에서 품질 왜곡. 별도로 두면 "skip 비율" 이라는 지표 자체가 의미 있음.

## 왜 Extended 가 아니라 MVP 에 "실패 사유 입력" 을 넣나

이슈 #176 댓글에서 사용자가 먼저 질문한 항목이고, 실제로 "없으면 rework 프롬프트가 공허해짐". Developer 가 `실패 사유: 버튼이 안 눌린다` 정도만 받아도 핀포인트 가능. textarea 1 개 추가라 구현 비용 거의 0.

## 왜 C-2 를 선행 권장인가

마커 스크럽 로직이 C-2 에서 공용 유틸로 정리된다. 이 plan 에서 만든 `⚠️ Manual:` 라인은 마커가 아니지만, **Developer persona 규칙** ("chat message 에만 쓰고 파일에 안 씀") 은 동일. 공용 유틸 import 위치 같은 구조적 결정은 C-2 머지 뒤가 깔끔.

단 **hard dependency 는 아니다** — C-2 가 늦어지면 이 plan 먼저 구현 가능. 그 경우 `⚠️ Manual` 라인이 result report 에 포함되는지 (= Developer 응답 일부로 남는지) 육안 검증 필요.

# Extended — 후순위 (P2, 별도 plan 승격 대상)

- **자동 스크린샷**: Tauri FS API 로 item 확인 중 화면 캡처 → artifact 로 저장. 에이전트 후속 분석에 유용.
- **RT 기반 자동 확인**: 사람 대신 별도 RT (예: QA agent) 가 headless UI 실행 + 스크린샷 + LLM vision 판정. 기술적으로 무거움.
- **Item 간 의존성**: "A 확인 후에만 B 확인 가능" 같은 DAG. 지금은 flat.
- **Insight 연계**: `manual_verification_failed` event 를 Insight 에 집계해 "자주 실패하는 UI 영역" 분석.

# Developer 핸드오프 프롬프트 (실제 구현 시 사용)

> ✅ Issue #176 커뮤니티 피드백 수령 완료 (2026-04-24). `ready-to-implement` / P1. 아래 blob 을 새 Developer 세션에 붙여넣는다.

```
[작업] Manual verification gate — impl-complete 와 Reviewer 사이에 사용자 확인 단계 삽입 (B-19 / Issue #176)

[SSOT] docs/plans/manualVerificationGatePlan_2026-04-24.md 를 먼저 읽고, MVP 설계 §(1)~(6) 순서대로 처리.

[배경 3줄]
- Reviewer 는 shell 실행 불가라 UI/runtime 확인 owner 가 없음
- Developer 가 ⚠️ Manual 라인으로 항목 열거 → UI dialog 로 사람이 pass/skip/fail
- fail 있으면 기존 Rework 경로 재사용, pass 면 Review 진행

[수정 범위]

1) 신규: src/lib/manualVerification.ts
   - extractManualItems(implMessages): ManualVerificationItem[]
   - 타입: ManualVerificationItem, ManualVerificationResult, ManualVerificationReport
   - 정규식 /^\s*⚠️\s*Manual:\s*(.+)$/gm
   - Rework 이후 범위만 (syncResultReport 와 동일 필터링)

2) 신규: src/components/workflow/ManualVerificationGate.tsx
   - Props: open, items, onComplete, onCancel
   - daisyUI modal + CreateRoundtableDialog.tsx 패턴
   - item 별 [Pass][Skip][Fail] segmented + fail 시 reason textarea
   - "모두 Pass" 단축 버튼

3) 수정: src/lib/workflow/reviewWorkflow.ts
   - startReviewRT 시그니처에 runManualGate?: callback 추가
   - 진입 직후 (line 58 phase 전환 전) 게이트 로직 삽입
   - skipGate=true 또는 items.length===0 → 우회
   - all pass → plan_event "manual_verification_passed" + Review 진행
   - any fail → plan_event "manual_verification_failed" + updatePlanPhase(plan.id,"rework") + createReworkReasonArtifact + throw ManualVerificationFailed
   - cancel → throw "Manual verification cancelled by user"
   - neu class: export class ManualVerificationFailed extends Error

4) 수정: startReviewRT 호출 UI (DevProgressView.tsx impl-complete 핸들러)
   - runManualGate 콜백 주입 → ManualVerificationGate 를 열고 결과 Promise 반환
   - ManualVerificationFailed catch → DevProgressView 의 기존 rework 상태 전환 UI 트리거
   - 일반 Error (cancel) catch → toast "수동 확인이 취소되었습니다" + 버튼 상태 복원

5) 수정: src/components/tunaflow/settings/*Settings.tsx (DeveloperSettings 또는 신규 섹션)
   - skipManualVerificationGate boolean 토글 추가
   - getSetting/setSetting 패턴 (IdentityAnalysisSettings 참조)
   - helper text: "UI 검증을 자주 건너뛰면 회귀를 놓칠 수 있습니다"

6) 수정: docs/agents/developer.md
   - §"Verification — MANDATORY" 다음에 §"Manual Verification — FLAG, DO NOT RUN" 신규 섹션 추가
   - 프롬프트 텍스트는 SSOT plan §(1) 참조

7) 테스트
   - src/lib/manualVerification.test.ts — 5 케이스 (0/1/다건/Rework 이후/유니코드)
   - src/components/workflow/ManualVerificationGate.test.tsx — 5 케이스 (렌더/선택/disabled/fail reason/모두 pass)
   - reviewWorkflow 통합 테스트 — 5 시나리오 (skip toggle / 0 items / all pass / any fail / cancel)

[검증]
- npx tsc --noEmit: 0 에러
- npx vitest run: 신규 + 기존 전량 pass
- 풀사이클 smoke:
    1. 테스트 프로젝트 impl 1 회차에 ⚠️ Manual 2 줄 포함시키기
    2. impl-complete 마커 감지 → 다이얼로그 등장 확인
    3. 모두 Pass → Review RT 자동 시작 확인
    4. 다른 plan 에서 1 개 Fail + 사유 입력 → Rework 상태 + rework_reason artifact 에 사유 포함 확인
    5. Settings 에서 skipGate 켜고 재실행 → 다이얼로그 스킵 확인

[커밋]
- feat(workflow): extract ⚠️ Manual items parser + types
- feat(ui): ManualVerificationGate dialog
- feat(workflow): wire manual gate into startReviewRT (pass/fail/skip/cancel)
- feat(settings): skipManualVerificationGate toggle
- docs(persona): developer ⚠️ Manual verification guidance
- test: manual verification gate coverage

각 커밋 trailer: Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>

[PR 제목]
feat(workflow): manual verification gate between impl-complete and review (B-19 / #176)

[주의]
- git stash drop/clear 금지
- workflow 함수는 React 직접 import 하지 말 것 — callback 주입 방식 유지
- skipGate=true 경우에도 plan_events 에 "manual_verification_skipped" 기록하지 말 것 (toggle 은 개발자 본인 선택, 기록 노이즈). items.length===0 인 경우에만 "manual_verification_skipped" 기록.
- ⚠️ 이모지는 U+26A0 U+FE0F. 정규식에서 깨지지 않도록 UTF-8 리터럴 사용.
- ManualVerificationFailed 는 일반 Error 와 구별돼야 함 (호출부에서 catch 분기) — instanceof 체크로.
```

# 커뮤니티 피드백 대응

## ✅ 수령 완료 (Issue #176, batmania52, 2026-04-24)

### 1. **실패 사유 필수 여부** — 확정

> "fail 사유 입력은 옵셔널로 넣어주시면 좋을 것 같습니다. 간단한 체크리스트 실패면 굳이 사유를 적지 않아도 되고, 복잡한 케이스는 메모를 남길 수 있게요. Developer 한테 넘어갈 때 사유가 있으면 포함, 없으면 그냥 'manual verification failed' 정도로 넘어가는 방식이면 충분할 것 같습니다."

- **채택**: optional textarea 유지. 초안 결정과 일치.
- **rework_reason artifact 포맷**:
  - 사유 있음 → `- {label} ({reason})`
  - 사유 없음 → `- {label} (manual verification failed)`  ← placeholder 문자열 통일
- 구현 시 §(4) reviewWorkflow 코드의 reason 병합 로직을 위 규칙에 맞게 작성할 것.

## 피드백 미수령 — 초안 결정 유지 (Extended 로 보류)

### 2. **게이트 트리거 시점** — 자동 유지 (초안 그대로)

- 초안 결정 그대로: impl-complete 마커 감지 시 자동 startReviewRT → 그 안에서 게이트
- 명시적 "리뷰 시작" 버튼 도입은 UX 깊이가 커져 MVP 범위 초과. 추후 사용성 피드백 오면 재검토.

### 3. **Manual 항목 누락 감지** — Extended 로 보류 (초안 그대로)

- 현재 MVP: 파싱 결과 0 이면 게이트 우회 (Developer 가 shell 로 다 검증했다고 주장)
- 확장 (plan 단 "manual 필수" 플래그 + 누락 경고) 은 P2 로 보류
- 추후 피드백 또는 실측 "manual 누락이 실제로 문제 되는 케이스" 확인 후 승격

# 관련 기록

- `docs/plans/postBetaBacklogPlan_2026-04-24.md` **B-19** — 본 plan 으로 승격됨. backlog 항목은 링크만 유지.
- Issue #176 (커뮤니티 `batmania52`) — 이 기능의 원 발의. 2026-04-24 공개 첫날 등록.
- `docs/agents/reviewer.md` — "Read code only" 규칙의 구조적 귀결로 이 게이트가 필요해진 배경.
