# tunaFlow Chat UI parity with tunaChat 마스터 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Chat UI parity with tunaChat

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/chatUiVsTunaChatGapReview_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatUiParityWithTunaChatPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatMarkdownCodeblockUpgradePlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatFileViewerIntegrationPlan_2026-03-29.md`

핵심 전제:
- 목표는 tunaChat 수준의 채팅 읽기 경험에 근접시키는 것
- 하지만 tunaFlow의 branch / roundtable / follow-up / artifacts 구조는 보존할 것
- 단순 복붙이 아니라 tunaFlow 구조에 맞는 이식이어야 함

## 작업 방식

각 단계 시작 전에 반드시 먼저 `Opinion`을 짧게 제시하라.
그 다음 구현을 진행하라.

Opinion에는 아래를 포함:
- 현재 단계에서 가장 중요한 판단 2~3개
- 이번에 같이 묶어야 할 것
- 이번에 묶지 말아야 할 것

## 순차 작업

### Step 1. Markdown / Codeblock Upgrade

기준:
- `chatMarkdownCodeblockUpgradePlan_2026-03-29.md`

### Step 2. FileViewer Integration

기준:
- `chatFileViewerIntegrationPlan_2026-03-29.md`

### Step 3. Message density / grouping 개선

이번 단계에서는 아래를 검토하고 구현하라:
- grouped message spacing
- header density
- branch/follow-up metadata placement
- 필요 시 `MessageMeta.tsx`, `MessageItem.tsx`, `MessageActions.tsx` 정리

### Step 4. 긴 대화 scroll / virtualization 검토 및 구현

이번 단계에서는:
- virtualization이 필요한지 먼저 의견 제시
- 필요하면 최소 도입
- 불필요하면 이유를 설명하고 deferred로 남겨라

## 이번 마스터 프롬프트에서 하지 말 것

- 입력 command palette
- quick chips
- 전체 레이아웃 재설계
- 엔진/컨텍스트 아키텍처 변경

## 검증

각 단계마다 가능한 범위에서:
- type check
- 관련 테스트
- 수동 UX 확인 포인트

## 결과 보고 형식

### A. Overall Opinion
### B. Step-by-Step Results

각 step마다:
- Opinion
- Changes Made
- Verification
- Residual Risk
- Recommendation

### C. Files Changed
### D. What Still Falls Short of tunaChat
### E. Next Suggested Step
```

