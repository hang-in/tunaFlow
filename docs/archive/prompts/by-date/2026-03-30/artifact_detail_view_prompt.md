# tunaFlow Artifact 상세 보기 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Artifact Detail View

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactDetailViewPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactsTabUsabilityPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactManualPromotionMvpPlan_2026-03-30.md`

우선 짧게 의견부터 말하라:
- 왜 지금 Artifacts에 검색보다 상세 보기가 먼저 필요한지
- modal / inline expand / split pane 중 어떤 방식이 가장 안전한지
- 이번 단계에서 full editor나 export 기능으로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. artifact 카드에서 상세 보기 진입 제공
2. title/type/status/date/content 전체 표시
3. 가능하면 detail view에서 status 변경 지원

## 권장 방향

- modal detail view 우선
- 읽기 중심
- 메타는 상단 compact row
- 본문은 scroll 가능한 content 영역

## 수정 대상 후보

- `src/components/tunaflow/context-panel/ArtifactsPanel.tsx`
- 새 detail modal 컴포넌트

## 중요

- 이번 단계는 detail view다
- full editor 금지
- export/publish 금지
- search 도입 금지
- artifact-plan 자동 연결 금지

## 검증

- `tsc --noEmit`
- artifact 목록에서 detail 진입이 동작하는지
- 긴 content를 전체로 읽을 수 있는지
- detail에서 어떤 메타를 확인할 수 있게 되었는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Detail UX Model
### E. Verification
### F. Next Recommendation
```
