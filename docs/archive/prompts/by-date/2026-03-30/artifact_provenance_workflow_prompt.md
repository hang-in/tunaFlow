# tunaFlow Artifact Provenance / Workflow Linkage 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Artifact Provenance / Workflow Linkage

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactProvenanceWorkflowPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactDetailViewPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactManualPromotionMvpPlan_2026-03-30.md`

우선 짧게 의견부터 말하라:
- 왜 지금 단계에서 artifact search보다 provenance/workflow linkage가 더 중요한지
- 목록과 detail 중 어디에 어떤 provenance를 보여주는 것이 적절한지
- 이번 단계에서 자동 연결이나 graph view로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. artifact가 어디서 왔는지 UI에서 더 명확히 보이게 한다
2. plan/subtask/workflow 연결이 있으면 이를 읽을 수 있게 한다
3. artifact를 문서 허브이자 작업 흐름의 일부처럼 느끼게 만든다

## 권장 방향

- 목록에는 compact provenance
- detail modal에는 더 많은 메타
- 이미 있는 관계 데이터만 우선 재사용

## 수정 대상 후보

- `src/components/tunaflow/context-panel/ArtifactsPanel.tsx`
- artifact detail modal

## 중요

- 이번 단계는 provenance/linkage다
- artifact-plan 자동 생성 금지
- graph view 금지
- full audit trail 금지
- deep linking 대규모 구현 금지

## 검증

- `tsc --noEmit`
- artifact 출처가 이전보다 더 명확히 읽히는지
- linked subtask/plan 정보가 있는 경우 어떻게 보이게 되었는지 설명
- artifact가 workflow의 일부처럼 보이는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Provenance Model
### E. Verification
### F. Next Recommendation
```
